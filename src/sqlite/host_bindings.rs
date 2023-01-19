use std::path::Path;

use diesel::{result::Error, QueryResult};

use lunatic::Process;
pub use lunatic_sqlite_api::guest_api::*;
use lunatic_sqlite_api::wire_format::{BindPair, SqliteError};
pub use lunatic_sqlite_api::*;

pub fn open(path: &Path) -> u64 {
    let conn_id = 0u64;
    let path_str = path.to_str().unwrap();
    unsafe {
        sqlite_guest_bindings::open(path_str.as_ptr(), path_str.len(), &mut (conn_id as u32));
    }
    conn_id
}

/// returns a tuple consisting of the length of data written to the buf
/// as well as the resource id
pub fn query_prepare(conn_id: u64, query: &str) -> (u32, u64) {
    let len = 0u32;
    unsafe {
        let resource_id =
            sqlite_guest_bindings::query_prepare(conn_id, query.as_ptr(), query.len() as u32);
        (len, resource_id)
    }
}

pub fn execute(conn_id: u64, exec_str: &str) -> u32 {
    unsafe { sqlite_guest_bindings::execute(conn_id, exec_str.as_ptr(), exec_str.len() as u32) }
}

pub fn bind_value(statement_id: u64, value: BindPair) {
    let bind_list = BindList(vec![value]);
    let encoded = bincode::serialize(&bind_list).unwrap();
    unsafe {
        sqlite_guest_bindings::bind_value(
            statement_id,
            encoded.as_ptr() as u32,
            encoded.len() as u32,
        )
    }
}

pub fn sqlite3_reset(statement_id: u64) {
    unsafe {
        sqlite_guest_bindings::statement_reset(statement_id);
    }
}

// helper function to unwrap byte slice that was allocated during host call
fn unroll_vec(ptr: u32, len: u32) -> Vec<u8> {
    let len = len as usize;
    let proc_id = Process::<()>::this().id();
    println!("[lunatic-sql {}] calling unroll_vec", proc_id);
    println!(
        "[lunatic-sql {}] called unroll_vec({}, {})",
        proc_id, ptr, len
    );
    unsafe { Vec::from_raw_parts(ptr as *mut u8, len, len) }
}

pub fn last_error(connection_id: u64) -> QueryResult<SqliteError> {
    let mut len_ptr = 0u32;
    unsafe {
        let ptr = sqlite_guest_bindings::last_error(connection_id, &mut len_ptr);
        let encoded_error = unroll_vec(ptr, len_ptr);
        bincode::deserialize(encoded_error.as_slice())
            .map_err(|_| Error::DeserializationError("Failed to deserialize sqlite error".into()))
    }
}

pub fn sqlite3_finalize(statement_id: u64) {
    unsafe {
        sqlite_guest_bindings::sqlite3_finalize(statement_id);
    }
}

pub fn sqlite3_step(statement_id: u64) -> u32 {
    unsafe { sqlite_guest_bindings::sqlite3_step(statement_id) }
}

// pub fn read_column(
//     statement_id: u64,
//     column_idx: u32,
// ) -> QueryResult<lunatic_sqlite_api::SqliteValue> {
//     unsafe {
//         let ptr = sqlite_guest_bindings::read_column(statement_id, column_idx as u32);
//         let encoded_column = unroll_vec(ptr);
//         bincode::deserialize(encoded_column.as_slice())
//             .map_err(|_| Error::DeserializationError("Failed to deserialize sqlite column".into()))
//     }
// }

pub fn read_row(statement_id: u64) -> QueryResult<SqliteRow> {
    unsafe {
        let mut len_ptr = 0u32;
        let proc_id = Process::<()>::this().id();
        let ptr = sqlite_guest_bindings::read_row(statement_id, &mut len_ptr);
        let encoded_row = unroll_vec(ptr, len_ptr);
        println!(
            "[lunatic-sql {}] READING ROW FROM VEC {:?}",
            proc_id, encoded_row
        );
        bincode::deserialize(encoded_row.as_slice()).map_err(|e| {
            eprintln!("Failed to deserialize sqlite row {:?}", e);
            Error::DeserializationError("Failed to deserialize sqlite row".into())
        })
    }
}

// pub fn sqlite3_column_name(statement_id: u64, column_idx: u32) -> QueryResult<String> {
//     unsafe {
//         let ptr = sqlite_guest_bindings::column_name(statement_id, column_idx as u32);
//         let encoded_column_name = unroll_vec(ptr);
//         bincode::deserialize(encoded_column_name.as_slice()).map_err(|_| {
//             Error::DeserializationError("Failed to deserialize sqlite column name".into())
//         })
//     }
// }

// pub fn sqlite3_column_count(statement_id: u64) -> u32 {
//     unsafe { sqlite_guest_bindings::column_count(statement_id) }
// }

pub fn column_names(statement_id: u64) -> QueryResult<Vec<String>> {
    unsafe {
        let mut len_ptr = 0u32;
        let ptr = sqlite_guest_bindings::column_names(statement_id, &mut len_ptr);
        let proc_id = Process::<()>::this().id();
        println!(
            "[lunatic-sql {}] GETTING COLUMN NAMES {} | {}",
            proc_id, len_ptr, ptr
        );
        let encoded_column_name = unroll_vec(ptr, len_ptr);
        println!(
            "[lunatic-sql {}] UNROLLED COLUMN NAME {:?}",
            proc_id, encoded_column_name
        );
        bincode::deserialize(encoded_column_name.as_slice()).map_err(|_| {
            Error::DeserializationError("Failed to deserialize list of sqlite column names".into())
        })
    }
}
