use std::path::Path;

use diesel::{
    connection::{
        statement_cache::StatementCache, AnsiTransactionManager, ConnectionGatWorkaround,
        DefaultLoadingMode, LoadConnection, LoadRowIter, SimpleConnection, TransactionManager,
    },
    expression::QueryMetadata,
    query_builder::{Query, QueryFragment, QueryId},
    result::{DatabaseErrorKind, Error},
    row::{Field, PartialRow, Row, RowGatWorkaround, RowIndex},
    Connection, ConnectionResult, QueryResult,
};
use lunatic_sqlite_api::{SqliteError, SqliteValue};

use super::{
    diesel_backend::Sqlite,
    host_bindings,
    stmt::{Statement, StatementUse},
};

pub(crate) struct RawConnection {
    pub(crate) connection_id: u64,
}

impl RawConnection {
    fn exec(&mut self, query: &str) -> QueryResult<()> {
        match host_bindings::execute(self.connection_id, query) {
            0 => Ok(()),
            _ => Err(last_error(self.connection_id)),
        }
    }

    pub(crate) fn establish(path: &str) -> RawConnection {
        let path = Path::new(path);
        let connection_id = host_bindings::open(path);

        RawConnection { connection_id }
    }

    pub(super) fn rows_affected_by_last_query(&self) -> usize {
        unsafe {
            lunatic_sqlite_api::sqlite_guest_bindings::sqlite3_changes(self.connection_id) as usize
        }
    }

    // TODO: in order for this to work there needs to be a proper way of sending functions to the host
    // which could be done by sending a wasm functions name, so that the callback stored by the sqlite
    // instance in the host will actually point to a host function which calls the provided guest function.
    // However, this will require quite a bit of engineering and since it doesn't have a high priority at
    // the moment we'll keep the functionality out for now.
    //
    // pub(super) fn register_collation_function<F>(
    //     &self,
    //     collation_name: &str,
    //     collation: F,
    // ) -> QueryResult<()>
    // where
    //     F: Fn(&str, &str) -> std::cmp::Ordering + std::panic::UnwindSafe + Send + 'static,
    // {
    //     let callback_fn = Box::into_raw(Box::new(CollationUserPtr {
    //         callback: collation,
    //         collation_name: collation_name.to_owned(),
    //     }));
    //     let collation_name = Self::get_fn_name(collation_name)?;

    //     let result = unsafe {
    //         ffi::sqlite3_create_collation_v2(
    //             self.internal_connection.as_ptr(),
    //             collation_name.as_ptr(),
    //             ffi::SQLITE_UTF8,
    //             callback_fn as *mut _,
    //             Some(run_collation_function::<F>),
    //             Some(destroy_boxed::<CollationUserPtr<F>>),
    //         )
    //     };

    //     let result = Self::process_sql_function_result(result);
    //     if result.is_err() {
    //         destroy_boxed::<CollationUserPtr<F>>(callback_fn as *mut _);
    //     }
    //     result
    // }
}

fn last_error(connection_id: u64) -> Error {
    if let Ok(SqliteError {
        code: Some(error_code),
        message,
    }) = host_bindings::last_error(connection_id)
    {
        let error_kind = match error_code {
            lunatic_sqlite_api::SQLITE_CONSTRAINT_UNIQUE
            | lunatic_sqlite_api::SQLITE_CONSTRAINT_PRIMARYKEY => {
                DatabaseErrorKind::UniqueViolation
            }
            lunatic_sqlite_api::SQLITE_CONSTRAINT_FOREIGNKEY => {
                DatabaseErrorKind::ForeignKeyViolation
            }
            lunatic_sqlite_api::SQLITE_CONSTRAINT_NOTNULL => DatabaseErrorKind::NotNullViolation,
            lunatic_sqlite_api::SQLITE_CONSTRAINT_CHECK => DatabaseErrorKind::CheckViolation,
            _ => DatabaseErrorKind::Unknown,
        };
        return Error::DatabaseError(
            error_kind,
            Box::new(message.unwrap_or("sqlite error".to_string())),
        );
    }
    Error::DatabaseError(
        DatabaseErrorKind::Unknown,
        Box::new("unknown error code".to_string()),
    )
}

