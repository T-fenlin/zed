mod fake_git_repo_tests;

use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use futures::{FutureExt, StreamExt};

use fs::*;
use gpui::{BackgroundExecutor, TestAppContext};
use serde_json::json;
use tempfile::TempDir;
use util::path;

#[gpui::test]
async fn test_fake_fs(executor: BackgroundExecutor) {
    let fs = FakeFs::new(executor.clone());
    fs.insert_tree(
        path!("/root"),
        json!({
            "dir1": {
                "a": "A",
                "b": "B"
            },
            "dir2": {
                "c": "C",
                "dir3": {
                    "d": "D"
                }
            }
        }),
    )
    .await;

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/root/dir1/a")),
            PathBuf::from(path!("/root/dir1/b")),
            PathBuf::from(path!("/root/dir2/c")),
            PathBuf::from(path!("/root/dir2/dir3/d")),
        ]
    );

    fs.create_symlink(path!("/root/dir2/link-to-dir3").as_ref(), "./dir3".into())
        .await
        .unwrap();

    assert_eq!(
        fs.canonicalize(path!("/root/dir2/link-to-dir3").as_ref())
            .await
            .unwrap(),
        PathBuf::from(path!("/root/dir2/dir3")),
    );
    assert_eq!(
        fs.canonicalize(path!("/root/dir2/link-to-dir3/d").as_ref())
            .await
            .unwrap(),
        PathBuf::from(path!("/root/dir2/dir3/d")),
    );
    assert_eq!(
        fs.load(path!("/root/dir2/link-to-dir3/d").as_ref())
            .await
            .unwrap(),
        "D",
    );
}

#[gpui::test]
async fn test_copy_recursive_with_single_file(executor: BackgroundExecutor) {
    let fs = FakeFs::new(executor.clone());
    fs.insert_tree(
        path!("/outer"),
        json!({
            "a": "A",
            "b": "B",
            "inner": {}
        }),
    )
    .await;

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/a")),
            PathBuf::from(path!("/outer/b")),
        ]
    );

    let source = Path::new(path!("/outer/a"));
    let target = Path::new(path!("/outer/a copy"));
    copy_recursive(fs.as_ref(), source, target, Default::default())
        .await
        .unwrap();

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/a")),
            PathBuf::from(path!("/outer/a copy")),
            PathBuf::from(path!("/outer/b")),
        ]
    );

    let source = Path::new(path!("/outer/a"));
    let target = Path::new(path!("/outer/inner/a copy"));
    copy_recursive(fs.as_ref(), source, target, Default::default())
        .await
        .unwrap();

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/a")),
            PathBuf::from(path!("/outer/a copy")),
            PathBuf::from(path!("/outer/b")),
            PathBuf::from(path!("/outer/inner/a copy")),
        ]
    );
}

#[gpui::test]
async fn test_copy_recursive_with_single_dir(executor: BackgroundExecutor) {
    let fs = FakeFs::new(executor.clone());
    fs.insert_tree(
        path!("/outer"),
        json!({
            "a": "A",
            "empty": {},
            "non-empty": {
                "b": "B",
            }
        }),
    )
    .await;

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/a")),
            PathBuf::from(path!("/outer/non-empty/b")),
        ]
    );
    assert_eq!(
        fs.directories(false),
        vec![
            PathBuf::from(path!("/")),
            PathBuf::from(path!("/outer")),
            PathBuf::from(path!("/outer/empty")),
            PathBuf::from(path!("/outer/non-empty")),
        ]
    );

    let source = Path::new(path!("/outer/empty"));
    let target = Path::new(path!("/outer/empty copy"));
    copy_recursive(fs.as_ref(), source, target, Default::default())
        .await
        .unwrap();

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/a")),
            PathBuf::from(path!("/outer/non-empty/b")),
        ]
    );
    assert_eq!(
        fs.directories(false),
        vec![
            PathBuf::from(path!("/")),
            PathBuf::from(path!("/outer")),
            PathBuf::from(path!("/outer/empty")),
            PathBuf::from(path!("/outer/empty copy")),
            PathBuf::from(path!("/outer/non-empty")),
        ]
    );

    let source = Path::new(path!("/outer/non-empty"));
    let target = Path::new(path!("/outer/non-empty copy"));
    copy_recursive(fs.as_ref(), source, target, Default::default())
        .await
        .unwrap();

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/a")),
            PathBuf::from(path!("/outer/non-empty/b")),
            PathBuf::from(path!("/outer/non-empty copy/b")),
        ]
    );
    assert_eq!(
        fs.directories(false),
        vec![
            PathBuf::from(path!("/")),
            PathBuf::from(path!("/outer")),
            PathBuf::from(path!("/outer/empty")),
            PathBuf::from(path!("/outer/empty copy")),
            PathBuf::from(path!("/outer/non-empty")),
            PathBuf::from(path!("/outer/non-empty copy")),
        ]
    );
}

