extern crate rusqlite;

use rusqlite::Connection;
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::fs;
mod create_sql;
use super::common;


#[derive(Debug)]
pub struct DatabaseUpload {
    pub table_name: String,
    pub table_columns: String,
    pub data_rows: Vec<String>,
}

pub fn setup_database(create_new: bool) -> Connection {
    if create_new == true {
        fs::remove_file(get_database_path()).ok();
        OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(get_database_path()).ok();
    }
    let database = rusqlite::Connection::open(get_database_path()).unwrap();
    create_tables_in_db(&database);
    return database;
}

pub fn put_data_to_db(_db_upload: DatabaseUpload) {
    //let database = rusqlite::Connection::open(get_database_path()).unwrap();
    //let abc = String::from("INSERT INTO {}");
    //database.
    //database.execute_named(&abc, &[(":db_upload.table_columns", db_upload.data_rows)]);

}

fn get_database_path() -> PathBuf {
    return common::get_fbh_data_path().join("results.db");
}

fn create_tables_in_db(database: &Connection) {
    database.execute_batch(create_sql::CREATE_SQL).ok();
}
