use diesel::query_builder::BindCollector;
use diesel::serialize::{IsNull, Output};
use diesel::sql_types::HasSqlType;
use diesel::QueryResult;
use lunatic_sqlite_api::wire_format::{BindKey, BindPair, BindValue};

use super::diesel_backend::{Sqlite, SqliteType};

#[derive(Debug)]
pub struct SqliteBindCollector<'a> {
    pub(in crate::sqlite) binds: Vec<(InternalSqliteBindValue<'a>, SqliteType)>,
}

impl SqliteBindCollector<'_> {
    pub(in crate::sqlite) fn new() -> Self {
        Self { binds: Vec::new() }
    }
}

/// This type represents a value bound to
/// an sqlite prepared statement
///
/// It can be constructed via the various `From<T>` implementations
#[derive(Debug)]
pub struct SqliteBindValue<'a> {
    pub(in crate::sqlite) inner: InternalSqliteBindValue<'a>,
}

impl<'a> From<i32> for SqliteBindValue<'a> {
    fn from(i: i32) -> Self {
        Self {
            inner: InternalSqliteBindValue::I32(i),
        }
    }
}

impl<'a> From<i64> for SqliteBindValue<'a> {
    fn from(i: i64) -> Self {
        Self {
            inner: InternalSqliteBindValue::I64(i),
        }
    }
}

impl<'a> From<f64> for SqliteBindValue<'a> {
    fn from(f: f64) -> Self {
        Self {
            inner: InternalSqliteBindValue::F64(f),
        }
    }
}

impl<'a, T> From<Option<T>> for SqliteBindValue<'a>
where
    T: Into<SqliteBindValue<'a>>,
{
    fn from(o: Option<T>) -> Self {
        match o {
            Some(v) => v.into(),
            None => Self {
                inner: InternalSqliteBindValue::Null,
            },
        }
    }
}

impl<'a> From<&'a str> for SqliteBindValue<'a> {
    fn from(s: &'a str) -> Self {
        Self {
            inner: InternalSqliteBindValue::BorrowedString(s),
        }
    }
}

impl<'a> From<String> for SqliteBindValue<'a> {
    fn from(s: String) -> Self {
        Self {
            inner: InternalSqliteBindValue::String(s.into_boxed_str()),
        }
    }
}

impl<'a> From<Vec<u8>> for SqliteBindValue<'a> {
    fn from(b: Vec<u8>) -> Self {
        Self {
            inner: InternalSqliteBindValue::Binary(b.into_boxed_slice()),
        }
    }
}

impl<'a> From<&'a [u8]> for SqliteBindValue<'a> {
    fn from(b: &'a [u8]) -> Self {
        Self {
            inner: InternalSqliteBindValue::BorrowedBinary(b),
        }
    }
}

#[derive(Debug)]
pub(crate) enum InternalSqliteBindValue<'a> {
    BorrowedString(&'a str),
    String(Box<str>),
    BorrowedBinary(&'a [u8]),
    Binary(Box<[u8]>),
    I32(i32),
    I64(i64),
    F64(f64),
    Null,
}

impl<'a> InternalSqliteBindValue<'a> {
    pub fn to_ffi_struct(idx: i32, src: InternalSqliteBindValue<'a>) -> BindPair {
        let key = BindKey::Numeric(idx as usize);
        match src {
            InternalSqliteBindValue::BorrowedString(s) => {
                BindPair(key, BindValue::Text(s.to_owned()))
            }
            InternalSqliteBindValue::String(s) => BindPair(key, BindValue::Text(s.to_string())),
            InternalSqliteBindValue::BorrowedBinary(blob) => {
                BindPair(key, BindValue::Blob(blob.to_owned()))
            }
            InternalSqliteBindValue::Binary(blob) => BindPair(key, BindValue::Blob(blob.to_vec())),
            InternalSqliteBindValue::I32(int) => BindPair(key, BindValue::Int(int)),
            InternalSqliteBindValue::I64(int) => BindPair(key, BindValue::Int64(int)),
            InternalSqliteBindValue::F64(double) => BindPair(key, BindValue::Double(double)),
            InternalSqliteBindValue::Null => BindPair(key, BindValue::Null),
        }
    }
}

impl std::fmt::Display for InternalSqliteBindValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = match self {
            InternalSqliteBindValue::BorrowedString(_) | InternalSqliteBindValue::String(_) => {
                "Text"
            }
            InternalSqliteBindValue::BorrowedBinary(_) | InternalSqliteBindValue::Binary(_) => {
                "Binary"
            }
            InternalSqliteBindValue::I32(_) | InternalSqliteBindValue::I64(_) => "Integer",
            InternalSqliteBindValue::F64(_) => "Float",
            InternalSqliteBindValue::Null => "Null",
        };
        f.write_str(n)
    }
}

impl<'a> BindCollector<'a, Sqlite> for SqliteBindCollector<'a> {
    type Buffer = SqliteBindValue<'a>;

    fn push_bound_value<T, U>(&mut self, bind: &'a U, metadata_lookup: &mut ()) -> QueryResult<()>
    where
        Sqlite: diesel::sql_types::HasSqlType<T>,
        U: diesel::serialize::ToSql<T, Sqlite>,
    {
        let value = SqliteBindValue {
            inner: InternalSqliteBindValue::Null,
        };
        let mut to_sql_output = Output::new(value, metadata_lookup);
        let is_null = bind
            .to_sql(&mut to_sql_output)
            .map_err(diesel::result::Error::SerializationError)?;
        let bind = to_sql_output.into_inner();
        let metadata = Sqlite::metadata(metadata_lookup);
        self.binds.push((
            match is_null {
                IsNull::No => bind.inner,
                IsNull::Yes => InternalSqliteBindValue::Null,
            },
            metadata,
        ));
        Ok(())
    }
}
