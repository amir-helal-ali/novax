//! إعدادات كيان واحد (Entity)
//!
//! الكيان = جدول في قاعدة البيانات + نموذج Rust + routes + قوالب HTML.
//! مثال: كيان "Product" يحتوي على حقول (title, price, description, ...)

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::FieldConfig;

/// إعدادات كيان
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityConfig {
    /// معرّف فريد للكيان داخل المشروع
    pub id: Uuid,

    /// اسم الكيان بالإنجليزية (PascalCase: Product, Order, UserProfile)
    pub name: String,

    /// اسم الجدول في قاعدة البيانات (snake_case جمع: products, orders)
    #[serde(default)]
    pub table_name: Option<String>,

    /// الاسم المعروض بالعربي (مفرد: "منتج")
    pub display_name: String,

    /// الاسم المعروض بالعربي (جمع: "منتجات")
    pub display_name_plural: String,

    /// الأيقونة (emoji أو اسم أيقونة)
    #[serde(default = "default_icon")]
    pub icon: String,

    /// الحقول
    pub fields: Vec<FieldConfig>,

    /// هل الكيان مفعَّل؟
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// وصف اختياري
    pub description: Option<String>,
}

fn default_true() -> bool { true }
fn default_icon() -> String { "📦".to_string() }

impl EntityConfig {
    /// إنشاء كيان جديد بحقول افتراضية (id + created_at + updated_at)
    pub fn new(name: &str, display_name: &str, display_name_plural: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            table_name: None,
            display_name: display_name.to_string(),
            display_name_plural: display_name_plural.to_string(),
            icon: default_icon(),
            fields: vec![
                FieldConfig::default_primary_key(),
                FieldConfig::created_at(),
                FieldConfig::updated_at(),
            ],
            enabled: true,
            description: None,
        }
    }

    /// اسم الجدول (محسوب من اسم الكيان إذا لم يُحدَّد)
    pub fn table(&self) -> String {
        self.table_name.clone()
            .unwrap_or_else(|| crate::to_snake_case_plural(&self.name))
    }

    /// اسم الـ struct في Rust (PascalCase)
    pub fn struct_name(&self) -> String {
        crate::to_pascal_case(&self.name)
    }

    /// المفتاح الأساسي
    pub fn primary_key(&self) -> Option<&FieldConfig> {
        self.fields.iter().find(|f| f.primary_key)
    }

    /// الحقول المعروضة في القائمة
    pub fn list_fields(&self) -> Vec<&FieldConfig> {
        self.fields.iter().filter(|f| f.display_in_list).collect()
    }

    /// الحقول المعروضة في النموذج
    pub fn form_fields(&self) -> Vec<&FieldConfig> {
        self.fields.iter().filter(|f| f.display_in_form).collect()
    }

    /// الحقول القابلة للبحث
    pub fn searchable_fields(&self) -> Vec<&FieldConfig> {
        self.fields.iter().filter(|f| f.searchable).collect()
    }

    /// توليد اسم الـ route (products, orders, user_profiles)
    pub fn route_prefix(&self) -> String {
        self.table()
    }

    /// التحقق من صحة الكيان
    pub fn validate(&self) -> Result<(), crate::CoreError> {
        if !crate::is_valid_identifier(&self.name) {
            return Err(crate::CoreError::InvalidEntityName(self.name.clone()));
        }

        if self.primary_key().is_none() {
            return Err(crate::CoreError::MissingPrimaryKey);
        }

        // التحقق من أسماء الحقول
        let mut seen_names = std::collections::HashSet::new();
        for field in &self.fields {
            if !crate::is_valid_identifier(&field.name) {
                return Err(crate::CoreError::InvalidFieldName(field.name.clone()));
            }
            if !seen_names.insert(&field.name) {
                return Err(crate::CoreError::InvalidFieldName(format!("حقل مكرر: {}", field.name)));
            }
        }

        Ok(())
    }
}
