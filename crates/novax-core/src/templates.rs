//! قوالب مشاريع جاهزة (Project Templates)
//!
//! قوالب سريعة البدء: Blog, Store, SaaS

use crate::{ProjectConfig, EntityConfig, FieldConfig, FieldType};

/// قالب مدونة (Blog)
pub fn blog_template() -> ProjectConfig {
    let mut project = ProjectConfig::new("Blog", "مدونتي");

    // Post entity
    let mut post = EntityConfig::new("Post", "مقال", "مقالات");
    post.icon = "📝".to_string();
    post.description = Some("مقالات المدونة".to_string());
    post.fields.push(FieldConfig {
        name: "title".to_string(),
        field_type: FieldType::String,
        label: "العنوان".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: Some(255), precision: None, scale: None, default_value: None,
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: true, sortable: true, references: None,
        description: Some("عنوان المقال".to_string()),
    });
    post.fields.push(FieldConfig {
        name: "slug".to_string(),
        field_type: FieldType::String,
        label: "الرابط".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: Some(255), precision: None, scale: None, default_value: None,
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: false, sortable: false, references: None,
        description: Some("رابط المقال (slug)".to_string()),
    });
    post.fields.push(FieldConfig {
        name: "body".to_string(),
        field_type: FieldType::Text,
        label: "المحتوى".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: None, precision: None, scale: None, default_value: None,
        display_in_list: false, display_in_form: true, display_in_detail: true,
        searchable: true, sortable: false, references: None,
        description: Some("محتوى المقال".to_string()),
    });
    post.fields.push(FieldConfig {
        name: "is_published".to_string(),
        field_type: FieldType::Boolean,
        label: "منشور".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: None, precision: None, scale: None, default_value: Some("false".to_string()),
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: false, sortable: true, references: None,
        description: Some("هل المقال منشور؟".to_string()),
    });
    project.add_entity(post);

    // Category entity
    let mut category = EntityConfig::new("Category", "تصنيف", "تصنيفات");
    category.icon = "📁".to_string();
    category.description = Some("تصنيفات المقالات".to_string());
    category.fields.push(FieldConfig {
        name: "name".to_string(),
        field_type: FieldType::String,
        label: "الاسم".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: Some(100), precision: None, scale: None, default_value: None,
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: true, sortable: true, references: None,
        description: Some("اسم التصنيف".to_string()),
    });
    project.add_entity(category);

    project
}

/// قالب متجر (Store)
pub fn store_template() -> ProjectConfig {
    let mut project = ProjectConfig::new("Store", "متجري");
    project.description = Some("متجر إلكتروني".to_string());

    // Product entity
    let mut product = EntityConfig::new("Product", "منتج", "منتجات");
    product.icon = "📦".to_string();
    product.description = Some("منتجات المتجر".to_string());
    product.fields.push(FieldConfig {
        name: "title".to_string(),
        field_type: FieldType::String,
        label: "الاسم".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: Some(255), precision: None, scale: None, default_value: None,
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: true, sortable: true, references: None,
        description: Some("اسم المنتج".to_string()),
    });
    product.fields.push(FieldConfig {
        name: "price".to_string(),
        field_type: FieldType::Decimal,
        label: "السعر".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: None, precision: Some(10), scale: Some(2), default_value: None,
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: false, sortable: true, references: None,
        description: Some("سعر المنتج".to_string()),
    });
    product.fields.push(FieldConfig {
        name: "description".to_string(),
        field_type: FieldType::Text,
        label: "الوصف".to_string(),
        primary_key: false, auto_generate: false, required: false, nullable: true,
        max_length: None, precision: None, scale: None, default_value: None,
        display_in_list: false, display_in_form: true, display_in_detail: true,
        searchable: true, sortable: false, references: None,
        description: Some("وصف المنتج".to_string()),
    });
    product.fields.push(FieldConfig {
        name: "stock".to_string(),
        field_type: FieldType::Integer,
        label: "المخزون".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: None, precision: None, scale: None, default_value: Some("0".to_string()),
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: false, sortable: true, references: None,
        description: Some("كمية المخزون".to_string()),
    });
    product.fields.push(FieldConfig {
        name: "is_active".to_string(),
        field_type: FieldType::Boolean,
        label: "متاح".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: None, precision: None, scale: None, default_value: Some("true".to_string()),
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: false, sortable: true, references: None,
        description: Some("هل المنتج متاح للبيع؟".to_string()),
    });
    project.add_entity(product);

    // Category entity
    let mut category = EntityConfig::new("Category", "فئة", "فئات");
    category.icon = "🏷️".to_string();
    category.fields.push(FieldConfig {
        name: "name".to_string(),
        field_type: FieldType::String,
        label: "الاسم".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: Some(100), precision: None, scale: None, default_value: None,
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: true, sortable: true, references: None,
        description: Some("اسم الفئة".to_string()),
    });
    project.add_entity(category);

    project
}

