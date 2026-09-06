//! How `#[derive(Entity)]` maps a Rust struct onto a table.
//!
//! These pin the derive's own decisions - the default table name, the schema
//! qualifier, renamed and skipped fields, nullability, and fields whose names
//! collide with Rust keywords - by rendering a query that uses them, because
//! the generated SQL is the only thing a user actually observes.

// The entity structs exist to be derived on; their fields are read through the
// generated metamodel, not directly.
#![allow(dead_code)]

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::Postgres;

/// No `table` attribute: the table name falls back to the lowercased struct
/// name, which is what a straightforward schema will already be called.
#[derive(Entity)]
struct Invoice {
    id: i64,
    amount: f64,
}

/// A struct whose fields are named after Rust keywords. The schema is free to
/// use `type`, `match` and `ref` as column names, so the metamodel has to keep
/// working when the Rust side has to write them as raw identifiers.
#[derive(Entity)]
#[oxider(table = "events")]
struct Event {
    id: i64,
    r#type: String,
    r#match: Option<String>,
    r#ref: String,
}

/// Renames, skips, and nullability in one place.
#[derive(Entity)]
#[oxider(table = "people", schema = "hr")]
struct Person {
    id: i64,
    #[oxider(column = "full_name")]
    name: String,
    nickname: Option<String>,
    /// Not a column: computed after loading.
    #[oxider(skip)]
    #[allow(dead_code)]
    display: String,
}

/// Exactly what `oxider-query-codegen` emits for a table whose column names
/// are not legal Rust identifiers. Keeping it here proves the two halves agree:
/// the generator escapes the name, the derive maps it back.
#[derive(Entity)]
#[oxider(table = "metrics")]
struct Metric {
    id: i64,
    #[oxider(column = "total-count")]
    total_count: i64,
    #[oxider(column = "2fa enabled")]
    _2fa_enabled: bool,
}

#[test]
fn a_struct_with_no_table_attribute_uses_its_lowercased_name() {
    assert_eq!(<Invoice as Entity>::TABLE, "invoice");
    assert_sql_only(
        Invoice::query().select((Invoice::id, Invoice::amount)),
        &Postgres,
        r#"SELECT "invoice"."id", "invoice"."amount" FROM "invoice""#,
    );
}

#[test]
fn a_field_written_as_a_raw_identifier_maps_to_the_bare_column_name() {
    // The Rust side must write `r#type`; the database column is `type`. The
    // `r#` is Rust syntax, not part of the name, and must not reach the SQL.
    assert_sql(
        Event::query()
            .select((Event::id, Event::r#type))
            .filter(Event::r#type.eq("signup").and(Event::r#ref.eq("web"))),
        &Postgres,
        r#"SELECT "events"."id", "events"."type" FROM "events" WHERE "events"."type" = $1 AND "events"."ref" = $2"#,
        &[text("signup"), text("web")],
    );
}

#[test]
fn a_nullable_raw_identifier_field_is_still_marked_nullable() {
    assert_eq!(
        (Event::r#type.nullable, Event::r#match.nullable),
        (false, true),
        "only the `Option<String>` field is nullable"
    );
    assert_sql_only(
        Event::query()
            .select(Event::id)
            .filter(Event::r#match.is_null()),
        &Postgres,
        r#"SELECT "events"."id" FROM "events" WHERE "events"."match" IS NULL"#,
    );
}

#[test]
fn a_renamed_field_uses_the_column_name_and_a_schema_qualifies_the_table() {
    assert_eq!(<Person as Entity>::SCHEMA, Some("hr"));
    assert_sql(
        Person::query()
            .select((Person::name, Person::nickname))
            .filter(Person::name.eq("ada")),
        &Postgres,
        r#"SELECT "people"."full_name", "people"."nickname" FROM "hr"."people" WHERE "people"."full_name" = $1"#,
        &[text("ada")],
    );
}

#[test]
fn an_option_field_is_nullable_and_a_plain_one_is_not() {
    assert_eq!(
        (Person::name.nullable, Person::nickname.nullable),
        (false, true),
        "only the `Option<String>` field is nullable"
    );
    // `Option<String>` keeps the string operators: the inner type is what the
    // metamodel records, so a nullable column is not second-class.
    assert_sql(
        Person::query()
            .select(Person::id)
            .filter(Person::nickname.starts_with("a")),
        &Postgres,
        r#"SELECT "people"."id" FROM "hr"."people" WHERE "people"."nickname" LIKE $1 ESCAPE '!'"#,
        &[text("a%")],
    );
}

/// A skipped field is not part of the metamodel, so it cannot be selected. The
/// proof that `Person::display` does not exist is the `compile_fail` doc test
/// on the crate root, because doc tests do not run for an integration test.
#[test]
fn a_skipped_field_is_absent_from_the_metamodel() {
    assert_sql_only(
        Person::query().select(Person::id),
        &Postgres,
        r#"SELECT "people"."id" FROM "hr"."people""#,
    );
}

#[test]
fn a_generated_rename_quotes_the_real_column_name() {
    assert_sql(
        Metric::query()
            .select((Metric::total_count, Metric::_2fa_enabled))
            .filter(Metric::_2fa_enabled.eq(true))
            .order_by(Metric::total_count.desc()),
        &Postgres,
        r#"SELECT "metrics"."total-count", "metrics"."2fa enabled" FROM "metrics" WHERE "metrics"."2fa enabled" = $1 ORDER BY "metrics"."total-count" DESC"#,
        &[flag(true)],
    );
}
