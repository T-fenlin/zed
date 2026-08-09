use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::Path,
    sync::LazyLock,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        System::RestartManager::{
            CCH_RM_SESSION_KEY, RmEndSession, RmGetList, RmRegisterResources, RmShutdown,
            RmStartSession,
        },
        UI::WindowsAndMessaging::PostMessageW,
    },
    core::{PCWSTR, PWSTR},
};

use crate::windows_impl::WM_JOB_UPDATED;

pub(crate) struct Job {
    pub apply: Box<dyn Fn(&Path) -> Result<()> + Send + Sync>,
    pub rollback: Box<dyn Fn(&Path) -> Result<()> + Send + Sync>,
}

impl Job {
    pub fn mkdir(name: &'static Path) -> Self {
        Job {
            apply: Box::new(move |app_dir| {
                let dir = app_dir.join(name);
                std::fs::create_dir_all(&dir)
                    .context(format!("无法创建目录 {}", dir.display()))
            }),
            rollback: Box::new(move |app_dir| {
                let dir = app_dir.join(name);
                std::fs::remove_dir_all(&dir)
                    .context(format!("删除目录 {} 失败", dir.display()))
            }),
        }
    }

    pub fn mkdir_if_exists(name: &'static Path, check: &'static Path) -> Self {
        Job {
            apply: Box::new(move |app_dir| {
                let dir = app_dir.join(name);
                let check = app_dir.join(check);

                if check.exists() {
                    std::fs::create_dir_all(&dir)
                        .context(format!("无法创建目录 {}", dir.display()))?
                }
                Ok(())
            }),
            rollback: Box::new(move |app_dir| {
                let dir = app_dir.join(name);

                if dir.exists() {
                    std::fs::remove_dir_all(&dir)
                        .context(format!("删除目录 {} 失败", dir.display()))?
                }

                Ok(())
            }),
        }
    }

    pub fn move_file(filename: &'static Path, new_filename: &'static Path) -> Self {
        Job {
            apply: Box::new(move |app_dir| {
                let old_file = app_dir.join(filename);
                let new_file = app_dir.join(new_filename);
                log::info!(
                    "移动文件: {}->{}",
                    old_file.display(),
                    new_file.display()
                );

                std::fs::rename(&old_file, new_file)
                    .context(format!("移动文件 {} 失败", old_file.display()))
            }),
            rollback: Box::new(move |app_dir| {
                let old_file = app_dir.join(filename);
                let new_file = app_dir.join(new_filename);
                log::info!(
                    "回滚文件移动: {}->{}",
                    old_file.display(),
                    new_file.display()
                );

                std::fs::rename(&new_file, &old_file).context(format!(
                    "回滚文件移动 {}->{} 失败",
                    new_file.display(),
                    old_file.display()
                ))
            }),
        }
    }

    pub fn move_if_exists(filename: &'static Path, new_filename: &'static Path) -> Self {
        Job {
            apply: Box::new(move |app_dir| {
                let old_file = app_dir.join(filename);
                let new_file = app_dir.join(new_filename);

                if old_file.exists() {
                    log::info!(
                        "移动文件: {}->{}",
                        old_file.display(),
                        new_file.display()
                    );

                    std::fs::rename(&old_file, new_file)
                        .context(format!("移动文件 {} 失败", old_file.display()))?;
                }

                Ok(())
            }),
            rollback: Box::new(move |app_dir| {
                let old_file = app_dir.join(filename);
                let new_file = app_dir.join(new_filename);

                if new_file.exists() {
                    log::info!(
                        "回滚文件移动: {}->{}",
                        old_file.display(),
                        new_file.display()
                    );

                    std::fs::rename(&new_file, &old_file).context(format!(
                        "回滚文件移动 {}->{} 失败",
                        new_file.display(),
                        old_file.display()
                    ))?
                }

                Ok(())
            }),
        }
    }

    pub fn rmdir_nofail(filename: &'static Path) -> Self {
        Job {
            apply: Box::new(move |app_dir| {
                let filename = app_dir.join(filename);
                log::info!("删除文件: {}", filename.display());
                if let Err(e) = std::fs::remove_dir_all(&filename) {
                    log::warn!("删除目录失败: {}", e);
                }

                Ok(())
            }),
            rollback: Box::new(move |app_dir| {
                let filename = app_dir.join(filename);
                anyhow::bail!(
                    "删除操作无法回滚,文件: {}",
                    filename.display()
                )
            }),
        }
    }
}

