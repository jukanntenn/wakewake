//! wakewake-server 库根。模块层级见 architecture/onion-architecture.md。
//!
//! 依赖方向：routes ─► service ─► repo ─► DB
//!                       └──► hub ─► (内存)
//! domain：零项目内依赖（纯内核）。

// 编译期加载 locales/*.toml（8 语言，fallback=en）。backend/i18n.md §5。
// 此宏必须在 crate 根展开，生成 _rust_i18n_t! 宏供 t! 调用。
rust_i18n::i18n!("locales", fallback = "en");

pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod hub;
pub mod i18n;
pub mod integrations;
pub mod management;
pub mod middleware;
pub mod observability;
pub mod repo;
pub mod routes;
pub mod service;
pub mod state;
pub mod util;
