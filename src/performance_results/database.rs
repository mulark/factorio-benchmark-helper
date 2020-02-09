use crate::performance_results::collection_data::CollectionData;
use crate::util::fbh_results_database;
use rusqlite::Connection;
use std::fs;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Mutex;

pub const CREATE_SQL: &str = "
BEGIN TRANSACTION;
CREATE TABLE IF NOT EXISTS `collection` (
  `collection_id` integer NOT NULL PRIMARY KEY AUTOINCREMENT
,  `name` varchar(100)  NOT NULL
,  `factorio_version` varchar(10)  NOT NULL
,  `platform` varchar(100)  NOT NULL
,  `executable_type` varchar(100)  NOT NULL
,  `cpuid` text NULL
);

CREATE TABLE IF NOT EXISTS `benchmark` (
  `benchmark_id` integer NOT NULL PRIMARY KEY AUTOINCREMENT
,  `map_name` varchar(100)  NOT NULL
,  `runs` integer  NOT NULL CHECK (`runs` > 0)
,  `ticks` integer  NOT NULL
,  `map_hash` char(64)  NOT NULL
,  `collection_id` integer  NOT NULL
,  CONSTRAINT `benchmark_base_ibfk_1` FOREIGN KEY (`collection_id`) REFERENCES `collection` (`collection_id`)
,  CONSTRAINT `hash_length_check` CHECK (length(`map_hash`) = 64)
);
CREATE INDEX IF NOT EXISTS 'idx_benchmark_base_collection_id' ON 'benchmark' (`collection_id`);

CREATE TABLE IF NOT EXISTS `verbose` (
'unused_row_index' integer PRIMARY KEY AUTOINCREMENT
,  `run_index` integer NOT NULL
,  `tick_number` integer NOT NULL
,  `wholeUpdate` integer  NOT NULL
,  `gameUpdate` integer  NOT NULL
,  `circuitNetworkUpdate` integer   NOT NULL
,  `transportLinesUpdate` integer   NOT NULL
,  `fluidsUpdate` integer   NOT NULL
,  `entityUpdate` integer   NOT NULL
,  `mapGenerator` integer   NOT NULL
,  `electricNetworkUpdate` integer   NOT NULL
,  `logisticManagerUpdate` integer   NOT NULL
,  `constructionManagerUpdate` integer   NOT NULL
,  `pathFinder` integer   NOT NULL
,  `trains` integer   NOT NULL
,  `trainPathFinder` integer   NOT NULL
,  `commander` integer   NOT NULL
,  `chartRefresh` integer   NOT NULL
,  `luaGarbageIncremental` integer   NOT NULL
,  `chartUpdate` integer   NOT NULL
,  `scriptUpdate` integer   NOT NULL
,  `benchmark_id` integer  NOT NULL
,  CONSTRAINT `benchmark_verbose_ibfk_1` FOREIGN KEY (`benchmark_id`) REFERENCES `benchmark` (`benchmark_id`)
);

CREATE TABLE IF NOT EXISTS `mods` (
`name` text NOT NULL
,  `version` text NOT NULL
,  `sha1` text NOT NULL
,  UNIQUE(name, version, sha1) ON CONFLICT IGNORE
);

CREATE TABLE IF NOT EXISTS `collection_mods` (
`collection_id` integer NOT NULL
,  `sha1` varchar(100) NOT NULL
,  CONSTRAINT `collection_mods_ibfk_1` FOREIGN KEY (`collection_id`) REFERENCES `collection` (`collection_id`)
);

CREATE VIEW IF NOT EXISTS `v_collection` AS
SELECT collection.collection_id,collection.name,factorio_version,platform,executable_type,cpuid,mods.name,mods.version,mods.sha1
from collection
join collection_mods on collection.collection_id = collection_mods.collection_id
join mods on collection_mods.sha1 = mods.sha1
;

COMMIT;
";

lazy_static! {
    static ref DB_CONNECTION: Mutex<Connection> =
        Mutex::new(setup_database(false, &fbh_results_database()));
}

pub fn setup_database(create_new_db: bool, db_path: &PathBuf) -> Connection {
    if create_new_db {
        fs::remove_file(db_path).ok();
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(db_path)
            .ok();
    }
    let database = rusqlite::Connection::open(db_path).unwrap();
    create_tables_in_db(&database);
    database
}