#[gpui::test]
async fn test_copy_recursive(executor: BackgroundExecutor) {
    let fs = FakeFs::new(executor.clone());
    fs.insert_tree(
        path!("/outer"),
        json!({
            "inner1": {
                "a": "A",
                "b": "B",
                "inner3": {
                    "d": "D",
                },
                "inner4": {}
            },
            "inner2": {
                "c": "C",
            }
        }),
    )
    .await;

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/inner1/a")),
            PathBuf::from(path!("/outer/inner1/b")),
            PathBuf::from(path!("/outer/inner2/c")),
            PathBuf::from(path!("/outer/inner1/inner3/d")),
        ]
    );
    assert_eq!(
        fs.directories(false),
        vec![
            PathBuf::from(path!("/")),
            PathBuf::from(path!("/outer")),
            PathBuf::from(path!("/outer/inner1")),
            PathBuf::from(path!("/outer/inner2")),
            PathBuf::from(path!("/outer/inner1/inner3")),
            PathBuf::from(path!("/outer/inner1/inner4")),
        ]
    );

    let source = Path::new(path!("/outer"));
    let target = Path::new(path!("/outer/inner1/outer"));
    copy_recursive(fs.as_ref(), source, target, Default::default())
        .await
        .unwrap();

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/inner1/a")),
            PathBuf::from(path!("/outer/inner1/b")),
            PathBuf::from(path!("/outer/inner2/c")),
            PathBuf::from(path!("/outer/inner1/inner3/d")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/a")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/b")),
            PathBuf::from(path!("/outer/inner1/outer/inner2/c")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/inner3/d")),
        ]
    );
    assert_eq!(
        fs.directories(false),
        vec![
            PathBuf::from(path!("/")),
            PathBuf::from(path!("/outer")),
            PathBuf::from(path!("/outer/inner1")),
            PathBuf::from(path!("/outer/inner2")),
            PathBuf::from(path!("/outer/inner1/inner3")),
            PathBuf::from(path!("/outer/inner1/inner4")),
            PathBuf::from(path!("/outer/inner1/outer")),
            PathBuf::from(path!("/outer/inner1/outer/inner1")),
            PathBuf::from(path!("/outer/inner1/outer/inner2")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/inner3")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/inner4")),
        ]
    );
}

#[gpui::test]
async fn test_copy_recursive_with_overwriting(executor: BackgroundExecutor) {
    let fs = FakeFs::new(executor.clone());
    fs.insert_tree(
        path!("/outer"),
        json!({
            "inner1": {
                "a": "A",
                "b": "B",
                "outer": {
                    "inner1": {
                        "a": "B"
                    }
                }
            },
            "inner2": {
                "c": "C",
            }
        }),
    )
    .await;

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/inner1/a")),
            PathBuf::from(path!("/outer/inner1/b")),
            PathBuf::from(path!("/outer/inner2/c")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/a")),
        ]
    );
    assert_eq!(
        fs.load(path!("/outer/inner1/outer/inner1/a").as_ref())
            .await
            .unwrap(),
        "B",
    );

    let source = Path::new(path!("/outer"));
    let target = Path::new(path!("/outer/inner1/outer"));
    copy_recursive(
        fs.as_ref(),
        source,
        target,
        CopyOptions {
            overwrite: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/inner1/a")),
            PathBuf::from(path!("/outer/inner1/b")),
            PathBuf::from(path!("/outer/inner2/c")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/a")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/b")),
            PathBuf::from(path!("/outer/inner1/outer/inner2/c")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/outer/inner1/a")),
        ]
    );
    assert_eq!(
        fs.load(path!("/outer/inner1/outer/inner1/a").as_ref())
            .await
            .unwrap(),
        "A"
    );
}

