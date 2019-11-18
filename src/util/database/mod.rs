extern crate rusqlite;

use std::sync::Mutex;
use crate::benchmark_runner::SimpleBenchmarkParam;
use crate::util::query_system_info;
use crate::util::FACTORIO_INFO;
use crate::procedure_file::BenchmarkSet;
use crate::util::fbh_results_database;
use rusqlite::{Connection, NO_PARAMS};
#[macro_use]
use rusqlite::params;
use std::fs;
use std::fs::OpenOptions;
mod create_sql;

lazy_static! {
    static ref DB_CONNECTION: Mutex<Connection> = Mutex::new(setup_database(false));
}

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

pub fn upload_collection() -> u32 {
    let database = DB_CONNECTION.lock().unwrap();
    let collection_header =
        "factorio_version,platform,executable_type,cpuid";
    let factorio_info = FACTORIO_INFO.clone();
    let sys_info = query_system_info();
    let collection_data = format!(
        "{:?},{:?},{:?},{:?}",
        factorio_info.0,
        factorio_info.1,
        factorio_info.2,
        sys_info,
    );
    let combined_sql = format!("INSERT INTO collection({}) VALUES ({})",collection_header, collection_data);
    match database.execute_batch(&combined_sql) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to insert collection data to database!");
            eprintln!("{}",e);
            eprintln!("{:?}", collection_data);
            std::process::exit(1);
        }
    }
    database.last_insert_rowid() as u32
}

pub fn upload_benchmark(params: SimpleBenchmarkParam) -> u32 {
    let database = DB_CONNECTION.lock().unwrap();
    let benchmark_header =
        "map_name,runs,ticks,map_hash,collection_id";
    let combined_sql = format!("INSERT INTO benchmark({}) VALUES (:name,:runs,:ticks,:sha256,:collection_id)",benchmark_header);
    match database.execute_named(
        &combined_sql, &[
            (":name", &params.name),
            (":runs", &params.runs),
            (":ticks", &params.ticks),
            (":sha256", &params.sha256),
            (":collection_id", &params.collection_id),
            ])
        {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to insert benchmark data to database!");
            eprintln!("{}",e);
            database.execute_batch(&format!("DELETE FROM collection where collection_id = {:?}:", params.collection_id)).expect("");
            std::process::exit(1);
        }
    }
    database.last_insert_rowid() as u32
}

pub fn upload_verbose(verbose_data: Vec<String>, benchmark_id: u32, collection_id: u32) {
    let mut database = DB_CONNECTION.lock().unwrap();
    let verbose_header =
        "tick_number,wholeUpdate,gameUpdate,circuitNetworkUpdate,transportLinesUpdate,\
         fluidsUpdate,entityUpdate,mapGenerator,electricNetworkUpdate,logisticManagerUpdate,\
         constructionManagerUpdate,pathFinder,trains,trainPathFinder,commander,chartRefresh,\
         luaGarbageIncremental,chartUpdate,scriptUpdate,run_index,benchmark_id";
    let mut combined_sql = String::from("BEGIN TRANSACTION;");
    for line in verbose_data {
        combined_sql.push_str(&format!("INSERT INTO verbose({}) VALUES ({});\n", verbose_header, line.replace("t","")));
    }
    combined_sql.push_str("COMMIT;\n");
    match database.execute_batch(&combined_sql) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to insert data to database!");
            eprintln!("{}",e);
            database.execute_batch(&format!("DELETE FROM benchmark where benchmark_id = {}", benchmark_id)).expect("");
            database.execute_batch(&format!("DELETE FROM collection where collection_id = {}:", collection_id)).expect("");
            std::process::exit(1);
         }
    }
}

fn create_tables_in_db(database: &Connection) {
    match database.execute_batch(create_sql::CREATE_SQL) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Couldn't create sql in db?");
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}