//! PostgreSQL schema introspection.
//!
//! Reads tables, columns, primary keys and foreign keys from the system
//! catalogs, then emits `#[derive(Entity)]` structs. Unlike SQLite, PostgreSQL
//! has real column types, so the mapping is a lookup rather than a guess at
//! affinity.
//!
//! # Which Rust type a column gets
//!
//! The types that need a feature are emitted only when this crate has that
//! feature, because a generated file that names `rust_decimal::Decimal` in a
//! project without the crate does not compile, and a struct that does not
//! compile is worse than no struct at all. Turn the same features on here that
//! you turn on for `oxider-query`, and the two agree.
//!
//! | PostgreSQL | Rust | Needs |
//! |---|---|---|
//! | `smallint`, `integer`, `bigint` | `i16`, `i32`, `i64` | |
//! | `real`, `double precision` | `f32`, `f64` | |
//! | `boolean` | `bool` | |
//! | `text`, `varchar`, `char`, `name` | `String` | |
//! | `bytea` | `Vec<u8>` | |
//! | `date`, `time`, `timestamp` | `NaiveDate`, `NaiveTime`, `NaiveDateTime` | `chrono` |
//! | `timestamptz` | `DateTime<Utc>` | `chrono` |
//! | `numeric`, `decimal` | `Decimal` | `rust_decimal` |
//! | `uuid` | `Uuid` | `uuid` |
//! | `json`, `jsonb` | `serde_json::Value` | `json` |
//!
//! Anything else - arrays, enums, ranges, PostGIS geometry, a domain over any
//! of them - becomes `String` with a doc comment naming the real type, so the
//! file still compiles and the places needing a human are visible.
//!
//! # Keys
//!
//! Primary and foreign keys are emitted as doc comments rather than attributes.
//! `#[derive(Entity)]` has nothing to do with either: it describes what a table
//! looks like, not how its rows relate, and joins are written explicitly.
//! A comment saying `user_id` references `users(id)` is what the reader needs
//! at the moment they write that join.

use crate::identifier::{escape_rust_string, field_name, type_name};
use sqlx::postgres::types::Oid;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::fmt::Write;

/// One table to generate a struct for.
struct TableRef {
    oid: Oid,
    schema: String,
    name: String,
}

/// A foreign key column and what it points at.
struct ForeignKey {
    column: String,
    ref_schema: String,
    ref_table: String,
    ref_column: String,
}

/// Generate entity structs for every table in every non-system schema, ordered
/// by schema then table and separated by a blank line.
///
/// Views, materialised views and sequences are skipped: an entity describes
/// something rows can be inserted into and updated, and a view generally is
/// not.
///
/// Use [`generate_entities_in`] to name the schemas instead. A database that
/// carries an extension's tables, or one application schema among several, is
/// the common case for it.
pub async fn generate_entities(pool: &PgPool) -> Result<String, sqlx::Error> {
    entities(pool, None).await
}

/// The same, restricted to the named schemas.
///
/// Naming a schema that does not exist is not an error: it contributes no
/// tables, exactly as an empty one would.
pub async fn generate_entities_in(pool: &PgPool, schemas: &[&str]) -> Result<String, sqlx::Error> {
    entities(pool, Some(schemas)).await
}

async fn entities(pool: &PgPool, schemas: Option<&[&str]>) -> Result<String, sqlx::Error> {
    let tables = list_tables(pool, schemas).await?;

    // A table name may repeat across schemas, and two structs with one name do
    // not compile. Where that happens - and only there - the schema goes into
    // the struct name, so the common single-schema case is unaffected.
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for table in &tables {
        *seen.entry(table.name.as_str()).or_default() += 1;
    }

    let mut out = String::new();
    for (i, table) in tables.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let ambiguous = seen[table.name.as_str()] > 1;
        out.push_str(&generate_struct(pool, table, ambiguous).await?);
    }
    Ok(out)
}

/// Every ordinary and partitioned table outside the system schemas, optionally
/// narrowed to the schemas the caller named.
///
/// The filter is one bound array rather than a list spliced into the SQL, so a
/// schema name is data here the same way a table name is everywhere else.
async fn list_tables(
    pool: &PgPool,
    schemas: Option<&[&str]>,
) -> Result<Vec<TableRef>, sqlx::Error> {
    let owned: Vec<String> = schemas
        .unwrap_or_default()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let rows = sqlx::query(
        "SELECT c.oid, n.nspname AS schema, c.relname AS name \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind IN ('r', 'p') \
           AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
           AND n.nspname NOT LIKE 'pg_toast%' \
           AND n.nspname NOT LIKE 'pg_temp%' \
           AND ($1 OR n.nspname = ANY($2)) \
         ORDER BY n.nspname, c.relname",
    )
    .bind(schemas.is_none())
    .bind(&owned)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| TableRef {
            oid: row.get("oid"),
            schema: row.get("schema"),
            name: row.get("name"),
        })
        .collect())
}

