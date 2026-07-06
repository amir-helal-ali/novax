//! novax-compiler — محرّك توليد الكود
//!
//! يأخذ `ProjectConfig` (من novax-core) ويُولّد:
//! - كود Rust (main.rs, routes.rs, models.rs, db.rs)
//! - قوالب HTML (list, form, detail, partials)
//! - ملفات SQL (CREATE TABLE + indexes)
//! - ملف CSS (من ThemeConfig)
//! - Cargo.toml + .env + README.md
//!
//! كل الكود المُولَّد مستقل تمامًا عن Novax (zero vendor lock-in).

pub mod rust_codegen;
pub mod html_codegen;
pub mod sql_codegen;
pub mod project_builder;
pub mod openapi_codegen;

pub use rust_codegen::*;
pub use html_codegen::*;
pub use sql_codegen::*;
pub use project_builder::*;
pub use openapi_codegen::*;

use thiserror::Error;

/// أخطاء التوليد
#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("خطأ في التحقق: {0}")]
    Validation(String),
    #[error("خطأ في الكتابة: {0}")]
    Io(String),
}
