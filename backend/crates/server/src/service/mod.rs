//! 应用逻辑层：调 repo + hub，返 AppError（不碰 HTTP 类型）。

pub mod admin_service;
pub mod agent_service;
pub mod auth_service;
pub mod command_handler;
pub mod device_service;
pub mod email_verification_service;
pub mod integration_service;
pub mod jwt;
pub mod login_lockout;
pub mod mailer_service;
pub mod password_reset_service;
pub mod pow;
pub mod secrets;
pub mod state;