/// Emit one struct for `table`.
async fn generate_struct(
    pool: &PgPool,
    table: &TableRef,
    qualify_name: bool,
) -> Result<String, sqlx::Error> {
    let primary_key = primary_key(pool, table.oid).await?;
    let foreign_keys = foreign_keys(pool, table.oid).await?;

    // Every column of the table, in the order the table declares them, so the
    // struct reads the way `\d table` does.
    let columns = sqlx::query(
        "SELECT a.attname AS name, \
                format_type(a.atttypid, a.atttypmod) AS sql_type, \
                a.attnotnull AS not_null \
         FROM pg_attribute a \
         WHERE a.attrelid = $1 AND a.attnum > 0 AND NOT a.attisdropped \
         ORDER BY a.attnum",
    )
    .bind(table.oid)
    .fetch_all(pool)
    .await?;

    let mut fields = String::new();
    for column in &columns {
        let name: String = column.get("name");
        let sql_type: String = column.get("sql_type");
        let not_null: bool = column.get("not_null");

        let mapped = map_type(&sql_type);
        let rust_type = if not_null {
            mapped.rust.to_string()
        } else {
            format!("Option<{}>", mapped.rust)
        };

        // What the reader cannot see from the Rust type alone: which column is
        // the key, where a reference points, and when the type is a stand-in.
        if primary_key.contains(&name) {
            let _ = writeln!(fields, "    /// Primary key.");
        }
        for fk in foreign_keys.iter().filter(|fk| fk.column == name) {
            let _ = writeln!(
                fields,
                "    /// References `{}`.`{}`.",
                qualified(&fk.ref_schema, &fk.ref_table),
                fk.ref_column
            );
        }
        if !mapped.exact {
            let _ = writeln!(
                fields,
                "    /// Column type is `{sql_type}`, which has no Rust mapping here; \
                 refine by hand.",
            );
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

    // `public` is on the search path, so naming it would add noise without
    // changing a single rendered query.
    let schema_attr = if table.schema == "public" {
        String::new()
    } else {
        format!(
            "#[oxider(schema = \"{}\")]\n",
            escape_rust_string(&table.schema)
        )
    };
    let struct_name = if qualify_name {
        format!("{}{}", type_name(&table.schema), type_name(&table.name))
    } else {
        type_name(&table.name)
    };

    let mut out = String::new();
    let _ = write!(
        out,
        "#[derive(Entity)]\n#[oxider(table = \"{}\")]\n{schema_attr}pub struct {struct_name} {{\n{fields}}}\n",
        escape_rust_string(&table.name),
    );
    Ok(out)
}

/// The primary-key column names, in key order.
async fn primary_key(pool: &PgPool, table: Oid) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT a.attname AS name \
         FROM pg_index i \
         JOIN unnest(i.indkey) WITH ORDINALITY AS k(attnum, ord) ON true \
         JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = k.attnum \
         WHERE i.indrelid = $1 AND i.indisprimary \
         ORDER BY k.ord",
    )
    .bind(table)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(|row| row.get("name")).collect())
}

/// Every foreign-key column of the table and the column it references.
///
/// A composite key produces one row per column pair, matched by position within
/// the constraint, so a two-column reference documents both halves rather than
/// pairing the first column with the second table's second key column.
async fn foreign_keys(pool: &PgPool, table: Oid) -> Result<Vec<ForeignKey>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT att.attname AS column, \
                fn.nspname AS ref_schema, \
                fc.relname AS ref_table, \
                fatt.attname AS ref_column \
         FROM pg_constraint con \
         JOIN unnest(con.conkey) WITH ORDINALITY AS k(attnum, ord) ON true \
         JOIN unnest(con.confkey) WITH ORDINALITY AS f(attnum, ord) ON f.ord = k.ord \
         JOIN pg_attribute att ON att.attrelid = con.conrelid AND att.attnum = k.attnum \
         JOIN pg_class fc ON fc.oid = con.confrelid \
         JOIN pg_namespace fn ON fn.oid = fc.relnamespace \
         JOIN pg_attribute fatt ON fatt.attrelid = con.confrelid AND fatt.attnum = f.attnum \
         WHERE con.conrelid = $1 AND con.contype = 'f' \
         ORDER BY con.conname, k.ord",
    )
    .bind(table)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| ForeignKey {
            column: row.get("column"),
            ref_schema: row.get("ref_schema"),
            ref_table: row.get("ref_table"),
            ref_column: row.get("ref_column"),
        })
        .collect())
}

