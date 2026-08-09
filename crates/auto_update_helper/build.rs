fn main() {
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-env-changed=RELEASE_CHANNEL");
        println!("cargo:rerun-if-env-changed=GITHUB_RUN_NUMBER");

        windows_resources::compile(true).expect("编译 Windows 资源失败");
    }
}
