//! إعدادات حقل واحد داخل كيان
//!
//! مثال: حقل "title" في كيان "Product" له إعدادات:
//! - type: String
//! - required: true
//! - max_length: 255
//! - display_in_list: true (يظهر في جدول القائمة)
//! - display_in_form: true (يظهر في نموذج الإنشاء/التعديل)
//! - label: "العنوان" (النص المعروض للمستخدم)

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::FieldType;

/// إعدادات حقل واحد
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldConfig {
    /// اسم الحقل في قاعدة البيانات وكود Rust (snake_case)
    pub name: String,

    /// نوع الحقل
    #[serde(rename = "type")]
    pub field_type: FieldType,

    /// التسمية المعروضة للمستخدم (عربي)
    pub label: String,

    /// هل هذا الحقل مفتاح أساسي؟
    #[serde(default)]
    pub primary_key: bool,

    /// هل يُولَّد تلقائيًا (مثل UUID الافتراضي أو created_at)؟
    #[serde(default)]
    pub auto_generate: bool,

    /// هل الحقل مطلوب (NOT NULL)؟
    #[serde(default)]
    pub required: bool,

    /// هل يُسمح بقيمة فارغة (NULL)؟
    #[serde(default)]
    pub nullable: bool,

    /// الطول الأقصى (للنصوص فقط)
    pub max_length: Option<u32>,

    /// الدقة (للأرقام العشرية)
    pub precision: Option<u32>,

    /// المنازل العشرية (للأرقام العشرية)
    pub scale: Option<u32>,

    /// قيمة افتراضية (SQL expression)
    pub default_value: Option<String>,

    /// هل يظهر في جدول القائمة؟
    #[serde(default = "default_true")]
    pub display_in_list: bool,

    /// هل يظهر في نموذج الإنشاء/التعديل؟
    #[serde(default = "default_true")]
    pub display_in_form: bool,

    /// هل يظهر في صفحة التفاصيل؟
    #[serde(default = "default_true")]
    pub display_in_detail: bool,

    /// هل الحقل قابل للبحث؟
    #[serde(default)]
    pub searchable: bool,

    /// هل الحقل قابل للترتيب؟
    #[serde(default)]
    pub sortable: bool,

    /// إذا كان نوعه Reference، اسم الكيان المُشار إليه
    pub references: Option<String>,

    /// وصف اختياري للحقل (يُستخدم في تعليقات الكود المُولَّد)
    pub description: Option<String>,
}

fn default_true() -> bool { true }

impl FieldConfig {
    /// إنشاء حقل UUID كمفتاح أساسي (افتراضي لكل كيان)
    pub fn default_primary_key() -> Self {
        Self {
            name: "id".to_string(),
            field_type: FieldType::Uuid,
            label: "المعرّف".to_string(),
            primary_key: true,
            auto_generate: true,
            required: true,
            nullable: false,
            max_length: None,
            precision: None,
            scale: None,
            default_value: Some("gen_random_uuid()".to_string()),
            display_in_list: false,
            display_in_form: false,
            display_in_detail: true,
            searchable: false,
            sortable: true,
            references: None,
            description: Some("UUID v4 — يُولَّد تلقائيًا بواسطة PostgreSQL".to_string()),
        }
    }

    /// إنشاء حقل created_at (طابع زمني للإنشاء)
    pub fn created_at() -> Self {
        Self {
            name: "created_at".to_string(),
            field_type: FieldType::Timestamp,
            label: "تاريخ الإنشاء".to_string(),
            primary_key: false,
            auto_generate: true,
            required: true,
            nullable: false,
            max_length: None,
            precision: None,
            scale: None,
            default_value: Some("NOW()".to_string()),
            display_in_list: true,
            display_in_form: false,
            display_in_detail: true,
            searchable: false,
            sortable: true,
            references: None,
            description: Some("تاريخ ووقت إنشاء السجل — يُولَّد تلقائيًا".to_string()),
        }
    }

