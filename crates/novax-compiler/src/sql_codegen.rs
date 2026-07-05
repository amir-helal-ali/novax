//! توليد ملفات SQL (CREATE TABLE + indexes) من EntityConfig

use novax_core::EntityConfig;

/// توليد ملف migration SQL لكيان واحد
pub fn generate_migration(entity: &EntityConfig) -> String {
    let table = entity.table();
    let struct_name = entity.struct_name();

    let columns: String = entity.fields.iter()
        .map(|f| format!("    {}", f.sql_column_definition()))
        .collect::<Vec<_>>()
        .join(",\n");

    // indexes على الحقول القابلة للبحث والترتيب
    let indexes: String = entity.fields.iter()
        .filter(|f| f.searchable || f.sortable)
        .filter(|f| !f.primary_key)
        .map(|f| {
            let index_name = format!("idx_{}_{}", table, f.name);
            if f.field_type.is_searchable() {
                format!(
                    "CREATE INDEX {} ON {} USING gin (to_tsvector('english', {}));",
                    index_name, table, f.name
                )
            } else {
                format!("CREATE INDEX {} ON {} ({});", index_name, table, f.name)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"-- Migration: {struct_name} table
-- تم توليده بواسطة Novax Engine

CREATE TABLE {table} (
{columns}
);

{indexes}
"#,
        struct_name = struct_name,
        table = table,
        columns = columns,
        indexes = if indexes.is_empty() { String::new() } else { format!("\n{}", indexes) },
    )
}

/// توليد جميع migrations لمشروع كامل
pub fn generate_all_migrations(entities: &[EntityConfig]) -> Vec<(String, String)> {
    entities.iter()
        .enumerate()
        .map(|(i, e)| {
            let filename = format!("{:03}_create_{}.sql", i + 1, e.table());
            let content = generate_migration(e);
            (filename, content)
        })
        .collect()
}