pub fn upload_to_db(collection_data: CollectionData) {
    let mut database = DB_CONNECTION.lock().unwrap();
    {
        // Not a transaction, but perf should be ok since we're not inserting a LOT of mods
        for indiv_mod in &collection_data.mods {
            //let save_point1 = save_point.savepoint().unwrap();
            let mods_header = "name,version,sha1";
            let combined_sql = format!(
                "INSERT OR IGNORE INTO mods({}) VALUES ({:?},{:?},{:?});",
                mods_header, indiv_mod.name, indiv_mod.version, indiv_mod.sha1,
            );
            match database.execute_batch(&combined_sql) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("Failed to insert mods data to database!");
                    eprintln!("{}", e);
                    eprintln!("{:?}", combined_sql);
                    exit(1);
                }
            }
        }
    }
    let mut transacter = database.transaction().unwrap();

    let collection_header = "name,factorio_version,platform,executable_type,cpuid";
    let csv_collection = format!(
        "{:?},{:?},{:?},{:?},{:?}",
        collection_data.benchmark_name,
        collection_data.factorio_version,
        collection_data.os,
        collection_data.executable_type,
        collection_data.cpuid,
    );

    let combined_sql = format!(
        "INSERT INTO collection({}) VALUES ({});",
        collection_header, csv_collection
    );
    match transacter.execute_batch(&combined_sql) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to insert collection data to database!");
            eprintln!("{}", e);
            eprintln!("{:?}", combined_sql);
            exit(1);
        }
    }
    let collection_id = transacter.last_insert_rowid() as u32;

    for indiv_mod in &collection_data.mods {
        let save_point2 = transacter.savepoint().unwrap();
        match save_point2.execute_named(
            "INSERT INTO collection_mods (collection_id, sha1) VALUES (:collection_id,:sha1)",
            &[
                (":collection_id", &collection_id),
                (":sha1", &indiv_mod.sha1),
            ],
        ) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Failed to insert collection_mods data to database!");
                eprintln!("{}", e);
                exit(1);
            }
        }
        save_point2.commit().unwrap();
    }

    let benchmark_header = "map_name,runs,ticks,map_hash,collection_id";
    for benchmark in collection_data.benchmarks {
        let save_point = transacter.savepoint().unwrap();
        let csv_benchmark = format!(
            "{:?},{:?},{:?},{:?},{:?}",
            benchmark.map_name, benchmark.runs, benchmark.ticks, benchmark.map_hash, collection_id,
        );
        let combined_sql = format!(
            "INSERT INTO benchmark({}) VALUES ({});",
            benchmark_header, csv_benchmark
        );
        match save_point.execute_batch(&combined_sql) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Failed to insert benchmark data to database!");
                eprintln!("{}", e);
                eprintln!("{:?}", combined_sql);
                exit(1);
            }
        }
        let benchmark_id = save_point.last_insert_rowid() as u32;
        let verbose_header =
            "tick_number,wholeUpdate,gameUpdate,circuitNetworkUpdate,transportLinesUpdate,\
             fluidsUpdate,entityUpdate,mapGenerator,electricNetworkUpdate,logisticManagerUpdate,\
             constructionManagerUpdate,pathFinder,trains,trainPathFinder,commander,chartRefresh,\
             luaGarbageIncremental,chartUpdate,scriptUpdate,run_index,benchmark_id";
        let mut combined_sql = String::new();
        for line in benchmark.verbose_data {
            combined_sql.push_str(&format!(
                "INSERT INTO verbose({}) VALUES ({},{});\n",
                verbose_header, line, benchmark_id
            ));
        }
        match save_point.execute_batch(&combined_sql) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Failed to insert verbose data to database!");
                eprintln!("{}", e);
                exit(1);
            }
        }
        save_point.commit().unwrap();
    }
    transacter.commit().unwrap();
}

fn create_tables_in_db(database: &Connection) {
    match database.execute_batch(CREATE_SQL) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Couldn't create sql in db?");
            eprintln!("{}", e);
            exit(1);
        }
    }
}
