use std::collections::HashMap;
use crate::regression_tester::RegressionTestInstance;
use megabase_index_incrementer::FactorioVersion;
use rusqlite::NO_PARAMS;
use std::error::Error;
use crate::regression_tester::RegressionScenario;
use crate::util::fbh_regression_testing_dir;
use std::sync::Mutex;
use std::process::exit;
use rusqlite::Connection;
use std::convert::TryFrom;

const SQL: &str =
r"
CREATE TABLE IF NOT EXISTS `regression_scenario` (
`ID` integer NOT NULL PRIMARY KEY AUTOINCREMENT
,  `start_factorio_version` varchar(100) NOT NULL
,  `platform` varchar(100) NOT NULL
,  `cpuid` varchar(100) NOT NULL
,  `map_name` varchar(100) NOT NULL
,  `sha256` varchar(100) NOT NULL
,  `author` varchar(100) NOT NULL
);

CREATE TABLE IF NOT EXISTS `regression_test_instance` (
`ID` INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT
,  `factorio_version` varchar(100) NOT NULL
,  `runs` integer NOT NULL
,  `ticks` integer NOT NULL
,  `execution_time` real NOT NULL
,  `scenario_ID` integer NOT NULL
,  CONSTRAINT `scenario_fk` FOREIGN KEY (`scenario_ID`) REFERENCES `regression_scenario` (`ID`)
);

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
,  `instance_ID` integer  NOT NULL
,  CONSTRAINT `verbose_scenario_fk` FOREIGN KEY (`instance_ID`)
    REFERENCES `regression_test_instance` (`ID`)
);
";

lazy_static! {
    static ref DB_CONNECTION: Mutex<Connection> =
        Mutex::new(setup_regression_db());
}

fn setup_regression_db() -> Connection {
    let p = fbh_regression_testing_dir().join("regression.db");
    if !p.exists() {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&p)
            .ok();
    }
    let database = rusqlite::Connection::open(&p).unwrap();
    match database.execute_batch(SQL) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Couldn't create tables in db?");
            eprintln!("{}", e);
            exit(1);
        }
    }
    database
}

/// Puts a specific testcase into the database
pub fn put_testcase_to_db(data: RegressionTestInstance, scenario_id: u32) {
    let test_instance_header = "factorio_version,runs,ticks,execution_time,scenario_id";
    let mut db = DB_CONNECTION.lock().unwrap();
    let tx = db.transaction().unwrap();

    let csv_instance = format!("{:?},{:?},{:?},{:?},{:?}",
        data.factorio_version.to_string(),
        data.runs,
        data.ticks,
        data.execution_time,
        scenario_id,
    );
    let combined_sql = format!(
        "INSERT INTO regression_test_instance({}) VALUES ({});",
        test_instance_header, csv_instance
    );
    match tx.execute_batch(&combined_sql) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Failed to insert benchmark data to database!");
            eprintln!("{}", e);
            eprintln!("{:?}", combined_sql);
            exit(1);
        }
    }

    let instance_id = tx.last_insert_rowid() as u32;
    let verbose_header =
        "tick_number,wholeUpdate,gameUpdate,circuitNetworkUpdate,transportLinesUpdate,\
         fluidsUpdate,entityUpdate,mapGenerator,electricNetworkUpdate,logisticManagerUpdate,\
         constructionManagerUpdate,pathFinder,trains,trainPathFinder,commander,chartRefresh,\
         luaGarbageIncremental,chartUpdate,scriptUpdate,run_index,instance_ID";
    let mut combined_sql = String::new();
    for line in data.verbose_data {
        combined_sql.push_str(&format!(
            "INSERT INTO verbose({}) VALUES ({},{});\n",
            verbose_header, line, instance_id
        ));
    }
    match tx.execute_batch(&combined_sql) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Failed to insert verbose data to database!");
            eprintln!("{}", e);
            exit(1);
        }
    }
    tx.commit().unwrap();
}

/// Puts data to the database.
pub fn put_scenario_to_db(data: RegressionScenario) {
    let scenario_id = {
        let collection_header =
            "map_name,start_factorio_version,platform,cpuid,sha256,author";
        let csv_collection = format!(
            "{:?},{:?},{:?},{:?},{:?},{:?}",
            data.map_name,
            data.factorio_version.to_string(),
            data.platform,
            data.cpuid,
            data.sha256,
            data.author,
        );

        let combined_sql = format!(
            "INSERT INTO regression_scenario({}) VALUES ({});",
            collection_header, csv_collection
        );

        let mut db = DB_CONNECTION.lock().unwrap();
        let tx = db.transaction().unwrap();
        match tx.execute_batch(&combined_sql) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Failed to insert regression data to database!");
                eprintln!("{}", e);
                eprintln!("{:?}", combined_sql);
                exit(1);
            }
        }
        let scenario_id = tx.last_insert_rowid() as u32;
        tx.commit().unwrap();
        scenario_id
    };
    for instance in data.test_instances {
        put_testcase_to_db(instance, scenario_id);
    }
}

pub fn get_scenarios() -> Result<HashMap<String, RegressionScenario>, Box<dyn Error>> {
    let db = &*DB_CONNECTION.lock().unwrap();
    let mut stmt = db.prepare(
r"
SELECT ID, platform, cpuid, map_name, sha256, author, start_factorio_version
FROM regression_scenario;
")?;
    let rows = stmt.query_map(NO_PARAMS, |row| {
        Ok(
            RegressionScenario {
                db_id: Some(row.get(0)?),
                platform: row.get(1)?,
                cpuid: row.get(2)?,
                map_name: row.get(3)?,
                sha256: row.get(4)?,
                author: row.get(5)?,
                factorio_version: FactorioVersion::try_from(row.get::<_, String>(6)?.as_ref()).unwrap(),
                versions: Some(Vec::new()),
                test_instances: vec![],
            }
        )
    })?;
    let mut stmt = db.prepare(
        "SELECT factorio_version FROM regression_test_instance where scenario_ID = ?")?;

    let mut mash = HashMap::new();
    for row in rows {
        let mut row = row?;
        let versions_rows = stmt.query_map(&[row.db_id], |row| {
            Ok(
                FactorioVersion::try_from(row.get::<_, String>(0)?.as_ref()).unwrap(),
            )
        })?;
        for ver in versions_rows {
            let ver = ver?;
            if let Some(x) = row.versions.as_mut() { x.push(ver) }
        }
        mash.insert(row.sha256.clone(), row);
    }

    Ok(mash)
}