/// Connections for the SQLite backend. Unlike other backends, SQLite supported
/// connection URLs are:
///
/// - File paths (`test.db`)
/// - [URIs](https://sqlite.org/uri.html) (`file://test.db`)
/// - Special identifiers (`:memory:`)
///
/// # Supported loading model implementations
///
/// * [`DefaultLoadingMode`]
///
/// As `SqliteConnection` only supports a single loading mode implementation
/// it is **not required** to explicitly specify a loading mode
/// when calling [`RunQueryDsl::load_iter()`] or [`LoadConnection::load`]
///
/// [`RunQueryDsl::load_iter()`]: crate::query_dsl::RunQueryDsl::load_iter
///
/// ## DefaultLoadingMode
///
/// `SqliteConnection` only supports a single loading mode, which loads
/// values row by row from the result set.
///
/// ```rust
/// # include!("../../doctest_setup.rs");
/// #
/// # fn main() {
/// #     run_test().unwrap();
/// # }
/// #
/// # fn run_test() -> QueryResult<()> {
/// #     use schema::users;
/// #     let connection = &mut establish_connection();
/// use diesel::connection::DefaultLoadingMode;
/// { // scope to restrict the lifetime of the iterator
///     let iter1 = users::table.load_iter::<(i32, String), DefaultLoadingMode>(connection)?;
///
///     for r in iter1 {
///         let (id, name) = r?;
///         println!("Id: {} Name: {}", id, name);
///     }
/// }
///
/// // works without specifying the loading mode
/// let iter2 = users::table.load_iter::<(i32, String), _>(connection)?;
///
/// for r in iter2 {
///     let (id, name) = r?;
///     println!("Id: {} Name: {}", id, name);
/// }
/// #   Ok(())
/// # }
/// ```
///
/// This mode does **not support** creating
/// multiple iterators using the same connection.
///
/// ```compile_fail
/// # include!("../../doctest_setup.rs");
/// #
/// # fn main() {
/// #     run_test().unwrap();
/// # }
/// #
/// # fn run_test() -> QueryResult<()> {
/// #     use schema::users;
/// #     let connection = &mut establish_connection();
/// use diesel::connection::DefaultLoadingMode;
///
/// let iter1 = users::table.load_iter::<(i32, String), DefaultLoadingMode>(connection)?;
/// let iter2 = users::table.load_iter::<(i32, String), DefaultLoadingMode>(connection)?;
///
/// for r in iter1 {
///     let (id, name) = r?;
///     println!("Id: {} Name: {}", id, name);
/// }
///
/// for r in iter2 {
///     let (id, name) = r?;
///     println!("Id: {} Name: {}", id, name);
/// }
/// #   Ok(())
/// # }
/// ```
#[allow(missing_debug_implementations)]
pub struct SqliteConnection {
    // statement_cache needs to be before raw_connection
    // otherwise we will get errors about open statements before closing the
    // connection itself
    statement_cache: diesel::connection::statement_cache::StatementCache<Sqlite, Statement>,
    raw_connection: RawConnection,
    transaction_state: AnsiTransactionManager,
}

// This relies on the invariant that RawConnection or Statement are never
// leaked. If a reference to one of those was held on a different thread, this
// would not be thread safe.
unsafe impl Send for SqliteConnection {}

impl SimpleConnection for SqliteConnection {
    fn batch_execute(&mut self, query: &str) -> QueryResult<()> {
        self.raw_connection.exec(query)
    }
}

impl<'conn, 'query> ConnectionGatWorkaround<'conn, 'query, Sqlite> for SqliteConnection {
    type Cursor = StatementIterator<'conn, 'query>;
    type Row = SqliteRow;
}

impl Connection for SqliteConnection {
    type Backend = super::diesel_backend::Sqlite;
    type TransactionManager = AnsiTransactionManager;

