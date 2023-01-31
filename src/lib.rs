#[macro_use]
extern crate diesel;
extern crate lunatic_sqlite_api;

pub mod sqlite;

pub use diesel::*;

pub use sqlite::SqliteConnection;

#[export_name = "lunatic_alloc"]
extern "C" fn lunatic_alloc(len: u32) -> *mut u8 {
    let buf = Vec::with_capacity(len as usize);
    let mut buf = std::mem::ManuallyDrop::new(buf);
    buf.as_mut_ptr()
}
