// 让新增迁移文件触发重编译。
// 否则只加迁移文件、不改任何 .rs，cargo run 不会重新编译，sqlx::migrate! 嵌入的新迁移静默不应用。
// 路径相对 server crate 的 Cargo.toml 目录（与 src/main.rs 里 migrate!("../../migrations") 一致）。
// sqlx 官方文档 src/macros/mod.rs:800-860 明确推荐此修复（或用 `sqlx migrate build-script` 生成）。
fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