    /// إنشاء حقل updated_at (طابع زمني للتحديث)
    pub fn updated_at() -> Self {
        Self {
            name: "updated_at".to_string(),
            field_type: FieldType::Timestamp,
            label: "آخر تحديث".to_string(),
            primary_key: false,
            auto_generate: true,
            required: true,
            nullable: false,
            max_length: None,
            precision: None,
            scale: None,
            default_value: Some("NOW()".to_string()),
            display_in_list: false,
            display_in_form: false,
            display_in_detail: true,
            searchable: false,
            sortable: true,
            references: None,
            description: Some("تاريخ ووقت آخر تحديث — يُحدَّث تلقائيًا".to_string()),
        }
    }

    /// هل هذا الحقل مفتاح أساسي؟
    pub fn is_primary_key(&self) -> bool {
        self.primary_key
    }

    /// هل هذا الحقل مرجع لكيان آخر؟
    pub fn is_reference(&self) -> bool {
        self.field_type == FieldType::Reference && self.references.is_some()
    }

    /// توليد SQL column definition
    pub fn sql_column_definition(&self) -> String {
        let mut sql = format!("{} {}", self.name, self.field_type.sql_type());

        // VARCHAR length
        if self.field_type == FieldType::String {
            if let Some(max_len) = self.max_length {
                sql = format!("{}({})", sql, max_len);
            } else {
                sql = format!("{}(255)", sql);
            }
        }

        // DECIMAL precision + scale
        if self.field_type == FieldType::Decimal {
            let p = self.precision.unwrap_or(10);
            let s = self.scale.unwrap_or(2);
            sql = format!("{}({}, {})", sql, p, s);
        }

        // NOT NULL
        if self.required && !self.nullable {
            sql.push_str(" NOT NULL");
        }

        // PRIMARY KEY
        if self.primary_key {
            sql.push_str(" PRIMARY KEY");
        }

        // DEFAULT
        if let Some(default) = &self.default_value {
            if self.auto_generate {
                sql.push_str(&format!(" DEFAULT {}", default));
            }
        }

        // REFERENCES (FK)
        if self.is_reference() {
            if let Some(ref_entity) = &self.references {
                let ref_table = to_snake_case_plural(ref_entity);
                sql.push_str(&format!(" REFERENCES {}(id) ON DELETE CASCADE", ref_table));
            }
        }

        sql
    }
}

/// تحويل اسم الكيان إلى snake_case جمع (Product → products)
pub fn to_snake_case_plural(name: &str) -> String {
    let singular = to_snake_case(name);
    // قواعد جمع بسيطة بالإنجليزية
    if singular.ends_with('s') || singular.ends_with("sh") || singular.ends_with("ch") || singular.ends_with('x') || singular.ends_with('z') {
        format!("{}es", singular)
    } else if singular.ends_with('y') && singular.len() > 1 {
        let chars: Vec<char> = singular.chars().collect();
        let before_y = chars[chars.len() - 2];
        if !"aeiou".contains(before_y) {
            format!("{}ies", &singular[..singular.len() - 1])
        } else {
            format!("{}s", singular)
        }
    } else {
        format!("{}s", singular)
    }
}

/// تحويل إلى snake_case (Product → product, UserProfile → user_profile)
pub fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

/// تحويل إلى PascalCase (product → Product, user_profile → UserProfile)
pub fn to_pascal_case(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_case_plural() {
        assert_eq!(to_snake_case_plural("Product"), "products");
        assert_eq!(to_snake_case_plural("Category"), "categories");
        assert_eq!(to_snake_case_plural("Box"), "boxes");
        assert_eq!(to_snake_case_plural("User"), "users");
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(to_pascal_case("product"), "Product");
        assert_eq!(to_pascal_case("user_profile"), "UserProfile");
    }

    #[test]
    fn test_sql_column_definition() {
        let field = FieldConfig {
            name: "title".to_string(),
            field_type: FieldType::String,
            label: "العنوان".to_string(),
            primary_key: false,
            auto_generate: false,
            required: true,
            nullable: false,
            max_length: Some(255),
            precision: None,
            scale: None,
            default_value: None,
            display_in_list: true,
            display_in_form: true,
            display_in_detail: true,
            searchable: true,
            sortable: true,
            references: None,
            description: None,
        };
        assert_eq!(field.sql_column_definition(), "title VARCHAR(255) NOT NULL");
    }
}
