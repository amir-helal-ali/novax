//! بناء مشروع كامل من ProjectConfig
//!
//! يجمع كل ملفات Rust + HTML + SQL + CSS في هيكل مشروع مستقل.

use std::collections::HashMap;

use novax_core::ProjectConfig;

use crate::{rust_codegen, html_codegen, sql_codegen, CompilerError};

/// ملف واحد في المشروع المُصدَّر
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

/// بناء مشروع كامل — يُرجع قائمة بكل الملفات
pub fn build_project(project: &ProjectConfig) -> Result<Vec<GeneratedFile>, CompilerError> {
    // التحقق من صحة المشروع
    project.validate().map_err(|e| CompilerError::Validation(e.to_string()))?;

    let mut files = Vec::new();

    // ─── Cargo.toml ───
    files.push(GeneratedFile {
        path: "Cargo.toml".to_string(),
        content: rust_codegen::generate_cargo_toml(project),
    });

    // ─── src/main.rs ───
    files.push(GeneratedFile {
        path: "src/main.rs".to_string(),
        content: rust_codegen::generate_main(project),
    });

    // ─── src/config.rs ───
    files.push(GeneratedFile {
        path: "src/config.rs".to_string(),
        content: rust_codegen::generate_config(project),
    });

    // ─── src/db.rs ───
    files.push(GeneratedFile {
        path: "src/db.rs".to_string(),
        content: rust_codegen::generate_db(),
    });

    // ─── لكل كيان: models + routes + templates + migration ───
    for entity in &project.entities {
        let snake = novax_core::to_snake_case(&entity.name);

        // models/{entity}.rs
        files.push(GeneratedFile {
            path: format!("src/models/{}.rs", snake),
            content: rust_codegen::generate_model(entity),
        });

        // routes/{entity}.rs
        files.push(GeneratedFile {
            path: format!("src/routes/{}.rs", snake),
            content: rust_codegen::generate_routes(entity),
        });

        // templates/{entity}_list.html
        files.push(GeneratedFile {
            path: format!("templates/{}_list.html", snake),
            content: html_codegen::generate_list_template(entity),
        });

        // templates/{entity}_form.html
        files.push(GeneratedFile {
            path: format!("templates/{}_form.html", snake),
            content: html_codegen::generate_form_template(entity),
        });

        // templates/{entity}_detail.html
        files.push(GeneratedFile {
            path: format!("templates/{}_detail.html", snake),
            content: html_codegen::generate_detail_template(entity),
        });

        // templates/partials/{entity}_row.html
        files.push(GeneratedFile {
            path: format!("templates/partials/{}_row.html", snake),
            content: html_codegen::generate_row_partial(entity),
        });

        // migrations/{N}_create_{table}.sql
        let migrations = sql_codegen::generate_all_migrations(&project.entities);
        for (filename, content) in migrations {
            files.push(GeneratedFile {
                path: format!("migrations/{}", filename),
                content,
            });
        }
    }

    // ─── templates/layout.html ───
    files.push(GeneratedFile {
        path: "templates/layout.html".to_string(),
        content: html_codegen::generate_layout_template(project),
    });

    // ─── static/styles.css ───
    files.push(GeneratedFile {
        path: "static/styles.css".to_string(),
        content: project.theme.to_css(),
    });

    // ─── .env.example ───
    files.push(GeneratedFile {
        path: ".env.example".to_string(),
        content: format!(
            r#"DATABASE_URL=postgres://user:password@localhost:5432/{db}
BIND_ADDR=0.0.0.0:3000
"#,
            db = project.dir_name(),
        ),
    });

    // ─── README.md ───
    files.push(GeneratedFile {
        path: "README.md".to_string(),
        content: generate_readme(project),
    });

    Ok(files)
}

fn generate_readme(project: &ProjectConfig) -> String {
    let entity_list: String = project.entities.iter()
        .map(|e| format!("- **{}** ({}): {}", e.name, e.display_name, e.fields.len()))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"# {name}

{description}

تم توليده بواسطة **Novax Engine** — منشئ التطبيقات الموجّه بالنيّة.

## الكيانات

{entities}

## التشغيل

```bash
# 1. إعداد قاعدة البيانات
createdb {db_name}

# 2. نسخ ملف البيئة
cp .env.example .env
# عدّل DATABASE_URL في .env

# 3. تشغيل التطبيق (يطبّق migrations تلقائيًا)
cargo run --release
```

افتح المتصفح على: http://localhost:3000

## التقنيات

- **Rust** + **Axum** (backend)
- **SQLx** (ORM مع فحص في وقت الترجمة)
- **Askama** (قوالب HTML مُصيَّرة من الخادم)
- **HTMX** (تحديثات جزئية بدون JavaScript)
- **Alpine.js** (تفاعل بسيط من العميل)
- **PostgreSQL** (قاعدة البيانات)

## الأمان

- ✅ SQLx يمنع SQL injection في وقت الترجمة
- ✅ كل المدخلات تُطهَّر تلقائيًا
- ✅ CSRF protection افتراضيًا
- ✅ Secure headers

---
هذا المشروع **مستقل تمامًا** عن Novax. يمكن حذف Novax بعد التصدير.
"#,
        name = project.display_name,
        description = project.description.as_deref().unwrap_or("مشروع ويب متكامل"),
        entities = entity_list,
        db_name = project.dir_name(),
    )
}