#[cfg(not(test))]
pub(crate) static JOBS: LazyLock<[Job; 22]> = LazyLock::new(|| {
    fn p(value: &str) -> &Path {
        Path::new(value)
    }
    [
        // Move old files
        // Not deleting because installing new files can fail
        Job::mkdir(p("old")),
        Job::move_file(p("Zed.exe"), p("old\\Zed.exe")),
        Job::mkdir(p("old\\bin")),
        Job::move_file(p("bin\\Zed.exe"), p("old\\bin\\Zed.exe")),
        Job::move_file(p("bin\\zed"), p("old\\bin\\zed")),
        //
        // TODO: remove after a few weeks once everyone is on the new version and this file never exists
        Job::move_if_exists(p("OpenConsole.exe"), p("old\\OpenConsole.exe")),
        Job::mkdir(p("old\\x64")),
        Job::mkdir(p("old\\arm64")),
        Job::move_if_exists(p("x64\\OpenConsole.exe"), p("old\\x64\\OpenConsole.exe")),
        Job::move_if_exists(
            p("arm64\\OpenConsole.exe"),
            p("old\\arm64\\OpenConsole.exe"),
        ),
        //
        Job::move_file(p("conpty.dll"), p("old\\conpty.dll")),
        // Copy new files
        Job::move_file(p("install\\Zed.exe"), p("Zed.exe")),
        Job::move_file(p("install\\bin\\Zed.exe"), p("bin\\Zed.exe")),
        Job::move_file(p("install\\bin\\zed"), p("bin\\zed")),
        //
        Job::mkdir_if_exists(p("x64"), p("install\\x64")),
        Job::mkdir_if_exists(p("arm64"), p("install\\arm64")),
        Job::move_if_exists(
            p("install\\x64\\OpenConsole.exe"),
            p("x64\\OpenConsole.exe"),
        ),
        Job::move_if_exists(
            p("install\\arm64\\OpenConsole.exe"),
            p("arm64\\OpenConsole.exe"),
        ),
        //
        Job::move_file(p("install\\conpty.dll"), p("conpty.dll")),
        // Cleanup installer and updates folder
        Job::rmdir_nofail(p("updates")),
        Job::rmdir_nofail(p("install")),
        // Cleanup old installation
        Job::rmdir_nofail(p("old")),
    ]
});

#[cfg(test)]
pub(crate) static JOBS: LazyLock<[Job; 9]> = LazyLock::new(|| {
    fn p(value: &str) -> &Path {
        Path::new(value)
    }
    [
        Job {
            apply: Box::new(|_| {
                std::thread::sleep(Duration::from_millis(1000));
                if let Ok(config) = std::env::var("ZED_AUTO_UPDATE") {
                    match config.as_str() {
                        "err1" => Err(std::io::Error::other("模拟错误")).context("Anyhow!"),
                        "err2" => Ok(()),
                        _ => panic!("未知的 ZED_AUTO_UPDATE 值: {}", config),
                    }
                } else {
                    Ok(())
                }
            }),
            rollback: Box::new(|_| {
                unsafe { std::env::set_var("ZED_AUTO_UPDATE_RB", "rollback1") };
                Ok(())
            }),
        },
        Job::mkdir(p("test1")),
        Job::mkdir_if_exists(p("test_exists"), p("test1")),
        Job::mkdir_if_exists(p("test_missing"), p("dont")),
        Job {
            apply: Box::new(|folder| {
                std::fs::write(folder.join("test1/test"), "test")?;
                Ok(())
            }),
            rollback: Box::new(|folder| {
                std::fs::remove_file(folder.join("test1/test"))?;
                Ok(())
            }),
        },
        Job::move_file(p("test1/test"), p("test1/moved")),
        Job::move_if_exists(p("test1/test"), p("test1/noop")),
        Job {
            apply: Box::new(|_| {
                std::thread::sleep(Duration::from_millis(1000));
                if let Ok(config) = std::env::var("ZED_AUTO_UPDATE") {
                    match config.as_str() {
                        "err1" => Ok(()),
                        "err2" => Err(std::io::Error::other("模拟错误")).context("Anyhow!"),
                        _ => panic!("未知的 ZED_AUTO_UPDATE 值: {}", config),
                    }
                } else {
                    Ok(())
                }
            }),
            rollback: Box::new(|_| Ok(())),
        },
        Job::rmdir_nofail(p("test1/nofolder")),
    ]
});

