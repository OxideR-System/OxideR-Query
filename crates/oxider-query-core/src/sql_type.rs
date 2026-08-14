//! SQL type markers and the Rust-to-SQL type mapping.
//!
//! SQL types are represented as zero-sized marker types. They exist only at the
//! type level to make comparisons type-safe (you cannot compare a `Text` column
//! against an `Integer` literal). The [`HasSqlType`] trait maps concrete Rust
//! types to their marker so the derive macro never needs to know the mapping.

/// Marker trait implemented by every SQL type marker.
pub trait SqlType {}

macro_rules! sql_types {
    ($($doc:literal $name:ident),* $(,)?) => {
        $(
            #[doc = $doc]
            pub struct $name;
            impl SqlType for $name {}
        )*
    };
}

sql_types! {
    "Signed integer SQL type (SMALLINT/INT/BIGINT)." Integer,
    "Textual SQL type (VARCHAR/TEXT)." Text,
    "Boolean SQL type." Bool,
    "Floating point SQL type (REAL/DOUBLE)." Real,
}

/// Maps a Rust type to its SQL type marker.
///
/// The derive macro emits `Column<<FieldType as HasSqlType>::Sql>` for every
/// field, delegating the mapping here. `Option<T>` maps to the same SQL type as
/// `T`; nullability is tracked separately (see [`crate::column::Column`]).
pub trait HasSqlType {
    /// The SQL type marker for this Rust type.
    type Sql: SqlType;
}

impl HasSqlType for i16 {
    type Sql = Integer;
}
impl HasSqlType for i32 {
    type Sql = Integer;
}
impl HasSqlType for i64 {
    type Sql = Integer;
}
impl HasSqlType for String {
    type Sql = Text;
}
impl HasSqlType for bool {
    type Sql = Bool;
}
impl HasSqlType for f32 {
    type Sql = Real;
}
impl HasSqlType for f64 {
    type Sql = Real;
}
impl<T: HasSqlType> HasSqlType for Option<T> {
    type Sql = T::Sql;
}
