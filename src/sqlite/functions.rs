// use lunatic_sqlite_api::guest_api::{SqliteRow, SqliteValue};

// use super::bind_collector::{InternalSqliteBindValue, SqliteBindValue};
// use super::{RawConnection, Sqlite, SqliteAggregateFunction};
// use crate::deserialize::{FromSqlRow, StaticallySizedRow};
// use crate::result::{DatabaseErrorKind, Error, QueryResult};
// use crate::row::{Field, PartialRow, Row, RowGatWorkaround, RowIndex};
// use crate::serialize::{IsNull, Output, ToSql};
// use crate::sql_types::HasSqlType;
// use std::cell::{Ref, RefCell};
// use std::mem::ManuallyDrop;
// use std::rc::Rc;

// pub(super) fn register<ArgsSqlType, RetSqlType, Args, Ret, F>(
//     conn: &RawConnection,
//     fn_name: &str,
//     deterministic: bool,
//     mut f: F,
// ) -> QueryResult<()>
// where
//     F: FnMut(&RawConnection, Args) -> Ret + std::panic::UnwindSafe + Send + 'static,
//     Args: FromSqlRow<ArgsSqlType, Sqlite> + StaticallySizedRow<ArgsSqlType, Sqlite>,
//     Ret: ToSql<RetSqlType, Sqlite>,
//     Sqlite: HasSqlType<RetSqlType>,
// {
//     let fields_needed = Args::FIELD_COUNT;
//     if fields_needed > 127 {
//         return Err(Error::DatabaseError(
//             DatabaseErrorKind::UnableToSendCommand,
//             Box::new("SQLite functions cannot take more than 127 parameters".to_string()),
//         ));
//     }

//     // conn.register_sql_function(fn_name, fields_needed, deterministic, move |conn, args| {
//     //     let args = build_sql_function_args::<ArgsSqlType, Args>(args)?;

//     //     Ok(f(conn, args))
//     // })?;
//     Ok(())
// }

// pub(super) fn register_noargs<RetSqlType, Ret, F>(
//     conn: &RawConnection,
//     fn_name: &str,
//     deterministic: bool,
//     mut f: F,
// ) -> QueryResult<()>
// where
//     F: FnMut() -> Ret + std::panic::UnwindSafe + Send + 'static,
//     Ret: ToSql<RetSqlType, Sqlite>,
//     Sqlite: HasSqlType<RetSqlType>,
// {
//     conn.register_sql_function(fn_name, 0, deterministic, move |_, _| Ok(f()))?;
//     Ok(())
// }

// pub(super) fn register_aggregate<ArgsSqlType, RetSqlType, Args, Ret, A>(
//     conn: &RawConnection,
//     fn_name: &str,
// ) -> QueryResult<()>
// where
//     A: SqliteAggregateFunction<Args, Output = Ret> + 'static + Send + std::panic::UnwindSafe,
//     Args: FromSqlRow<ArgsSqlType, Sqlite> + StaticallySizedRow<ArgsSqlType, Sqlite>,
//     Ret: ToSql<RetSqlType, Sqlite>,
//     Sqlite: HasSqlType<RetSqlType>,
// {
//     let fields_needed = Args::FIELD_COUNT;
//     if fields_needed > 127 {
//         return Err(Error::DatabaseError(
//             DatabaseErrorKind::UnableToSendCommand,
//             Box::new("SQLite functions cannot take more than 127 parameters".to_string()),
//         ));
//     }

//     conn.register_aggregate_function::<ArgsSqlType, RetSqlType, Args, Ret, A>(
//         fn_name,
//         fields_needed,
//     )?;

//     Ok(())
// }

// pub(super) fn build_sql_function_args<ArgsSqlType, Args>(
//     args: &mut [SqliteValue],
// ) -> Result<Args, Error>
// where
//     Args: FromSqlRow<ArgsSqlType, Sqlite>,
// {
//     let row = FunctionRow::new(args);
//     Args::build_from_row(&row).map_err(Error::DeserializationError)
// }

