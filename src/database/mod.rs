extern crate rusqlite;

use crate::util::fbh_results_database;
use rusqlite::{Connection, NO_PARAMS};
#[macro_use]
use rusqlite::params;
use std::fs;
use std::fs::OpenOptions;
use std::path::PathBuf;
mod create_sql;

#[derive(Debug)]
pub struct BenchmarkResults {
    pub collection_data: String,
    pub benchmark_data: Vec<String>,
    pub verbose_data: Vec<String>,
}

impl BenchmarkResults {
    pub fn new() -> BenchmarkResults {
        BenchmarkResults {
            collection_data: String::new(),
            benchmark_data: Vec::new(),
            verbose_data: Vec::new(),
        }
    }
}

pub fn setup_database(create_new_db: bool) -> Connection {
    if create_new_db {
        fs::remove_file(fbh_results_database()).ok();
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(fbh_results_database())
            .ok();
    }
    let database = rusqlite::Connection::open(fbh_results_database()).unwrap();
    create_tables_in_db(&database);
    return database;
}

pub fn put_data_to_db(data_upload: BenchmarkResults) {
    let database = setup_database(false);
    /*println!("{}",
        database.map(
            "SELECT last_insert_rowid() from benchmark_collection",
             NO_PARAMS,
             |row| row.get(0)
         ).unwrap());*/
    match database.execute
        ("INSERT INTO benchmark_collection values (null, ?1", params!["foobar"]) {
            Ok(r) => (),
            Err(e) => (println!("failed, {}", e)),
        }
    if let Ok(e) = database.execute("SELECT last_insert_rowid() from benchmark_collection", NO_PARAMS) {
        println!("{}",e);
    }
    //let abc = String::from("INSERT INTO {}");
    //database.
    //database.execute_named(&abc, &[(":db_upload.table_columns", db_upload.data_rows)]);
}

fn create_tables_in_db(database: &Connection) {
    database.execute_batch(create_sql::CREATE_SQL).ok();
}
