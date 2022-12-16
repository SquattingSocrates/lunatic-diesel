use diesel::query_builder::ReturningClause;

use crate::query_builder::{AstPass, QueryFragment};
use crate::result::QueryResult;
use crate::sqlite::diesel_backend::SqliteReturningClause;
use crate::sqlite::Sqlite;

impl<Expr> QueryFragment<Sqlite, SqliteReturningClause> for ReturningClause<Expr>
where
    Expr: QueryFragment<Sqlite>,
{
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        // out.skip_from(true);
        out.push_sql(" RETURNING ");
        self.0.walk_ast(out.reborrow())?;
        Ok(())
    }
}
