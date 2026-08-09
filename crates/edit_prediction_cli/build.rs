fn main() {
    let cargo_toml =
        std::fs::read_to_string("../zed/Cargo.toml").expect("无法读取 crates/zed/Cargo.toml");
    let version = cargo_toml
        .lines()
        .find(|line| line.starts_with("version = "))
        .expect("在 crates/zed/Cargo.toml 中未找到版本号")
        .split('=')
        .nth(1)
        .expect("版本号格式无效")
        .trim()
        .trim_matches('"');
    println!("cargo:rustc-env=ZED_PKG_VERSION={}", version);
}