/// Write a table the way a reader would say it: bare in `public`, qualified
/// anywhere else.
fn qualified(schema: &str, table: &str) -> String {
    if schema == "public" {
        table.to_string()
    } else {
        format!("{schema}.{table}")
    }
}

/// A Rust type for a column, and whether it really is that type.
struct Mapped {
    rust: &'static str,
    /// False when the type is a stand-in for something with no mapping, so the
    /// caller can say so in a comment instead of letting it pass as exact.
    exact: bool,
}

const fn exact(rust: &'static str) -> Mapped {
    Mapped { rust, exact: true }
}

/// Map a PostgreSQL type name, as `format_type` spells it, to a Rust type.
fn map_type(sql_type: &str) -> Mapped {
    // format_type writes the modifier into the name: `character varying(40)`,
    // `numeric(10,2)`, `timestamp(3) with time zone`. The length never changes
    // which Rust type fits, so it is cut away before matching.
    let base = strip_modifier(sql_type);

    match base.as_str() {
        "smallint" | "smallserial" => exact("i16"),
        "integer" | "serial" => exact("i32"),
        "bigint" | "bigserial" => exact("i64"),
        "real" => exact("f32"),
        "double precision" => exact("f64"),
        "boolean" => exact("bool"),
        "text" | "character varying" | "character" | "name" | "citext" => exact("String"),
        "bytea" => exact("Vec<u8>"),

        #[cfg(feature = "chrono")]
        "date" => exact("chrono::NaiveDate"),
        #[cfg(feature = "chrono")]
        "time without time zone" => exact("chrono::NaiveTime"),
        #[cfg(feature = "chrono")]
        "timestamp without time zone" => exact("chrono::NaiveDateTime"),
        #[cfg(feature = "chrono")]
        "timestamp with time zone" => exact("chrono::DateTime<chrono::Utc>"),

        #[cfg(feature = "rust_decimal")]
        "numeric" => exact("rust_decimal::Decimal"),
        #[cfg(feature = "uuid")]
        "uuid" => exact("uuid::Uuid"),
        #[cfg(feature = "json")]
        "json" | "jsonb" => exact("serde_json::Value"),

        // Arrays, enums, ranges, domains, geometry, and the types above when
        // their feature is off. Text keeps the file compiling and the comment
        // keeps it honest about what is left to do.
        _ => Mapped {
            rust: "String",
            exact: false,
        },
    }
}

/// Drop a type modifier: `numeric(10,2)` is `numeric`, and `timestamp(3) with
/// time zone` is `timestamp with time zone`.
fn strip_modifier(sql_type: &str) -> String {
    let Some(open) = sql_type.find('(') else {
        return sql_type.to_string();
    };
    let Some(close) = sql_type[open..].find(')') else {
        return sql_type.to_string();
    };
    let mut out = String::with_capacity(sql_type.len());
    out.push_str(sql_type[..open].trim_end());
    out.push_str(&sql_type[open + close + 1..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_modifier_does_not_change_which_rust_type_fits() {
        assert_eq!(strip_modifier("character varying(40)"), "character varying");
        assert_eq!(strip_modifier("numeric(10,2)"), "numeric");
        assert_eq!(
            strip_modifier("timestamp(3) with time zone"),
            "timestamp with time zone"
        );
        assert_eq!(strip_modifier("text"), "text");
    }

    #[test]
    fn a_type_with_no_mapping_falls_back_to_text_and_says_so() {
        let mapped = map_type("text[]");
        assert_eq!(mapped.rust, "String");
        assert!(!mapped.exact, "an array is not really a String");
    }

    #[test]
    fn the_plain_types_map_without_a_feature() {
        assert_eq!(map_type("bigint").rust, "i64");
        assert_eq!(map_type("character varying(255)").rust, "String");
        assert_eq!(map_type("bytea").rust, "Vec<u8>");
        assert!(map_type("boolean").exact);
    }
}