/// قالب SaaS
pub fn saas_template() -> ProjectConfig {
    let mut project = ProjectConfig::new("SaaSApp", "تطبيقي");
    project.description = Some("تطبيق SaaS".to_string());

    // Organization entity
    let mut org = EntityConfig::new("Organization", "مؤسسة", "مؤسسات");
    org.icon = "🏢".to_string();
    org.fields.push(FieldConfig {
        name: "name".to_string(),
        field_type: FieldType::String,
        label: "الاسم".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: Some(255), precision: None, scale: None, default_value: None,
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: true, sortable: true, references: None,
        description: Some("اسم المؤسسة".to_string()),
    });
    org.fields.push(FieldConfig {
        name: "plan".to_string(),
        field_type: FieldType::String,
        label: "الباقة".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: Some(50), precision: None, scale: None, default_value: Some("'free'".to_string()),
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: false, sortable: true, references: None,
        description: Some("باقة الاشتراك: free, pro, enterprise".to_string()),
    });
    project.add_entity(org);

    // Task entity
    let mut task = EntityConfig::new("Task", "مهمة", "مهام");
    task.icon = "✅".to_string();
    task.fields.push(FieldConfig {
        name: "title".to_string(),
        field_type: FieldType::String,
        label: "العنوان".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: Some(255), precision: None, scale: None, default_value: None,
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: true, sortable: true, references: None,
        description: Some("عنوان المهمة".to_string()),
    });
    task.fields.push(FieldConfig {
        name: "status".to_string(),
        field_type: FieldType::String,
        label: "الحالة".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: Some(50), precision: None, scale: None, default_value: Some("'todo'".to_string()),
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: false, sortable: true, references: None,
        description: Some("todo, in_progress, done".to_string()),
    });
    task.fields.push(FieldConfig {
        name: "priority".to_string(),
        field_type: FieldType::Integer,
        label: "الأولوية".to_string(),
        primary_key: false, auto_generate: false, required: true, nullable: false,
        max_length: None, precision: None, scale: None, default_value: Some("0".to_string()),
        display_in_list: true, display_in_form: true, display_in_detail: true,
        searchable: false, sortable: true, references: None,
        description: Some("0=منخفضة, 1=متوسطة, 2=عالية".to_string()),
    });
    project.add_entity(task);

    project
}

/// الحصول على قالب بالاسم
pub fn get_template(name: &str) -> Option<ProjectConfig> {
    match name.to_lowercase().as_str() {
        "blog" => Some(blog_template()),
        "store" | "ecommerce" | "shop" => Some(store_template()),
        "saas" => Some(saas_template()),
        _ => None,
    }
}

/// قائمة كل القوالب المتاحة
pub fn list_templates() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("blog", "📝 مدونة", "مدونة بسيطة بمقالات وتصنيفات"),
        ("store", "📦 متجر", "متجر إلكتروني بمنتجات وفئات ومخزون"),
        ("saas", "✅ SaaS", "تطبيق SaaS بمؤسسات ومهام"),
    ]
}
