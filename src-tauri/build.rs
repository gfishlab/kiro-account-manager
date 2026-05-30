fn main() {
    // 禁用对 dist 目录的监控，避免 Vite 构建触发 Rust 重编译
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=tauri.conf.json");

    tauri_build::build();
}
