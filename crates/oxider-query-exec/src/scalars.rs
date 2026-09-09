//! Binding the value kinds that cross the AST as text.
//!
//! [`Value`](oxider_query_core::Value) carries decimals, UUIDs and JSON as
//! canonical text so the core crate depends on no decimal, UUID or JSON library.
//! A backend that wants to hand its driver a real typed value has to read that
//! text back first, and these macros are that step, shared so PostgreSQL and
//! MySQL do not each spell it out.
//!
//! # The rule
//!
//! **A value binds as the engine's own type where the engine has one, and as
//! text where it does not.**
//!
//! | | PostgreSQL | MySQL | SQLite |
//! |---|---|---|---|
//! | decimal | `NUMERIC` | `DECIMAL` | text |
//! | UUID | `uuid` | text | text |
//! | JSON | `jsonb` | `JSON` | text |
//!
//! UUID on MySQL is the one that looks inconsistent and is not. MySQL has no
//! UUID type, so a column is either `CHAR(36)` or `BINARY(16)`, and sqlx's
//! `Uuid` encodes as the binary form. Picking that would silently write
//! unreadable bytes into every `CHAR(36)` column. Text is the form
//! [`Value::Uuid`](oxider_query_core::Value::Uuid) already holds and the one a
//! human reading the table expects; a `BINARY(16)` column wants
//! `Uuid::as_bytes` bound as a blob, which is the caller's own choice to make.
//!
//! Where a feature is off, or the text does not parse, the value binds as text
//! and the engine decides. Text that does not parse can only come from a `Value`
//! this crate did not produce.

/// Bind an exact decimal, natively where the driver has the type.
#[cfg(feature = "rust_decimal")]
macro_rules! bind_decimal {
    ($query:expr, $text:expr) => {
        match $text.parse::<rust_decimal::Decimal>() {
            Ok(value) => $query.bind(value),
            Err(_) => $query.bind($text.as_str()),
        }
    };
}

#[cfg(not(feature = "rust_decimal"))]
macro_rules! bind_decimal {
    ($query:expr, $text:expr) => {
        $query.bind($text.as_str())
    };
}

/// Bind a UUID as the engine's own type. Only PostgreSQL has one.
#[cfg(feature = "uuid")]
macro_rules! bind_uuid_native {
    ($query:expr, $text:expr) => {
        match $text.parse::<uuid::Uuid>() {
            Ok(value) => $query.bind(value),
            Err(_) => $query.bind($text.as_str()),
        }
    };
}

#[cfg(not(feature = "uuid"))]
macro_rules! bind_uuid_native {
    ($query:expr, $text:expr) => {
        $query.bind($text.as_str())
    };
}

/// Bind a JSON document, natively where the driver has the type.
#[cfg(feature = "json")]
macro_rules! bind_json {
    ($query:expr, $text:expr) => {
        match serde_json::from_str::<serde_json::Value>($text) {
            Ok(value) => $query.bind(value),
            Err(_) => $query.bind($text.as_str()),
        }
    };
}

#[cfg(not(feature = "json"))]
macro_rules! bind_json {
    ($query:expr, $text:expr) => {
        $query.bind($text.as_str())
    };
}

pub(crate) use {bind_decimal, bind_json, bind_uuid_native};
