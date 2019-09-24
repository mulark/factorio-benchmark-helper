extern crate rusqlite;

use rusqlite::Connection;
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::fs;
mod create_sql;
use super::common;

pub fn setup_database(create_new: bool) -> Connection {
    if create_new == true {
        fs::remove_file(get_database_path()).ok();
        OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(get_database_path());
    }
    let database = rusqlite::Connection::open(get_database_path()).unwrap();
    return database;
}

fn get_database_path() -> PathBuf {
    return common::get_data_path().join("results.db");
}

fn create_tables_in_db(database: Connection) {
    database.execute_batch(create_sql::CREATE_SQL).ok();
}