    /// Establish a connection to the database specified by `database_url`.
    ///
    /// See [SqliteConnection] for supported `database_url`.
    ///
    /// If the database does not exist, this method will try to
    /// create a new database and then establish a connection to it.
    fn establish(database_url: &str) -> ConnectionResult<Self> {
        // use diesel::result::ConnectionError::CouldntSetupConfiguration;

        let raw_connection = RawConnection::establish(database_url);
        let conn = Self {
            statement_cache: StatementCache::new(),
            raw_connection,
            transaction_state: AnsiTransactionManager::default(),
        };
        // conn.register_diesel_sql_functions()
        //     .map_err(diesel::ConnectionError::CouldntSetupConfiguration)?;
        Ok(conn)
    }

    fn execute_returning_count<T>(&mut self, source: &T) -> QueryResult<usize>
    where
        T: QueryFragment<Self::Backend> + QueryId,
    {
        let statement_use = self.prepared_query(source)?;
        statement_use.run()?;

        Ok(self.raw_connection.rows_affected_by_last_query())
    }

    fn transaction_state(&mut self) -> &mut AnsiTransactionManager
    where
        Self: Sized,
    {
        &mut self.transaction_state
    }
}

impl LoadConnection<DefaultLoadingMode> for SqliteConnection {
    fn load<'conn, 'query, T>(
        &'conn mut self,
        source: T,
    ) -> QueryResult<LoadRowIter<'conn, 'query, Self, Self::Backend>>
    where
        T: Query + QueryFragment<Self::Backend> + QueryId + 'query,
        Self::Backend: QueryMetadata<T::SqlType>,
    {
        let statement_use = self.prepared_query(source)?;

        Ok(StatementIterator::new(statement_use))
        // Ok(StatementIterator {})
    }
}

pub struct StatementIterator<'stmt, 'query> {
    is_first: bool,
    statement_use: StatementUse<'stmt, 'query>,
}

impl<'stmt, 'query> StatementIterator<'stmt, 'query> {
    pub fn new(statement_use: StatementUse<'stmt, 'query>) -> Self {
        Self {
            is_first: true,
            statement_use,
        }
    }
}

pub struct SqliteRow {
    pub inner_row: lunatic_sqlite_api::SqliteRow,
    pub statement_id: u64,
    pub field_names: Vec<String>,
}

// impl Deref for SqliteRow {
//     type Target = lunatic_sqlite_api::SqliteRow;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

impl<'field, 'stmt, 'query> RowGatWorkaround<'field, Sqlite> for SqliteRow {
    type Field = SqliteField<'field>;
}

impl<'stmt, 'query> Row<'stmt, Sqlite> for SqliteRow {
    type InnerPartialRow = Self;

    fn field_count(&self) -> usize {
        self.inner_row.0.len()
    }

    fn get<'field, I>(
        &'field self,
        idx: I,
    ) -> Option<<Self as RowGatWorkaround<'field, Sqlite>>::Field>
    where
        'stmt: 'field,
        Self: RowIndex<I>,
    {
        if let Some(column_index) = self.idx(idx) {
            if let Some(original_column) = self.inner_row.get_column(column_index as i32) {
                return Some(SqliteField {
                    inner_field: original_column,
                    field_name: self.field_names.get(column_index).map(|name| name.clone()),
                });
            }
        }
        None
        // let idx = self.idx(idx)?;
        // Some(SqliteField {
        //     row: self.inner.borrow(),
        //     col_idx: i32::try_from(idx).ok()?,
        // })
    }

    fn partial_row(&self, range: std::ops::Range<usize>) -> PartialRow<'_, Self::InnerPartialRow> {
        PartialRow::new(self, range)
    }
}

impl<'stmt, 'query> RowIndex<usize> for SqliteRow {
    fn idx(&self, idx: usize) -> Option<usize> {
        if idx < self.field_count() {
            Some(idx)
        } else {
            None
        }
    }
}

impl<'stmt, 'idx, 'query> RowIndex<&'idx str> for SqliteRow {
    fn idx(&self, field_name: &'idx str) -> Option<usize> {
        self.field_names.iter().position(|s| s == field_name)
    }
}

#[allow(missing_debug_implementations)]
pub struct SqliteField<'a> {
    pub(super) inner_field: &'a SqliteValue,
    pub(super) field_name: Option<String>,
}

