mod bind_collector;
mod constants;
mod diesel_backend;
mod diesel_connection;
mod expression;
mod functions;
mod host_bindings;
mod query_builder;
mod stmt;
mod types;

pub use diesel_backend::Sqlite;
pub use diesel_backend::SqliteType;

pub use diesel_connection::*;

/// Trait for the implementation of a SQLite aggregate function
///
/// This trait is to be used in conjunction with the `sql_function!`
/// macro for defining a custom SQLite aggregate function. See
/// the documentation [there](super::prelude::sql_function!) for details.
pub trait SqliteAggregateFunction<Args>: Default {
    /// The result type of the SQLite aggregate function
    type Output;

    /// The `step()` method is called once for every record of the query.
    ///
    /// This is called through a C FFI, as such panics do not propagate to the caller. Panics are
    /// caught and cause a return with an error value. The implementation must still ensure that
    /// state remains in a valid state (refer to [`std::panic::UnwindSafe`] for a bit more detail).
    fn step(&mut self, args: Args);

    /// After the last row has been processed, the `finalize()` method is
    /// called to compute the result of the aggregate function. If no rows
    /// were processed `aggregator` will be `None` and `finalize()` can be
    /// used to specify a default result.
    ///
    /// This is called through a C FFI, as such panics do not propagate to the caller. Panics are
    /// caught and cause a return with an error value.
    fn finalize(aggregator: Option<Self>) -> Self::Output;
}
