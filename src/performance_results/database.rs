use crate::performance_results::collection_data::CollectionData;
use crate::util::fbh_results_database;
use rusqlite::Connection;
use rusqlite::NO_PARAMS;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
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
        // Not a transaction, but perf should be ok since we're not inserting a
        // LOT of mods
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

    let collection_header =
        "name,factorio_version,platform,executable_type,cpuid";
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
            benchmark.map_name,
            benchmark.runs,
            benchmark.ticks,
            benchmark.map_hash,
            collection_id,
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
    print_results(&database, collection_id).unwrap();
}

fn print_results(
    database: &Connection,
    collection_id: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Collection id {}", collection_id);
    let mut ids_to_collect: Vec<(u32, String)> = Vec::new();
    let mut statement = database.prepare(&format!(
        "SELECT benchmark_id, map_name from benchmark where collection_id = {}",
        collection_id
    ))?;
    let rows = statement
        .query_map(NO_PARAMS, |row| {
            let bench_id = row.get(0)?;
            let map_name = row.get(1)?;
            Ok((bench_id, map_name))
        })
        .unwrap();

    for r in rows {
        ids_to_collect.push(r.unwrap());
    }

    assert!(!ids_to_collect.is_empty());

    let mut pivot_statement = String::new();
    pivot_statement
        .push_str(&format!("SELECT id{}.tick_number,\n", ids_to_collect[0].0));
    for id in &ids_to_collect {
        pivot_statement.push_str(&format!(
            "id{}.wholeUpdate as [{}.wholeUpdate],\n",
            id.0, id.1
        ));
    }
    pivot_statement = pivot_statement.trim_end_matches(",\n").to_string();
    pivot_statement.push_str(" FROM \n");
    for id in &ids_to_collect {
        pivot_statement
            .push_str("(SELECT tick_number, min(wholeUpdate) / 1000000.0 as wholeUpdate \n");
        pivot_statement.push_str(&format!(
            "from verbose where benchmark_id = {} group by tick_number) as id{},\n",
            id.0, id.0
        ));
    }
    pivot_statement = pivot_statement.trim_end_matches(",\n").to_string();
    let first_id = ids_to_collect[0].0;
    for (i, id) in ids_to_collect.iter().enumerate() {
        if ids_to_collect.len() != 1 && i == 0 {
            pivot_statement.push_str("\nWHERE ");
        }
        if i != 0 {
            pivot_statement.push_str(&format!(
                "id{}.tick_number = id{}.tick_number ",
                first_id, id.0
            ));
            if i != ids_to_collect.len() - 1 {
                pivot_statement.push_str("AND ");
            }
        }
    }

    let mut csv_file = std::fs::File::create("data.csv")?;

    let mut csv_header = String::from("tick_number,");
    for i in &ids_to_collect {
        csv_header.push_str(&format!(" {}.wholeUpdate,", i.1));
    }

    writeln!(csv_file, "{}", csv_header)?;

    let mut c = database.prepare(&pivot_statement)?;
    let rows = c
        .query_map(NO_PARAMS, |row| {
            assert!(row.column_count() > 0);
            let mut row_writer = String::new();
            let tick_number: u32 = row.get(0)?;
            row_writer.push_str(&format!("{}", tick_number));
            for i in 1..row.column_count() {
                row_writer.push_str(",");
                row_writer.push_str(&format!("{:.3}", row.get::<_, f64>(i)?));
            }
            Ok(row_writer)
        })
        .unwrap();

    for r in rows {
        writeln!(csv_file, "{}", r.unwrap())?;
    }
    Ok(())
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

#[cfg(test)]
mod test {
    use super::print_results;
    use crate::performance_results::collection_data::BenchmarkData;
    use crate::performance_results::collection_data::CollectionData;
    use crate::performance_results::database::upload_to_db;
    use crate::performance_results::database::DB_CONNECTION;
    use crate::util::query_system_cpuid;
    use std::collections::BTreeSet;

    #[test]
    fn test_collection() {
        let data = CollectionData {
            benchmark_name: String::from("TEST"),
            cpuid: query_system_cpuid(),
            executable_type: "TEST".to_owned(),
            factorio_version: "0.0.0".to_owned(),
            os: "TEST".to_owned(),
            mods: BTreeSet::new(),
            benchmarks: vec![BenchmarkData {
                map_hash: "".to_owned(),
                map_name: "TEST".to_owned(),
                runs: 1,
                ticks: 1,
                verbose_data: vec![
                    "1,5000,3000,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1".to_owned(),
                ],
            }],
        };
        let database = DB_CONNECTION.lock().unwrap();
        let collection_id = 1;
        if print_results(&database, collection_id).is_err() {
            upload_to_db(data);
            print_results(&database, collection_id).unwrap();
        }
    }
}