#[gpui::test]
async fn test_copy_recursive_with_ignoring(executor: BackgroundExecutor) {
    let fs = FakeFs::new(executor.clone());
    fs.insert_tree(
        path!("/outer"),
        json!({
            "inner1": {
                "a": "A",
                "b": "B",
                "outer": {
                    "inner1": {
                        "a": "B"
                    }
                }
            },
            "inner2": {
                "c": "C",
            }
        }),
    )
    .await;

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/inner1/a")),
            PathBuf::from(path!("/outer/inner1/b")),
            PathBuf::from(path!("/outer/inner2/c")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/a")),
        ]
    );
    assert_eq!(
        fs.load(path!("/outer/inner1/outer/inner1/a").as_ref())
            .await
            .unwrap(),
        "B",
    );

    let source = Path::new(path!("/outer"));
    let target = Path::new(path!("/outer/inner1/outer"));
    copy_recursive(
        fs.as_ref(),
        source,
        target,
        CopyOptions {
            ignore_if_exists: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/outer/inner1/a")),
            PathBuf::from(path!("/outer/inner1/b")),
            PathBuf::from(path!("/outer/inner2/c")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/a")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/b")),
            PathBuf::from(path!("/outer/inner1/outer/inner2/c")),
            PathBuf::from(path!("/outer/inner1/outer/inner1/outer/inner1/a")),
        ]
    );
    assert_eq!(
        fs.load(path!("/outer/inner1/outer/inner1/a").as_ref())
            .await
            .unwrap(),
        "B"
    );
}

#[gpui::test]
async fn test_realfs_atomic_write(executor: BackgroundExecutor) {
    // With the file handle still open, the file should be replaced
    // https://github.com/zed-industries/zed/issues/30054
    let fs = RealFs::new(None, executor);
    let temp_dir = TempDir::new().unwrap();
    let file_to_be_replaced = temp_dir.path().join("file.txt");
    let mut file = std::fs::File::create_new(&file_to_be_replaced).unwrap();
    file.write_all(b"Hello").unwrap();
    // drop(file);  // We still hold the file handle here
    let content = std::fs::read_to_string(&file_to_be_replaced).unwrap();
    assert_eq!(content, "Hello");
    gpui::block_on(fs.atomic_write(file_to_be_replaced.clone(), "World".into())).unwrap();
    let content = std::fs::read_to_string(&file_to_be_replaced).unwrap();
    assert_eq!(content, "World");
}

#[gpui::test]
async fn test_realfs_atomic_write_non_existing_file(executor: BackgroundExecutor) {
    let fs = RealFs::new(None, executor);
    let temp_dir = TempDir::new().unwrap();
    let file_to_be_replaced = temp_dir.path().join("file.txt");
    gpui::block_on(fs.atomic_write(file_to_be_replaced.clone(), "Hello".into())).unwrap();
    let content = std::fs::read_to_string(&file_to_be_replaced).unwrap();
    assert_eq!(content, "Hello");
}

#[gpui::test]
#[cfg(target_os = "windows")]
async fn test_realfs_canonicalize(executor: BackgroundExecutor) {
    use util::paths::SanitizedPath;

    let fs = RealFs::new(None, executor);
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("test (1).txt");
    let file = SanitizedPath::new(&file);
    std::fs::write(&file, "test").unwrap();

    let canonicalized = fs.canonicalize(file.as_path()).await;
    assert!(canonicalized.is_ok());
}

#[gpui::test]
async fn test_rename(executor: BackgroundExecutor) {
    let fs = FakeFs::new(executor.clone());
    fs.insert_tree(
        path!("/root"),
        json!({
            "src": {
                "file_a.txt": "content a",
                "file_b.txt": "content b"
            }
        }),
    )
    .await;

    fs.rename(
        Path::new(path!("/root/src/file_a.txt")),
        Path::new(path!("/root/src/new/renamed_a.txt")),
        RenameOptions {
            create_parents: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Assert that the `file_a.txt` file was being renamed and moved to a
    // different directory that did not exist before.
    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/root/src/file_b.txt")),
            PathBuf::from(path!("/root/src/new/renamed_a.txt")),
        ]
    );

    let result = fs
        .rename(
            Path::new(path!("/root/src/file_b.txt")),
            Path::new(path!("/root/src/old/renamed_b.txt")),
            RenameOptions {
                create_parents: false,
                ..Default::default()
            },
        )
        .await;

    // Assert that the `file_b.txt` file was not renamed nor moved, as
    // `create_parents` was set to `false`.
    // different directory that did not exist before.
    assert!(result.is_err());
    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/root/src/file_b.txt")),
            PathBuf::from(path!("/root/src/new/renamed_a.txt")),
        ]
    );
}