// // clippy is wrong here, the let binding is required
// // for lifetime reasons
// #[allow(clippy::let_unit_value)]
// pub(super) fn process_sql_function_result<RetSqlType, Ret>(
//     result: &'_ Ret,
// ) -> QueryResult<InternalSqliteBindValue<'_>>
// where
//     Ret: ToSql<RetSqlType, Sqlite>,
//     Sqlite: HasSqlType<RetSqlType>,
// {
//     let mut metadata_lookup = ();
//     let value = SqliteBindValue {
//         inner: InternalSqliteBindValue::Null,
//     };
//     let mut buf = Output::new(value, &mut metadata_lookup);
//     let is_null = result.to_sql(&mut buf).map_err(Error::SerializationError)?;

//     if let IsNull::Yes = is_null {
//         Ok(InternalSqliteBindValue::Null)
//     } else {
//         Ok(buf.into_inner().inner)
//     }
// }

// struct FunctionRow<'a> {
//     // we use `ManuallyDrop` to prevent dropping the content of the internal vector
//     // as this buffer is owned by sqlite not by diesel
//     // args: Rc<RefCell<ManuallyDrop<SqliteRow>>>,
//     args: &'a [SqliteValue],
//     field_count: usize,
//     // marker: PhantomData<&'a ffi::sqlite3_value>,
// }

// impl<'a> Drop for FunctionRow<'a> {
//     fn drop(&mut self) {
//         // if let Some(args) = Rc::get_mut(&mut self.args) {
//         //     if let SqliteRow = DerefMut::deref_mut(RefCell::get_mut(args)) {
//         //         if Rc::strong_count(column_names) == 1 {
//         //             // According the https://doc.rust-lang.org/std/mem/struct.ManuallyDrop.html#method.drop
//         //             // it's fine to just drop the values here
//         //             unsafe { std::ptr::drop_in_place(column_names as *mut _) }
//         //         }
//         //     }
//         // }
//     }
// }

// impl<'a> FunctionRow<'a> {
//     fn new(args: &mut [SqliteValue]) -> Self {
//         let lengths = args.len();

//         Self {
//             field_count: lengths,
//             args,
//         }
//     }
// }

// impl<'a, 'b> RowGatWorkaround<'a, Sqlite> for FunctionRow<'b> {
//     type Field = FunctionArgument<'a>;
// }

// impl<'a> Row<'a, Sqlite> for FunctionRow<'a> {
//     type InnerPartialRow = Self;

//     fn field_count(&self) -> usize {
//         self.field_count
//     }

//     fn get<'b, I>(&'b self, idx: I) -> Option<<Self as RowGatWorkaround<'b, Sqlite>>::Field>
//     where
//         'a: 'b,
//         Self: crate::row::RowIndex<I>,
//     {
//         let idx = self.idx(idx)?;
//         Some(FunctionArgument {
//             args: self.args.borrow(),
//             col_idx: idx as i32,
//         })
//     }

//     fn partial_row(&self, range: std::ops::Range<usize>) -> PartialRow<'_, Self::InnerPartialRow> {
//         PartialRow::new(self, range)
//     }
// }

// impl<'a> RowIndex<usize> for FunctionRow<'a> {
//     fn idx(&self, idx: usize) -> Option<usize> {
//         if idx < self.field_count() {
//             Some(idx)
//         } else {
//             None
//         }
//     }
// }

// impl<'a, 'b> RowIndex<&'a str> for FunctionRow<'b> {
//     fn idx(&self, _idx: &'a str) -> Option<usize> {
//         None
//     }
// }

// struct FunctionArgument<'a> {
//     args: Ref<'a, ManuallyDrop<SqliteRow>>,
//     col_idx: i32,
// }

// impl<'a> Field<'a, Sqlite> for FunctionArgument<'a> {
//     fn field_name(&self) -> Option<&str> {
//         None
//     }

//     fn is_null(&self) -> bool {
//         self.value().is_none()
//     }

//     fn value(&self) -> Option<crate::backend::RawValue<'_, Sqlite>> {
//         SqliteValue::new(
//             Ref::map(Ref::clone(&self.args), |drop| std::ops::Deref::deref(drop)),
//             self.col_idx,
//         )
//     }
// }