impl<'stmt, 'query> Field<'stmt, Sqlite> for SqliteField<'stmt> {
    fn field_name(&self) -> Option<&str> {
        if let Some(name) = &self.field_name {
            return Some(name.as_str());
        }
        None
    }

    fn is_null(&self) -> bool {
        self.value().is_none()
    }

    fn value(&self) -> Option<diesel::backend::RawValue<'_, Sqlite>> {
        Some(&self.inner_field)
    }
}

impl<'stmt, 'query> Iterator for StatementIterator<'stmt, 'query> {
    type Item = QueryResult<SqliteRow>;

    fn next(&mut self) -> Option<Self::Item> {
        let step = unsafe { self.statement_use.step(self.is_first) };
        self.is_first = false;
        match step {
            Err(e) => Some(Err(e)),
            Ok(false) => None,
            Ok(true) => {
                let statement_id = self.statement_use.statement.statement.statement_id;
                Some(
                    host_bindings::read_row(statement_id).map(|inner_row| SqliteRow {
                        inner_row,
                        statement_id: statement_id as u64,
                        field_names: host_bindings::column_names(statement_id).unwrap(),
                    }),
                )
            }
        }
    }
}

#[cfg(feature = "r2d2")]
impl crate::r2d2::R2D2Connection for crate::sqlite::SqliteConnection {
    fn ping(&mut self) -> QueryResult<()> {
        use crate::RunQueryDsl;

        crate::r2d2::CheckConnectionQuery.execute(self).map(|_| ())
    }

    fn is_broken(&mut self) -> bool {
        AnsiTransactionManager::is_broken_transaction_manager(self)
    }
}

impl SqliteConnection {
    /// Run a transaction with `BEGIN IMMEDIATE`
    ///
    /// This method will return an error if a transaction is already open.
    ///
    /// # Example
    ///
    /// ```rust
    /// # include!("../../doctest_setup.rs");
    /// #
    /// # fn main() {
    /// #     run_test().unwrap();
    /// # }
    /// #
    /// # fn run_test() -> QueryResult<()> {
    /// #     let mut conn = SqliteConnection::establish(":memory:").unwrap();
    /// conn.immediate_transaction(|conn| {
    ///     // Do stuff in a transaction
    ///     Ok(())
    /// })
    /// # }
    /// ```
    pub fn immediate_transaction<T, E, F>(&mut self, f: F) -> Result<T, E>
    where
        F: FnOnce(&mut Self) -> Result<T, E>,
        E: From<Error>,
    {
        self.transaction_sql(f, "BEGIN IMMEDIATE")
    }

    /// Run a transaction with `BEGIN EXCLUSIVE`
    ///
    /// This method will return an error if a transaction is already open.
    ///
    /// # Example
    ///
    /// ```rust
    /// # include!("../../doctest_setup.rs");
    /// #
    /// # fn main() {
    /// #     run_test().unwrap();
    /// # }
    /// #
    /// # fn run_test() -> QueryResult<()> {
    /// #     let mut conn = SqliteConnection::establish(":memory:").unwrap();
    /// conn.exclusive_transaction(|conn| {
    ///     // Do stuff in a transaction
    ///     Ok(())
    /// })
    /// # }
    /// ```
    pub fn exclusive_transaction<T, E, F>(&mut self, f: F) -> Result<T, E>
    where
        F: FnOnce(&mut Self) -> Result<T, E>,
        E: From<Error>,
    {
        self.transaction_sql(f, "BEGIN EXCLUSIVE")
    }

    fn transaction_sql<T, E, F>(&mut self, f: F, sql: &str) -> Result<T, E>
    where
        F: FnOnce(&mut Self) -> Result<T, E>,
        E: From<Error>,
    {
        AnsiTransactionManager::begin_transaction_sql(&mut *self, sql)?;
        match f(&mut *self) {
            Ok(value) => {
                AnsiTransactionManager::commit_transaction(&mut *self)?;
                Ok(value)
            }
            Err(e) => {
                AnsiTransactionManager::rollback_transaction(&mut *self)?;
                Err(e)
            }
        }
    }