#[gpui::test]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
async fn test_realfs_parallel_rename_without_overwrite_preserves_losing_source(
    executor: BackgroundExecutor,
) {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let source_a = root.join("dir_a/shared.txt");
    let source_b = root.join("dir_b/shared.txt");
    let target = root.join("shared.txt");

    std::fs::create_dir_all(source_a.parent().unwrap()).unwrap();
    std::fs::create_dir_all(source_b.parent().unwrap()).unwrap();
    std::fs::write(&source_a, "from a").unwrap();
    std::fs::write(&source_b, "from b").unwrap();

    let fs = RealFs::new(None, executor);
    let (first_result, second_result) = futures::future::join(
        fs.rename(&source_a, &target, RenameOptions::default()),
        fs.rename(&source_b, &target, RenameOptions::default()),
    )
    .await;

    assert_ne!(first_result.is_ok(), second_result.is_ok());
    assert!(target.exists());
    assert_eq!(source_a.exists() as u8 + source_b.exists() as u8, 1);
}

#[gpui::test]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
async fn test_realfs_rename_ignore_if_exists_leaves_source_and_target_unchanged(
    executor: BackgroundExecutor,
) {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();
    let source = root.join("source.txt");
    let target = root.join("target.txt");

    std::fs::write(&source, "from source").unwrap();
    std::fs::write(&target, "from target").unwrap();

    let fs = RealFs::new(None, executor);
    let result = fs
        .rename(
            &source,
            &target,
            RenameOptions {
                ignore_if_exists: true,
                ..Default::default()
            },
        )
        .await;

    assert!(result.is_ok());

    assert_eq!(std::fs::read_to_string(&source).unwrap(), "from source");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "from target");
}

#[gpui::test]
#[cfg(unix)]
async fn test_realfs_broken_symlink_metadata(executor: BackgroundExecutor) {
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.path();
    let fs = RealFs::new(None, executor);
    let symlink_path = path.join("symlink");
    gpui::block_on(fs.create_symlink(&symlink_path, PathBuf::from("file_a.txt"))).unwrap();
    let metadata = fs
        .metadata(&symlink_path)
        .await
        .expect("metadata call succeeds")
        .expect("metadata returned");
    assert!(metadata.is_symlink);
    assert!(!metadata.is_dir);
    assert!(!metadata.is_fifo);
    assert!(!metadata.is_executable);
    // don't care about len or mtime on symlinks?
}

#[gpui::test]
#[cfg(unix)]
async fn test_realfs_symlink_loop_metadata(executor: BackgroundExecutor) {
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.path();
    let fs = RealFs::new(None, executor);
    let symlink_path = path.join("symlink");
    gpui::block_on(fs.create_symlink(&symlink_path, PathBuf::from("symlink"))).unwrap();
    let metadata = fs
        .metadata(&symlink_path)
        .await
        .expect("metadata call succeeds")
        .expect("metadata returned");
    assert!(metadata.is_symlink);
    assert!(!metadata.is_dir);
    assert!(!metadata.is_fifo);
    assert!(!metadata.is_executable);
    // don't care about len or mtime on symlinks?
}

