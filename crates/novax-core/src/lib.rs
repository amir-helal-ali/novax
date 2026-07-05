//! novax-core — نماذج جوهر منصة Novax
//!
//! هذا الـ crate يُعرّف كل نماذج البيانات التي تصف "مشروع Novax":
//! - `ProjectConfig`: مشروع كامل (اسم، كيانات، مظهر)
//! - `EntityConfig`: كيان واحد (مثل "Product" أو "Order")
//! - `FieldConfig`: حقل واحد داخل كيان (مثل "title" أو "price")
//! - `ThemeConfig`: إعدادات المظهر (ألوان، خطوط، زوايا)
//!
//! هذه النماذج تُخزَّن في PostgreSQL (migration #006) وتُستخدم لاحقًا
//! من قِبل `novax-compiler` لتوليد كود Rust + HTML + SQL + CSS.

pub mod entity;
pub mod field;
pub mod field_config;
pub mod project;
pub mod theme;

pub use entity::*;
pub use field::*;
pub use field_config::*;
pub use project::*;
pub use theme::*;

use thiserror::Error;

/// أخطاء التحقق من صحة النماذج
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("اسم الكيان غير صالح: {0}")]
    InvalidEntityName(String),
    #[error("اسم الحقل غير صالح: {0}")]
    InvalidFieldName(String),
    #[error("نوع الحقل غير مدعوم: {0}")]
    UnsupportedFieldType(String),
    #[error("الكيان يجب أن يحتوي على مفتاح أساسي واحد على الأقل")]
    MissingPrimaryKey,
}

/// التحقق من أن الاسم هو معرّف Rust صالح
pub fn is_valid_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let first = name.chars().next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    name.chars().all(|c| c.is_alphanumeric() || c == '_')
}
