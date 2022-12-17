use std::path::Path;

use diesel::{result::Error, QueryResult};
use lunatic_sqlite_api::sqlite_guest_bindings;

pub use lunatic_sqlite_api::sqlite_guest_bindings::*;
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

pub fn bind_value(statement_id: u64, value: lunatic_sqlite_api::BindPair) {
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
fn unroll_vec(ptr: u64) -> Vec<u8> {
    unsafe {
        let length = (ptr >> 32) as usize;
        // cut off the bytes on the left
        let ptr = 0x00000000FFFFFFFF & ptr;
        Vec::from_raw_parts(ptr as *mut u8, length, length)
    }
}

pub fn last_error(connection_id: u64) -> QueryResult<lunatic_sqlite_api::SqliteError> {
    unsafe {
        let ptr = sqlite_guest_bindings::last_error(connection_id);
        let encoded_error = unroll_vec(ptr);
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

pub fn read_row(statement_id: u64) -> QueryResult<lunatic_sqlite_api::SqliteRow> {
    unsafe {
        let ptr = sqlite_guest_bindings::read_row(statement_id);
        let encoded_row = unroll_vec(ptr);
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
        let ptr = sqlite_guest_bindings::column_names(statement_id);
        let encoded_column_name = unroll_vec(ptr);
        bincode::deserialize(encoded_column_name.as_slice()).map_err(|_| {
            Error::DeserializationError("Failed to deserialize list of sqlite column names".into())
        })
    }
}

pub fn set_custom_guest_allocator(connection_id: u64, allocator_name: &str) -> QueryResult<()> {
    unsafe {
        sqlite_guest_bindings::set_custom_guest_allocator(
            connection_id,
            allocator_name.as_ptr() as u32,
            allocator_name.len() as u32,
        );
    }
    Ok(())
}