#[gpui::test]
async fn test_fake_fs_trash(executor: BackgroundExecutor) {
    let fs = FakeFs::new(executor.clone());
    fs.insert_tree(
        path!("/root"),
        json!({
            "src": {
                "file_c.txt": "File C",
                "file_d.txt": "File D"
            },
            "file_a.txt": "File A",
            "file_b.txt": "File B",
        }),
    )
    .await;

    // Trashing a file.
    let path = path!("/root/file_a.txt").as_ref();
    fs.trash(path, Default::default())
        .await
        .expect("应能够将 {path:?} 移至废纸篓");

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/root/file_b.txt")),
            PathBuf::from(path!("/root/src/file_c.txt")),
            PathBuf::from(path!("/root/src/file_d.txt"))
        ]
    );

    // Trashing a directory.
    let path = path!("/root/src").as_ref();
    fs.trash(
        path,
        RemoveOptions {
            recursive: true,
            ..Default::default()
        },
    )
    .await
    .expect("应能够将 {path:?} 移至废纸篓");

    assert_eq!(fs.files(), vec![PathBuf::from(path!("/root/file_b.txt"))]);
}

#[gpui::test]
async fn test_fake_fs_restore(executor: BackgroundExecutor) {
    let fs = FakeFs::new(executor.clone());
    fs.insert_tree(
        path!("/root"),
        json!({
            "src": {
                "file_a.txt": "File A",
                "file_b.txt": "File B",
            },
            "file_c.txt": "File C",
        }),
    )
    .await;

    // Attempt deleting a file, asserting that the filesystem no longer reports
    // it as part of its list of files, restore it and verify that the list of
    // files and trash has been updated accordingly.
    let path = path!("/root/src/file_a.txt").as_ref();
    let trashed_entry = fs.trash(path, Default::default()).await.unwrap();

    assert_eq!(
        fs.original_path_for_trash_id(trashed_entry),
        Some(path.to_path_buf())
    );

    fs.restore(trashed_entry).await.unwrap();
    assert_eq!(fs.original_path_for_trash_id(trashed_entry), None);

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/root/file_c.txt")),
            PathBuf::from(path!("/root/src/file_a.txt")),
            PathBuf::from(path!("/root/src/file_b.txt"))
        ]
    );

    // Deleting and restoring a directory should also remove all of its files
    // but create a single trashed entry, which should be removed after
    // restoration.
    let options = RemoveOptions {
        recursive: true,
        ..Default::default()
    };
    let path = path!("/root/src/").as_ref();
    let trashed_entry = fs.trash(path, options).await.unwrap();

    assert_eq!(fs.files(), vec![PathBuf::from(path!("/root/file_c.txt"))]);

    fs.restore(trashed_entry).await.unwrap();

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/root/file_c.txt")),
            PathBuf::from(path!("/root/src/file_a.txt")),
            PathBuf::from(path!("/root/src/file_b.txt"))
        ]
    );

    // A collision error should be returned in case a file is being restored to
    // a path where a file already exists.
    let path = path!("/root/src/file_a.txt").as_ref();
    let trashed_entry = fs.trash(path, Default::default()).await.unwrap();

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/root/file_c.txt")),
            PathBuf::from(path!("/root/src/file_b.txt"))
        ]
    );

    fs.write(path, "New File A".as_bytes()).await.unwrap();

    assert_eq!(
        fs.files(),
        vec![
            PathBuf::from(path!("/root/file_c.txt")),
            PathBuf::from(path!("/root/src/file_a.txt")),
            PathBuf::from(path!("/root/src/file_b.txt"))
        ]
    );

    let file_contents = fs.files_with_contents(path);
    assert!(fs.restore(trashed_entry).await.is_err());
    assert_eq!(
        file_contents,
        vec![(PathBuf::from(path), b"New File A".to_vec())]
    );

    // A collision error should be returned in case a directory is being
    // restored to a path where a directory already exists.
    let options = RemoveOptions {
        recursive: true,
        ..Default::default()
    };
    let path = path!("/root/src/").as_ref();
    let trashed_entry = fs.trash(path, options).await.unwrap();

    assert_eq!(fs.files(), vec![PathBuf::from(path!("/root/file_c.txt"))]);

    fs.create_dir(path).await.unwrap();

    assert_eq!(fs.files(), vec![PathBuf::from(path!("/root/file_c.txt"))]);

    let result = fs.restore(trashed_entry).await;
    assert!(result.is_err());

    assert_eq!(fs.files(), vec![PathBuf::from(path!("/root/file_c.txt"))]);
}

/// Create a directory symlink (`link` -> `target`) in a cross-platform way.
///
/// Returns `Err` when the platform cannot create symlinks (e.g. Windows without
/// the create-symlink privilege), so callers can skip a scenario gracefully
/// rather than failing the whole test.
fn make_dir_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "此平台不支持符号链接",
        ))
    }
}

