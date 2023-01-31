# `lunatic-sql`

A collection of Diesel Backends and Connections that enable the usage of various sql databases with the Diesel ORM.
The crate re-exports all of diesel and is therefore to be used as a replacement for diesel and should be used under
the name `diesel` in the dependencies

An example of using it with SQLite is provided here:
https://github.com/SquattingSocrates/sqlite-lunatic-diesel-example

Currently supported databases:

- [x] SQLite
- [ ] PostgreSQL
- [ ] MySQL


## Usage
Steps to use this library:

- install [lunatic](https://github.com/lunatic-solutions/lunatic)
- install [diesel cli](https://github.com/diesel-rs/diesel/tree/master/diesel_cli) + the cli dependencies for your database of choice
- create a new rust project
- add [lunatic-sql](https://github.com/SquattingSocrates/lunatic-sql) as dependency, but use it under the name of `diesel` like this: `diesel = {package = "lunatic-sql", version = "0.1.0"}` or else some of the features of diesel will not work properly
- create a migration with `diesel migration generate`
- start building your app


## Roadmap

- [x] Implement a working Backend and Connection for SQLite
  - [x] Reading from db
  - [x] Inserting into db
  - [x] Update entries
  - [x] Delete entries
  - [x] Use diesel models and helper functions
  - [x] Transactions
  - [x] Joining tables
  - [x] `Returning` statement
  - [ ] Support for custom SQL functions
- [ ] Implement a Backend and Connection for PostgreSQL
- [ ] Implement a Backend and Connection for MySQL