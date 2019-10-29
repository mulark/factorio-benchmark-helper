extern crate rusqlite;

use crate::util::fbh_results_database;
use rusqlite::Connection;
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::fs;
mod create_sql;

#[derive(Debug)]
pub struct DatabaseUpload {
    pub table_name: String,
    pub table_columns: String,
    pub data_rows: Vec<String>,
}

pub fn setup_database(create_new_db: bool) -> Connection {
    if create_new_db == true {
        fs::remove_file(fbh_results_database()).ok();
        OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(fbh_results_database()).ok();
    }
    let database = rusqlite::Connection::open(fbh_results_database()).unwrap();
    create_tables_in_db(&database);
    return database;
}

pub fn put_data_to_db(_db_upload: DatabaseUpload) {
    //let database = rusqlite::Connection::open(get_database_path()).unwrap();
    //let abc = String::from("INSERT INTO {}");
    //database.
    //database.execute_named(&abc, &[(":db_upload.table_columns", db_upload.data_rows)]);

}

fn create_tables_in_db(database: &Connection) {
    database.execute_batch(create_sql::CREATE_SQL).ok();
}