/// Waits up to `timeout` for `events` to deliver something that covers the
/// written file: either a path event whose path satisfies `path_matches`, or a
/// `Rescan` (which tells the consumer to re-scan this watcher's whole tree, so
/// it would discover the file anyway). Returns `false` if nothing relevant
/// arrives before the timeout.
async fn watcher_delivered_event(
    events: &mut (impl futures::Stream<Item = Vec<PathEvent>> + Unpin),
    executor: &BackgroundExecutor,
    timeout: Duration,
    path_matches: &(dyn Fn(&Path) -> bool + Send + Sync),
) -> bool {
    let timeout = executor.timer(timeout).fuse();
    futures::pin_mut!(timeout);
    loop {
        futures::select_biased! {
            batch = events.next().fuse() => {
                let Some(batch) = batch else { return false };
                let covered = batch.iter().any(|event| {
                    path_matches(&event.path) || event.kind == Some(PathEventKind::Rescan)
                });
                if covered {
                    return true;
                }
            }
            _ = timeout => return false,
        }
    }
}

/// Exercises a spread of real watchers whose registered watch path is spelled
/// differently from the path the OS reports events under. Each scenario watches
/// some directory and then mutates the on-disk file; a correct watcher must
/// deliver an event (or a rescan) for every scenario.
///
/// This asserts the residual path-aliasing bugs that real-casing the watch root
/// at add-time does NOT fix. The headline failure is `symlink_ancestor`:
/// watching a path that traverses a symlinked ancestor. On macOS FSEvents
/// reports events under the resolved real path, which no longer has the
/// symlinked prefix the watch root was registered with, so the events are
/// filtered out and never delivered.
///
/// Platform notes (the test runs everywhere but scenarios self-skip when they
/// cannot apply):
/// - Case scenarios require a case-insensitive filesystem (macOS/Windows
///   default; case-sensitive Linux/APFS skip them).
/// - Symlink scenarios require symlink creation (skipped on Windows without the
///   privilege).
/// - `symlink_ancestor` fails on macOS (FSEvents canonicalizes) but is expected
///   to pass on Linux (notify reconstructs paths from the watch path you pass),
///   which is itself a useful demonstration that this is an FSEvents-specific
///   bug.
#[gpui::test]
async fn test_realfs_watch_aliased_watch_paths_deliver_events(
    executor: BackgroundExecutor,
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();

    let fs = RealFs::new(None, executor.clone());
    let temp_dir = TempDir::new().expect("创建临时目录");
    let root = temp_dir.path().to_path_buf();
    let latency = Duration::from_millis(10);

    // Probe the real filesystem for case sensitivity rather than guessing from
    // the platform.
    std::fs::create_dir_all(root.join("CaseProbe")).expect("创建大小写探测目录");
    let case_insensitive = root.join("caseprobe").exists();
    eprintln!("文件系统不区分大小写: {case_insensitive}");

    struct Scenario {
        name: &'static str,
        events: Pin<Box<dyn Send + futures::Stream<Item = Vec<PathEvent>>>>,
        _watcher: Arc<dyn Watcher>,
        path_matches: Box<dyn Fn(&Path) -> bool + Send + Sync>,
        action: Option<Box<dyn FnOnce() + Send>>,
    }

    let mut scenarios: Vec<Scenario> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    // --- Headline residual bug: watch path traverses a symlinked ancestor. ---
    {
        let real = root.join("ancestor_real");
        let inner = real.join("inner");
        std::fs::create_dir_all(&inner).expect("创建符号链接祖先目标");
        let link = root.join("ancestor_link");
        match make_dir_symlink(&real, &link) {
            Ok(()) => {
                let (events, watcher) = fs.watch(&link.join("inner"), latency).await;
                let file = inner.join("symlink_ancestor.txt");
                scenarios.push(Scenario {
                    name: "symlink_ancestor",
                    events,
                    _watcher: watcher,
                    path_matches: Box::new(|path| {
                        path.ends_with(Path::new("inner/symlink_ancestor.txt"))
                    }),
                    action: Some(Box::new(move || {
                        std::fs::write(&file, b"x").expect("写入符号链接祖先文件");
                    })),
                });
            }
            Err(error) => skipped.push(format!("symlink_ancestor (无法创建符号链接: {error})")),
        }
    }

    // --- Control: watching a symlinked root IS handled (RealFs::watch follows
    //     the root symlink and also watches the target). ---
    {
        let real = root.join("root_real");
        std::fs::create_dir_all(&real).expect("创建符号链接根目标");
        let link = root.join("root_link");
        match make_dir_symlink(&real, &link) {
            Ok(()) => {
                let (events, watcher) = fs.watch(&link, latency).await;
                let file = real.join("symlink_root.txt");
                scenarios.push(Scenario {
                    name: "symlink_root",
                    events,
                    _watcher: watcher,
                    path_matches: Box::new(|path| path.ends_with(Path::new("symlink_root.txt"))),
                    action: Some(Box::new(move || {
                        std::fs::write(&file, b"x").expect("写入符号链接根文件");
                    })),
                });
            }
            Err(error) => skipped.push(format!("symlink_root (无法创建符号链接: {error})")),
        }
    }

    // --- Control: wrong-case watch root (the originally-reported bug, which the
    //     real-casing fix already addresses). ---
    if case_insensitive {
        let real = root.join("CaseAlpha");
        std::fs::create_dir_all(&real).expect("创建错误大小写根目录");
        let lower = PathBuf::from(real.to_string_lossy().to_lowercase());
        let (events, watcher) = fs.watch(&lower, latency).await;
        let file = real.join("alpha.txt");
        scenarios.push(Scenario {
            name: "wrong_case_root",
            events,
            _watcher: watcher,
            path_matches: Box::new(|path| path.ends_with(Path::new("alpha.txt"))),
            action: Some(Box::new(move || {
                std::fs::write(&file, b"x").expect("写入错误大小写根文件");
            })),
        });
    } else {
        skipped.push("wrong_case_root (区分大小写的文件系统)".to_owned());
    }

    // --- Control: wrong-case nested watch path. ---
    if case_insensitive {
        let real = root.join("CaseBravo").join("Inner");
        std::fs::create_dir_all(&real).expect("创建错误大小写嵌套目录");
        let lower = PathBuf::from(real.to_string_lossy().to_lowercase());
        let (events, watcher) = fs.watch(&lower, latency).await;
        let file = real.join("bravo.txt");
        scenarios.push(Scenario {
            name: "nested_wrong_case",
            events,
            _watcher: watcher,
            path_matches: Box::new(|path| path.ends_with(Path::new("bravo.txt"))),
            action: Some(Box::new(move || {
                std::fs::write(&file, b"x").expect("写入错误大小写嵌套文件");
            })),
        });
    } else {
        skipped.push("nested_wrong_case (区分大小写的文件系统)".to_owned());
    }

    // --- Residual bug: the watched root is renamed to a different casing after
    //     the watch is established, so later events arrive under a spelling the
    //     registered (old-case) root no longer matches. ---
    if case_insensitive {
        let real = root.join("CaseEcho");
        std::fs::create_dir_all(&real).expect("创建大小写重命名目录");
        let (events, watcher) = fs.watch(&real, latency).await;
        let renamed = root.join("CASEECHO");
        let file = renamed.join("echo.txt");
        scenarios.push(Scenario {
            name: "case_rename_root",
            events,
            _watcher: watcher,
            path_matches: Box::new(|path| path.ends_with(Path::new("echo.txt"))),
            action: Some(Box::new(move || {
                std::fs::rename(&real, &renamed).expect("仅大小写不同的受监视根目录重命名");
                std::fs::write(&file, b"x").expect("写入大小写重命名文件");
            })),
        });
    } else {
        skipped.push("case_rename_root (区分大小写的文件系统)".to_owned());
    }

    // Let every watch settle before mutating, then perform all mutations.
    executor.timer(Duration::from_millis(250)).await;
    for scenario in &mut scenarios {
        if let Some(action) = scenario.action.take() {
            action();
        }
    }

    let mut failures = Vec::new();
    for scenario in &mut scenarios {
        let delivered = watcher_delivered_event(
            &mut scenario.events,
            &executor,
            Duration::from_secs(3),
            scenario.path_matches.as_ref(),
        )
        .await;
        eprintln!("场景 {}: 已传递={delivered}", scenario.name);
        if !delivered {
            failures.push(scenario.name);
        }
    }

    for name in &skipped {
        eprintln!("场景已跳过: {name}");
    }

    assert!(
        failures.is_empty(),
        "监视器未能传递事件: {failures:?} (已跳过: {skipped:?})"
    );
}

