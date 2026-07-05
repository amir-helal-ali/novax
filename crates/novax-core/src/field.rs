//! تعريف أنواع الحقول المدعومة في Novax
//!
//! كل نوع حقل يُترجم إلى:
//! - Rust type (في models.rs)
//! - SQL type (في CREATE TABLE)
//! - HTML input type (في form.html)
//! - Axum extractor (في routes.rs)

use serde::{Deserialize, Serialize};

/// أنواع الحقول المدعومة
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// UUID (مفتاح أساسي افتراضي)
    Uuid,
    /// نص قصير (VARCHAR)
    String,
    /// نص طويل (TEXT)
    Text,
    /// عدد صحيح (INTEGER / BIGINT)
    Integer,
    /// رقم عشري (DECIMAL)
    Decimal,
    /// قيمة منطقية (BOOLEAN)
    Boolean,
    /// تاريخ ووقت (TIMESTAMPTZ)
    Timestamp,
    /// تاريخ فقط (DATE)
    Date,
    /// JSON (JSONB في PostgreSQL)
    Json,
    /// مرجع لكيان آخر (UUID FK)
    Reference,
}

impl FieldType {
    /// ترجمة نوع الحقل إلى Rust type
    pub fn rust_type(&self) -> &'static str {
        match self {
            Self::Uuid => "Uuid",
            Self::String => "String",
            Self::Text => "String",
            Self::Integer => "i64",
            Self::Decimal => "Decimal",
            Self::Boolean => "bool",
            Self::Timestamp => "DateTime<Utc>",
            Self::Date => "NaiveDate",
            Self::Json => "serde_json::Value",
            Self::Reference => "Uuid",
        }
    }

    /// ترجمة نوع الحقل إلى SQL type
    pub fn sql_type(&self) -> &'static str {
        match self {
            Self::Uuid => "UUID",
            Self::String => "VARCHAR",
            Self::Text => "TEXT",
            Self::Integer => "BIGINT",
            Self::Decimal => "DECIMAL",
            Self::Boolean => "BOOLEAN",
            Self::Timestamp => "TIMESTAMPTZ",
            Self::Date => "DATE",
            Self::Json => "JSONB",
            Self::Reference => "UUID",
        }
    }

    /// ترجمة نوع الحقل إلى HTML input type
    pub fn html_input_type(&self) -> &'static str {
        match self {
            Self::Uuid => "hidden",
            Self::String => "text",
            Self::Text => "textarea",
            Self::Integer => "number",
            Self::Decimal => "number",
            Self::Boolean => "checkbox",
            Self::Timestamp => "datetime-local",
            Self::Date => "date",
            Self::Json => "textarea",
            Self::Reference => "select",
        }
    }

    /// هل هذا النوع قابل للترتيب؟ (ORDER BY)
    pub fn is_sortable(&self) -> bool {
        matches!(self, Self::Uuid | Self::String | Self::Integer | Self::Decimal | Self::Timestamp | Self::Date)
    }

    /// هل هذا النوع قابل للبحث النصي؟ (ILIKE)
    pub fn is_searchable(&self) -> bool {
        matches!(self, Self::String | Self::Text)
    }
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap_or_default().trim_matches('"'))
    }
}

impl std::str::FromStr for FieldType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "uuid" => Ok(Self::Uuid),
            "string" | "varchar" => Ok(Self::String),
            "text" => Ok(Self::Text),
            "integer" | "int" | "bigint" => Ok(Self::Integer),
            "decimal" | "numeric" => Ok(Self::Decimal),
            "boolean" | "bool" => Ok(Self::Boolean),
            "timestamp" | "datetime" => Ok(Self::Timestamp),
            "date" => Ok(Self::Date),
            "json" | "jsonb" => Ok(Self::Json),
            "reference" | "fk" | "relation" => Ok(Self::Reference),
            other => Err(format!("نوع حقل غير مدعوم: {}", other)),
        }
    }
}
