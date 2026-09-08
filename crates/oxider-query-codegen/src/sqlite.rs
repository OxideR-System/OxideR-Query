//! SQLite schema introspection.
//!
//! Reads table and column metadata from `sqlite_master` and `PRAGMA
//! table_info`, then emits `#[derive(Entity)]` structs. SQLite's declared types
//! are mapped by type affinity to Rust types; a column that is not `NOT NULL`
//! becomes `Option<_>`.

use crate::identifier::{escape_rust_string, field_name, type_name};
use sqlx::{Row, SqlitePool};
use std::fmt::Write;

/// Generate entity structs for every user table in the connected database,
/// ordered by table name and separated by a blank line.
pub async fn generate_entities(pool: &SqlitePool) -> Result<String, sqlx::Error> {
    let tables: Vec<String> = sqlx::query(
        "SELECT name FROM sqlite_master \
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|row| row.get::<String, _>("name"))
    .collect();

    let mut out = String::new();
    for (i, table) in tables.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&generate_struct(pool, table).await?);
    }
    Ok(out)
}

/// Emit one struct for `table`.
async fn generate_struct(pool: &SqlitePool, table: &str) -> Result<String, sqlx::Error> {
    // The table-valued form of the pragma, so the name binds as a parameter.
    // The statement form (`PRAGMA table_info("...")`) takes no parameters and
    // would have to be built by interpolation, which a table name containing a
    // double quote escapes out of.
    let columns = sqlx::query("SELECT * FROM pragma_table_info(?)")
        .bind(table)
        .fetch_all(pool)
        .await?;

    let mut fields = String::new();
    for column in &columns {
        let name: String = column.get("name");
        let declared: String = column.get("type");
        let not_null: i64 = column.get("notnull");
        let primary_key: i64 = column.get("pk");

        // A rowid `INTEGER PRIMARY KEY` reports notnull = 0 but can never be
        // null, so treat any primary-key column as non-null.
        let mut rust_type = map_type(&declared).to_string();
        if not_null == 0 && primary_key == 0 {
            rust_type = format!("Option<{rust_type}>");
        }

        // The column name may not be a legal Rust identifier. When escaping it
        // changes the name, the attribute carries the real one back - as a
        // string literal, so it is escaped for that context rather than spliced.
        let field = field_name(&name);
        if let Some(column_name) = field.rename {
            let _ = writeln!(
                fields,
                "    #[oxider(column = \"{}\")]",
                escape_rust_string(&column_name)
            );
        }
        let _ = writeln!(fields, "    pub {}: {rust_type},", field.ident);
    }

    let mut out = String::new();
    let _ = write!(
        out,
        "#[derive(Entity)]\n#[oxider(table = \"{}\")]\npub struct {} {{\n{fields}}}\n",
        escape_rust_string(table),
        type_name(table)
    );
    Ok(out)
}

/// Map a SQLite declared type to a Rust type using SQLite's type-affinity rules.
fn map_type(declared: &str) -> &'static str {
    let t = declared.to_uppercase();
    if t.contains("INT") {
        "i64"
    } else if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") {
        "String"
    } else if t.contains("BOOL") {
        "bool"
    } else if t.contains("REAL") || t.contains("FLOA") || t.contains("DOUB") {
        "f64"
    } else if t.contains("BLOB") || t.is_empty() {
        "Vec<u8>"
    } else {
        // NUMERIC / DECIMAL / DATE and anything unrecognized: keep it as text and
        // let the caller refine. Safer than guessing a lossy numeric type.
        "String"
    }
}