#[gpui::test]
#[ignore = "压力测试;需要时显式运行"]
async fn test_realfs_watch_stress_reports_missed_paths(
    executor: BackgroundExecutor,
    cx: &mut TestAppContext,
) {
    const FILE_COUNT: usize = 32000;
    cx.executor().allow_parking();

    let fs = RealFs::new(None, executor.clone());
    let temp_dir = TempDir::new().expect("创建临时目录");
    let root = temp_dir.path();

    let mut file_paths = Vec::with_capacity(FILE_COUNT);
    let mut expected_paths = BTreeSet::new();

    for index in 0..FILE_COUNT {
        let dir_path = root.join(format!("dir-{index:04}"));
        let file_path = dir_path.join("file.txt");
        fs.create_dir(&dir_path).await.expect("创建受监视目录");
        fs.write(&file_path, b"before")
            .await
            .expect("创建初始文件");
        expected_paths.insert(file_path.clone());
        file_paths.push(file_path);
    }

    let (mut events, watcher) = fs.watch(root, Duration::from_millis(10)).await;
    let _watcher = watcher;

    for file_path in &expected_paths {
        _watcher
            .add(file_path.parent().expect("文件有父目录"))
            .expect("添加显式目录监视");
    }

    for (index, file_path) in file_paths.iter().enumerate() {
        let content = format!("after-{index}");
        fs.write(file_path, content.as_bytes())
            .await
            .expect("修改受监视文件");
    }

    let mut changed_paths = BTreeSet::new();
    let mut rescan_count: u32 = 0;
    let timeout = executor.timer(Duration::from_secs(10)).fuse();

    futures::pin_mut!(timeout);

    let mut ticks = 0;
    while ticks < 1000 {
        if let Some(batch) = events.next().fuse().now_or_never().flatten() {
            for event in batch {
                if event.kind == Some(PathEventKind::Rescan) {
                    rescan_count += 1;
                }
                if expected_paths.contains(&event.path) {
                    changed_paths.insert(event.path);
                }
            }
            if changed_paths.len() == expected_paths.len() {
                break;
            }
            ticks = 0;
        } else {
            ticks += 1;
            executor.timer(Duration::from_millis(10)).await;
        }
    }

    let missed_paths: BTreeSet<_> = expected_paths.difference(&changed_paths).cloned().collect();

    eprintln!(
        "realfs 监视压力测试: 预期={}, 观察到={}, 错过={}, 重新扫描={}",
        expected_paths.len(),
        changed_paths.len(),
        missed_paths.len(),
        rescan_count
    );

    assert!(
        missed_paths.is_empty() || rescan_count > 0,
        "错过了 {} 个路径,但未报告重新扫描",
        missed_paths.len()
    );
}

#[gpui::test]
async fn restore_can_be_retried_after_collision(cx: &mut TestAppContext) {
    let fs = FakeFs::new(cx.background_executor.clone());
    let path = path!("/root/a.txt");
    let remove_options = RemoveOptions::default();
    fs.insert_tree(path!("/root"), json!({ "a.txt": "original"}))
        .await;

    // We'll first trash the `a.txt` file so we can hold onto its `TrashId`,
    // allowing us to later attempt restoring it again, ensuring that it didn't
    // get removed from the trash state, even if restoring failed.
    let trash_id = fs.trash(path.as_ref(), remove_options).await.unwrap();

    fs.insert_file(path, "conflicting".into()).await;
    let err = fs.restore(trash_id).await.unwrap_err();
    assert!(matches!(err, TrashRestoreError::Collision { .. }));

    fs.remove_file(path.as_ref(), remove_options).await.unwrap();
    let restored_path = fs.restore(trash_id).await.unwrap();
    assert_eq!(fs.load(restored_path.as_path()).await.unwrap(), "original");

    assert!(matches!(
        fs.restore(trash_id).await.unwrap_err(),
        TrashRestoreError::AlreadyRestored
    ));
}
