//! إعدادات المشروع الكامل
//!
//! المشروع = مجموعة كيانات + مظهر + إعدادات. يُخزَّن في PostgreSQL.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{EntityConfig, ThemeConfig};

/// إعدادات مشروع Novax كامل
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// معرّف فريد للمشروع
    pub id: Uuid,

    /// اسم المشروع (PascalCase: MyStore, BlogApp)
    pub name: String,

    /// الاسم المعروض (عربي: "متجري")
    pub display_name: String,

    /// وصف المشروع
    pub description: Option<String>,

    /// الكيانات (Entities)
    pub entities: Vec<EntityConfig>,

    /// المظهر
    #[serde(default)]
    pub theme: ThemeConfig,

    /// هل المشروع مفعَّل؟
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// تاريخ الإنشاء
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// آخر تحديث
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn default_true() -> bool { true }

impl ProjectConfig {
    /// إنشاء مشروع جديد فارغ
    pub fn new(name: &str, display_name: &str) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            display_name: display_name.to_string(),
            description: None,
            entities: Vec::new(),
            theme: ThemeConfig::default(),
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// اسم المشروع كـ snake_case (يُستخدم كاسم مجلد)
    pub fn dir_name(&self) -> String {
        crate::to_snake_case(&self.name)
    }

    /// البحث عن كيان بالاسم
    pub fn find_entity(&self, name: &str) -> Option<&EntityConfig> {
        self.entities.iter().find(|e| e.name == name)
    }

    /// البحث عن كيان بالمعرّف
    pub fn find_entity_by_id(&self, id: Uuid) -> Option<&EntityConfig> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// إضافة كيان جديد
    pub fn add_entity(&mut self, entity: EntityConfig) {
        self.entities.push(entity);
        self.updated_at = chrono::Utc::now();
    }

    /// حذف كيان بالمعرّف
    pub fn remove_entity(&mut self, id: Uuid) -> bool {
        let before = self.entities.len();
        self.entities.retain(|e| e.id != id);
        let removed = self.entities.len() < before;
        if removed {
            self.updated_at = chrono::Utc::now();
        }
        removed
    }

    /// التحقق من صحة المشروع كاملًا
    pub fn validate(&self) -> Result<(), crate::CoreError> {
        if !crate::is_valid_identifier(&self.name) {
            return Err(crate::CoreError::InvalidEntityName(self.name.clone()));
        }

        // التحقق من عدم تكرار أسماء الكيانات
        let mut seen = std::collections::HashSet::new();
        for entity in &self.entities {
            entity.validate()?;
            if !seen.insert(&entity.name) {
                return Err(crate::CoreError::InvalidEntityName(format!("كيان مكرر: {}", entity.name)));
            }
        }

        Ok(())
    }

    /// توليد JSON للتخزين في PostgreSQL
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// قراءة من JSON (من PostgreSQL)
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