/// Attempts to use Windows Restart Manager to release file handles held by other processes
/// (e.g., Explorer.exe) on the files we need to move during the update.
///
/// This is a best-effort operation - if it fails, we'll still try the update and rely on
/// the retry logic.
fn release_file_handles(app_dir: &Path) -> Result<()> {
    // Files that commonly get locked by Explorer or other processes
    let files_to_release = [
        app_dir.join("Zed.exe"),
        app_dir.join("bin\\Zed.exe"),
        app_dir.join("bin\\zed"),
        app_dir.join("conpty.dll"),
    ];

    log::info!("正在尝试使用 Restart Manager 释放文件句柄...");

    let mut session: u32 = 0;
    let mut session_key = [0u16; CCH_RM_SESSION_KEY as usize + 1];

    // Start a Restart Manager session
    let err = unsafe {
        RmStartSession(
            &mut session,
            Some(0),
            PWSTR::from_raw(session_key.as_mut_ptr()),
        )
    };
    if err.is_err() {
        anyhow::bail!("RmStartSession 失败: {err:?}");
    }

    // Ensure we end the session when done
    let _session_guard = scopeguard::guard(session, |s| {
        let _ = unsafe { RmEndSession(s) };
    });

    // Convert paths to wide strings for Windows API
    let wide_paths: Vec<Vec<u16>> = files_to_release
        .iter()
        .filter(|p| p.exists())
        .map(|p| {
            OsStr::new(p)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        })
        .collect();

    if wide_paths.is_empty() {
        log::info!("没有需要释放句柄的文件");
        return Ok(());
    }

    let pcwstr_paths: Vec<PCWSTR> = wide_paths
        .iter()
        .map(|p| PCWSTR::from_raw(p.as_ptr()))
        .collect();

    // Register the files we want to modify
    let err = unsafe { RmRegisterResources(session, Some(&pcwstr_paths), None, None) };
    if err.is_err() {
        anyhow::bail!("RmRegisterResources 失败: {err:?}");
    }

    // Check if any processes are using these files
    let mut needed: u32 = 0;
    let mut count: u32 = 0;
    let mut reboot_reasons: u32 = 0;
    let _ = unsafe { RmGetList(session, &mut needed, &mut count, None, &mut reboot_reasons) };

    if needed == 0 {
        log::info!("没有进程持有这些文件的句柄");
        return Ok(());
    }

    log::info!(
        "{} 个进程持有这些文件的句柄,正在请求释放...",
        needed
    );

    // Request processes to release their handles
    // RmShutdown with flags=0 asks applications to release handles gracefully
    // For Explorer, this typically releases icon cache handles without closing Explorer
    let err = unsafe { RmShutdown(session, 0, None) };
    if err.is_err() {
        anyhow::bail!("RmShutdown 失败: {:?}", err);
    }

    log::info!("已成功请求释放句柄");
    Ok(())
}

pub(crate) fn perform_update(app_dir: &Path, hwnd: Option<isize>, launch: bool) -> Result<()> {
    let hwnd = hwnd.map(|ptr| HWND(ptr as _));

    // Try to release file handles before starting the update
    if let Err(e) = release_file_handles(app_dir) {
        log::warn!("Restart Manager 失败(将继续进行): {}", e);
    }

    let mut last_successful_job = None;
    'outer: for (i, job) in JOBS.iter().enumerate() {
        let start = Instant::now();
        loop {
            if start.elapsed().as_secs() > 2 {
                log::error!("超时,正在回滚");
                break 'outer;
            }
            match (job.apply)(app_dir) {
                Ok(_) => {
                    last_successful_job = Some(i);
                    unsafe { PostMessageW(hwnd, WM_JOB_UPDATED, WPARAM(0), LPARAM(0))? };
                    break;
                }
                Err(err) => match err.downcast_ref::<std::io::Error>() {
                    Some(io_err) => match io_err.kind() {
                        std::io::ErrorKind::NotFound => {
                            log::error!("操作因文件未找到而失败,正在中止: {}", err);
                            break 'outer;
                        }
                        _ => {
                            log::error!("操作失败(正在重试): {}", err);
                            std::thread::sleep(Duration::from_millis(50));
                        }
                    },
                    None => {
                        log::error!("操作因意外错误而失败,正在中止: {}", err);
                        break 'outer;
                    }
                },
            }
        }
    }

    if last_successful_job
        .map(|job| job != JOBS.len() - 1)
        .unwrap_or(true)
    {
        let Some(last_successful_job) = last_successful_job else {
            anyhow::bail!("自动更新失败,没有可回滚的内容");
        };

        for job in (0..=last_successful_job).rev() {
            let job = &JOBS[job];
            if let Err(e) = (job.rollback)(app_dir) {
                anyhow::bail!(
                    "任务回滚失败,应用可能处于不一致状态: ({:?})",
                    e
                );
            }
        }

        anyhow::bail!("自动更新失败,回滚成功");
    }

    if launch {
        #[allow(clippy::disallowed_methods, reason = "doesn't run in the main binary")]
        let _ = std::process::Command::new(app_dir.join("Zed.exe")).spawn();
    }
    log::info!("更新成功完成");
    Ok(())
}

#[cfg(test)]
mod test {
    use super::perform_update;

    #[test]
    fn test_perform_update() {
        let app_dir = tempfile::tempdir().unwrap();
        let app_dir = app_dir.path();
        assert!(perform_update(app_dir, None, false).is_ok());

        let app_dir = tempfile::tempdir().unwrap();
        let app_dir = app_dir.path();
        // Simulate a timeout
        unsafe { std::env::set_var("ZED_AUTO_UPDATE", "err1") };
        let ret = perform_update(app_dir, None, false);
        assert!(
            ret.is_err_and(|e| e.to_string().as_str() == "自动更新失败,没有可回滚的内容")
        );

        let app_dir = tempfile::tempdir().unwrap();
        let app_dir = app_dir.path();
        // Simulate a timeout
        unsafe { std::env::set_var("ZED_AUTO_UPDATE", "err2") };
        let ret = perform_update(app_dir, None, false);
        assert!(
            ret.is_err_and(|e| e.to_string().as_str() == "自动更新失败,回滚成功")
        );
        assert!(std::env::var("ZED_AUTO_UPDATE_RB").is_ok_and(|e| e == "rollback1"));
    }
}