    fn prepared_query<'a, 'b, T>(&'a mut self, source: T) -> QueryResult<StatementUse<'a, 'b>>
    where
        T: QueryFragment<Sqlite> + QueryId + 'b,
    {
        let raw_connection = &self.raw_connection;
        let cache = &mut self.statement_cache;
        let statement = cache.cached_statement(&source, &Sqlite, &[], |sql, is_cached| {
            Statement::prepare(raw_connection, sql, is_cached)
        })?;

        StatementUse::bind(statement, source)
    }

    // #[doc(hidden)]
    // pub fn register_sql_function<ArgsSqlType, RetSqlType, Args, Ret, F>(
    //     &mut self,
    //     fn_name: &str,
    //     deterministic: bool,
    //     mut f: F,
    // ) -> QueryResult<()>
    // where
    //     F: FnMut(Args) -> Ret + std::panic::UnwindSafe + Send + 'static,
    //     Args: FromSqlRow<ArgsSqlType, Sqlite> + StaticallySizedRow<ArgsSqlType, Sqlite>,
    //     Ret: ToSql<RetSqlType, Sqlite>,
    //     Sqlite: HasSqlType<RetSqlType>,
    // {
    //     functions::register(
    //         &self.raw_connection,
    //         fn_name,
    //         deterministic,
    //         move |_, args| f(args),
    //     )
    // }

    // #[doc(hidden)]
    // pub fn register_noarg_sql_function<RetSqlType, Ret, F>(
    //     &self,
    //     fn_name: &str,
    //     deterministic: bool,
    //     f: F,
    // ) -> QueryResult<()>
    // where
    //     F: FnMut() -> Ret + std::panic::UnwindSafe + Send + 'static,
    //     Ret: ToSql<RetSqlType, Sqlite>,
    //     Sqlite: HasSqlType<RetSqlType>,
    // {
    //     functions::register_noargs(&self.raw_connection, fn_name, deterministic, f)
    // }

    // #[doc(hidden)]
    // pub fn register_aggregate_function<ArgsSqlType, RetSqlType, Args, Ret, A>(
    //     &mut self,
    //     fn_name: &str,
    // ) -> QueryResult<()>
    // where
    //     A: SqliteAggregateFunction<Args, Output = Ret> + 'static + Send + std::panic::UnwindSafe,
    //     Args: FromSqlRow<ArgsSqlType, Sqlite> + StaticallySizedRow<ArgsSqlType, Sqlite>,
    //     Ret: ToSql<RetSqlType, Sqlite>,
    //     Sqlite: HasSqlType<RetSqlType>,
    // {
    //     functions::register_aggregate::<_, _, _, _, A>(&self.raw_connection, fn_name)
    // }

    // /// Register a collation function.
    // ///
    // /// `collation` must always return the same answer given the same inputs.
    // /// If `collation` panics and unwinds the stack, the process is aborted, since it is used
    // /// across a C FFI boundary, which cannot be unwound across and there is no way to
    // /// signal failures via the SQLite interface in this case..
    // ///
    // /// If the name is already registered it will be overwritten.
    // ///
    // /// This method will return an error if registering the function fails, either due to an
    // /// out-of-memory situation or because a collation with that name already exists and is
    // /// currently being used in parallel by a query.
    // ///
    // /// The collation needs to be specified when creating a table:
    // /// `CREATE TABLE my_table ( str TEXT COLLATE MY_COLLATION )`,
    // /// where `MY_COLLATION` corresponds to name passed as `collation_name`.
    // ///
    // /// # Example
    // ///
    // /// ```rust
    // /// # include!("../../doctest_setup.rs");
    // /// #
    // /// # fn main() {
    // /// #     run_test().unwrap();
    // /// # }
    // /// #
    // /// # fn run_test() -> QueryResult<()> {
    // /// #     let mut conn = SqliteConnection::establish(":memory:").unwrap();
    // /// // sqlite NOCASE only works for ASCII characters,
    // /// // this collation allows handling UTF-8 (barring locale differences)
    // /// conn.register_collation("RUSTNOCASE", |rhs, lhs| {
    // ///     rhs.to_lowercase().cmp(&lhs.to_lowercase())
    // /// })
    // /// # }
    // /// ```
    // pub fn register_collation<F>(&mut self, collation_name: &str, collation: F) -> QueryResult<()>
    // where
    //     F: Fn(&str, &str) -> std::cmp::Ordering + Send + 'static + std::panic::UnwindSafe,
    // {
    //     self.raw_connection
    //         .register_collation_function(collation_name, collation)
    // }

    // fn register_diesel_sql_functions(&self) -> QueryResult<()> {
    //     use diesel::sql_types::{Integer, Text};

    //     functions::register::<Text, Integer, _, _, _>(
    //         &self.raw_connection,
    //         "diesel_manage_updated_at",
    //         false,
    //         |conn, table_name: String| {
    //             conn.exec(&format!(
    //                 include_str!("diesel_manage_updated_at.sql"),
    //                 table_name = table_name
    //             ))
    //             .expect("Failed to create trigger");
    //             0 // have to return *something*
    //         },
    //     )
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::dsl::sql;
    use diesel::prelude::*;
    use diesel::sql_types::Integer;

    // #[test]
    // fn prepared_statements_are_cached_when_run() {
    //     let connection = &mut SqliteConnection::establish(":memory:").unwrap();
    //     let query = diesel::select(1.into_sql::<Integer>());

    //     assert_eq!(Ok(1), query.get_result(connection));
    //     assert_eq!(Ok(1), query.get_result(connection));
    //     assert_eq!(1, connection.statement_cache.len());
    // }

    #[lunatic::test]
    fn sql_literal_nodes_are_not_cached() {
        let connection = &mut SqliteConnection::establish(":memory:").unwrap();
        let query = diesel::select(sql::<Integer>("1"));

        assert_eq!(Ok(1), query.get_result(connection));
        assert_eq!(0, connection.statement_cache.len());
    }

    #[test]
    fn queries_containing_sql_literal_nodes_are_not_cached() {
        let connection = &mut SqliteConnection::establish(":memory:").unwrap();
        let one_as_expr = 1.into_sql::<Integer>();
        let query = diesel::select(one_as_expr.eq(sql::<Integer>("1")));

        assert_eq!(Ok(true), query.get_result(connection));
        assert_eq!(0, connection.statement_cache.len());
    }

    #[test]
    fn queries_containing_in_with_vec_are_not_cached() {
        let connection = &mut SqliteConnection::establish(":memory:").unwrap();
        let one_as_expr = 1.into_sql::<Integer>();
        let query = diesel::select(one_as_expr.eq_any(vec![1, 2, 3]));

        assert_eq!(Ok(true), query.get_result(connection));
        assert_eq!(0, connection.statement_cache.len());
    }

    #[test]
    fn queries_containing_in_with_subselect_are_cached() {
        let connection = &mut SqliteConnection::establish(":memory:").unwrap();
        let one_as_expr = 1.into_sql::<Integer>();
        let query = diesel::select(one_as_expr.eq_any(diesel::select(one_as_expr)));

        assert_eq!(Ok(true), query.get_result(connection));
        assert_eq!(1, connection.statement_cache.len());
    }

    use diesel::sql_types::Text;
    sql_function!(fn fun_case(x: Text) -> Text);

    // #[test]
    // fn register_custom_function() {
    //     let connection = &mut SqliteConnection::establish(":memory:").unwrap();
    //     fun_case::register_impl(connection, |x: String| {
    //         x.chars()
    //             .enumerate()
    //             .map(|(i, c)| {
    //                 if i % 2 == 0 {
    //                     c.to_lowercase().to_string()
    //                 } else {
    //                     c.to_uppercase().to_string()
    //                 }
    //             })
    //             .collect::<String>()
    //     })
    //     .unwrap();

    //     let mapped_string = diesel::select(fun_case("foobar"))
    //         .get_result::<String>(connection)
    //         .unwrap();
    //     assert_eq!("fOoBaR", mapped_string);
    // }

    sql_function!(fn my_add(x: Integer, y: Integer) -> Integer);

    // #[test]
    // fn register_multiarg_function() {
    //     let connection = &mut SqliteConnection::establish(":memory:").unwrap();
    //     my_add::register_impl(connection, |x: i32, y: i32| x + y).unwrap();

    //     let added = diesel::select(my_add(1, 2)).get_result::<i32>(connection);
    //     assert_eq!(Ok(3), added);
    // }

    // sql_function!(fn answer() -> Integer);

    // #[test]
    // fn register_noarg_function() {
    //     let connection = &mut SqliteConnection::establish(":memory:").unwrap();
    //     answer::register_impl(connection, || 42).unwrap();

    //     let answer = diesel::select(answer()).get_result::<i32>(connection);
    //     assert_eq!(Ok(42), answer);
    // }

    // #[test]
    // fn register_nondeterministic_noarg_function() {
    //     let connection = &mut SqliteConnection::establish(":memory:").unwrap();
    //     answer::register_nondeterministic_impl(connection, || 42).unwrap();

    //     let answer = diesel::select(answer()).get_result::<i32>(connection);
    //     assert_eq!(Ok(42), answer);
    // }

    // sql_function!(fn add_counter(x: Integer) -> Integer);

    // #[test]
    // fn register_nondeterministic_function() {
    //     let connection = &mut SqliteConnection::establish(":memory:").unwrap();
    //     let mut y = 0;
    //     add_counter::register_nondeterministic_impl(connection, move |x: i32| {
    //         y += 1;
    //         x + y
    //     })
    //     .unwrap();

    //     let added = diesel::select((add_counter(1), add_counter(1), add_counter(1)))
    //         .get_result::<(i32, i32, i32)>(connection);
    //     assert_eq!(Ok((2, 3, 4)), added);
    // }

    use crate::sqlite::SqliteAggregateFunction;

    sql_function! {
        #[aggregate]
        fn my_sum(expr: Integer) -> Integer;
    }

    #[derive(Default)]
    struct MySum {
        sum: i32,
    }

    impl SqliteAggregateFunction<i32> for MySum {
        type Output = i32;

        fn step(&mut self, expr: i32) {
            self.sum += expr;
        }

        fn finalize(aggregator: Option<Self>) -> Self::Output {
            aggregator.map(|a| a.sum).unwrap_or_default()
        }
    }

    table! {
        my_sum_example {
            id -> Integer,
            value -> Integer,
        }
    }

    // #[test]
    // fn register_aggregate_function() {
    //     use self::my_sum_example::dsl::*;

    //     let connection = &mut SqliteConnection::establish(":memory:").unwrap();
    //     diesel::sql_query(
    //         "CREATE TABLE my_sum_example (id integer primary key autoincrement, value integer)",
    //     )
    //     .execute(connection)
    //     .unwrap();
    //     diesel::sql_query("INSERT INTO my_sum_example (value) VALUES (1), (2), (3)")
    //         .execute(connection)
    //         .unwrap();

    //     my_sum::register_impl::<MySum, _>(connection).unwrap();

    //     let result = my_sum_example
    //         .select(my_sum(value))
    //         .get_result::<i32>(connection);
    //     assert_eq!(Ok(6), result);
    // }

    // #[test]
    // fn register_aggregate_function_returns_finalize_default_on_empty_set() {
    //     use self::my_sum_example::dsl::*;

    //     let connection = &mut SqliteConnection::establish(":memory:").unwrap();
    //     diesel::sql_query(
    //         "CREATE TABLE my_sum_example (id integer primary key autoincrement, value integer)",
    //     )
    //     .execute(connection)
    //     .unwrap();

    //     my_sum::register_impl::<MySum, _>(connection).unwrap();

    //     let result = my_sum_example
    //         .select(my_sum(value))
    //         .get_result::<i32>(connection);
    //     assert_eq!(Ok(0), result);
    // }

    // sql_function! {
    //     #[aggregate]
    //     fn range_max(expr1: Integer, expr2: Integer, expr3: Integer) -> Nullable<Integer>;
    // }

    #[derive(Default)]
    struct RangeMax<T> {
        max_value: Option<T>,
    }

    impl<T: Default + Ord + Copy + Clone> SqliteAggregateFunction<(T, T, T)> for RangeMax<T> {
        type Output = Option<T>;

        fn step(&mut self, (x0, x1, x2): (T, T, T)) {
            let max = if x0 >= x1 && x0 >= x2 {
                x0
            } else if x1 >= x0 && x1 >= x2 {
                x1
            } else {
                x2
            };

            self.max_value = match self.max_value {
                Some(current_max_value) if max > current_max_value => Some(max),
                None => Some(max),
                _ => self.max_value,
            };
        }

        fn finalize(aggregator: Option<Self>) -> Self::Output {
            aggregator?.max_value
        }
    }

    table! {
        range_max_example {
            id -> Integer,
            value1 -> Integer,
            value2 -> Integer,
            value3 -> Integer,
        }
    }

    // #[test]
    // fn register_aggregate_multiarg_function() {
    //     use self::range_max_example::dsl::*;

    //     let connection = &mut SqliteConnection::establish(":memory:").unwrap();
    //     diesel::sql_query(
    //         r#"CREATE TABLE range_max_example (
    //             id integer primary key autoincrement,
    //             value1 integer,
    //             value2 integer,
    //             value3 integer
    //         )"#,
    //     )
    //     .execute(connection)
    //     .unwrap();
    //     diesel::sql_query(
    //         "INSERT INTO range_max_example (value1, value2, value3) VALUES (3, 2, 1), (2, 2, 2)",
    //     )
    //     .execute(connection)
    //     .unwrap();

    //     range_max::register_impl::<RangeMax<i32>, _, _, _>(connection).unwrap();
    //     let result = range_max_example
    //         .select(range_max(value1, value2, value3))
    //         .get_result::<Option<i32>>(connection)
    //         .unwrap();
    //     assert_eq!(Some(3), result);
    // }

    table! {
        my_collation_example {
            id -> Integer,
            value -> Text,
        }
    }

    #[test]
    fn register_collation_function() {
        use self::my_collation_example::dsl::*;

        let connection = &mut SqliteConnection::establish(":memory:").unwrap();

        // connection
        //     .register_collation("RUSTNOCASE", |rhs, lhs| {
        //         rhs.to_lowercase().cmp(&lhs.to_lowercase())
        //     })
        //     .unwrap();

        diesel::sql_query(
                "CREATE TABLE my_collation_example (id integer primary key autoincrement, value text collate RUSTNOCASE)",
            ).execute(connection)
            .unwrap();
        diesel::sql_query(
            "INSERT INTO my_collation_example (value) VALUES ('foo'), ('FOo'), ('f00')",
        )
        .execute(connection)
        .unwrap();

        let result = my_collation_example
            .filter(value.eq("foo"))
            .select(value)
            .load::<String>(connection);
        assert_eq!(
            Ok(&["foo".to_owned(), "FOo".to_owned()][..]),
            result.as_ref().map(|vec| vec.as_ref())
        );

        let result = my_collation_example
            .filter(value.eq("FOO"))
            .select(value)
            .load::<String>(connection);
        assert_eq!(
            Ok(&["foo".to_owned(), "FOo".to_owned()][..]),
            result.as_ref().map(|vec| vec.as_ref())
        );

        let result = my_collation_example
            .filter(value.eq("f00"))
            .select(value)
            .load::<String>(connection);
        assert_eq!(
            Ok(&["f00".to_owned()][..]),
            result.as_ref().map(|vec| vec.as_ref())
        );

        let result = my_collation_example
            .filter(value.eq("F00"))
            .select(value)
            .load::<String>(connection);
        assert_eq!(
            Ok(&["f00".to_owned()][..]),
            result.as_ref().map(|vec| vec.as_ref())
        );

        let result = my_collation_example
            .filter(value.eq("oof"))
            .select(value)
            .load::<String>(connection);
        assert_eq!(Ok(&[][..]), result.as_ref().map(|vec| vec.as_ref()));
    }
}
