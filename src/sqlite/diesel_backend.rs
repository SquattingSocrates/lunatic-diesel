//! The SQLite backend

use diesel::backend::SqlDialect;

use diesel::backend::*;
use diesel::sql_types::{
    BigInt, Binary, Bool, Date, Double, Float, Integer, Numeric, SmallInt, Text, Time, Timestamp,
    TypeMetadata,
};

use lunatic_sqlite_api::wire_format::SqliteValue;

use super::bind_collector::SqliteBindCollector;
use super::query_builder::SqliteQueryBuilder;

/// The SQLite backend
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
pub struct Sqlite;

/// Determines how a bind parameter is given to SQLite
///
/// Diesel deals with bind parameters after serialization as opaque blobs of
/// bytes. However, SQLite instead has several functions where it expects the
/// relevant C types.
///
/// The variants of this struct determine what bytes are expected from
/// `ToSql` impls.
#[allow(missing_debug_implementations)]
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum SqliteType {
    /// Bind using `sqlite3_bind_blob`
    Binary,
    /// Bind using `sqlite3_bind_text`
    Text,
    /// `bytes` should contain an `f32`
    Float,
    /// `bytes` should contain an `f64`
    Double,
    /// `bytes` should contain an `i16`
    SmallInt,
    /// `bytes` should contain an `i32`
    Integer,
    /// `bytes` should contain an `i64`
    Long,
}

impl Backend for Sqlite {
    type QueryBuilder = SqliteQueryBuilder;
}

impl<'a> HasBindCollector<'a> for Sqlite {
    type BindCollector = SqliteBindCollector<'a>;
}

impl<'a> HasRawValue<'a> for Sqlite {
    type RawValue = &'a SqliteValue;
}

impl TypeMetadata for Sqlite {
    type TypeMetadata = SqliteType;
    type MetadataLookup = ();
}

impl SqlDialect for Sqlite {
    // #[cfg(not(feature = "returning_clauses_for_sqlite_3_35"))]
    // type ReturningClause = sql_dialect::returning_clause::DoesNotSupportReturningClause;
    // #[cfg(feature = "returning_clauses_for_sqlite_3_35")]
    type ReturningClause = SqliteReturningClause;

    type OnConflictClause = SqliteOnConflictClause;

    type InsertWithDefaultKeyword = sql_dialect::default_keyword_for_insert::IsoSqlDefaultKeyword;
    type BatchInsertSupport = SqliteBatchInsert;
    type ConcatClause = sql_dialect::concat_clause::ConcatWithPipesClause;
    type DefaultValueClauseForInsert = sql_dialect::default_value_clause::AnsiDefaultValueClause;

    type EmptyFromClauseSyntax = sql_dialect::from_clause_syntax::AnsiSqlFromClauseSyntax;
    type SelectStatementSyntax = sql_dialect::select_statement_syntax::AnsiSqlSelectStatement;

    type ExistsSyntax = sql_dialect::exists_syntax::AnsiSqlExistsSyntax;
    type ArrayComparison = sql_dialect::array_comparison::AnsiSqlArrayComparison;
}

impl DieselReserveSpecialization for Sqlite {}
impl TrustedBackend for Sqlite {}

#[derive(Debug, Copy, Clone)]
pub struct SqliteOnConflictClause;

impl sql_dialect::on_conflict_clause::SupportsOnConflictClause for SqliteOnConflictClause {}

#[derive(Debug, Copy, Clone)]
pub struct SqliteBatchInsert;

#[derive(Debug, Copy, Clone)]
pub struct SqliteReturningClause;

impl sql_dialect::returning_clause::SupportsReturningClause for SqliteReturningClause {}

macro_rules! wrap_sqlite_type {
    ($struct_name:ident, $ty:ident) => {
        impl diesel::sql_types::HasSqlType<$struct_name> for crate::sqlite::Sqlite {
            fn metadata(_: &mut ()) -> crate::sqlite::SqliteType {
                crate::sqlite::SqliteType::$ty
            }
        }
    };
}

wrap_sqlite_type!(Float, Float);
wrap_sqlite_type!(BigInt, Long);
wrap_sqlite_type!(SmallInt, SmallInt);
wrap_sqlite_type!(Bool, Integer);
wrap_sqlite_type!(Binary, Binary);
wrap_sqlite_type!(Text, Text);
wrap_sqlite_type!(Numeric, Double);
wrap_sqlite_type!(Double, Double);
wrap_sqlite_type!(Integer, Integer);
wrap_sqlite_type!(Date, Text);
wrap_sqlite_type!(Time, Text);
wrap_sqlite_type!(Timestamp, Text);
