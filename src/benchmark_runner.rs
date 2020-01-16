extern crate regex;

use crate::util::Mod;
use crate::util::performance_results::{BenchmarkData, CollectionData};
use crate::util::{
    download_benchmark_deps_parallel, fbh_mod_dl_dir, fbh_mod_use_dir, fbh_save_dl_dir,
    get_executable_path, upload_to_db, BenchmarkSet, FACTORIO_INFO, query_system_cpuid,
};
use regex::Regex;
use std::collections::HashMap;
use std::fs::read;
use std::path::PathBuf;
use std::process::exit;
use std::process::Command;
use std::sync::Mutex;
use std::time::Instant;

static NUMBER_ERROR_CHECKING_TICKS: u32 = 300;
static NUMBER_ERROR_CHECKING_RUNS: u32 = 3;

lazy_static! {
    static ref GENERIC_FACTORIO_ERROR_MATCH_PATTERN: Regex = Regex::new(r"[Ee]rror.*\n").unwrap();
    static ref GENERIC_NUMERIC_TIMESTAMP_PATTERN: Regex = Regex::new(r"\d+\.\d{3}").unwrap();
    static ref INITIALIZATION_TIME_PATTERN: Regex = Regex::new("\n .*[0-9].*.[0-9].*Factorio initialised\n").unwrap();
    static ref TOTAL_TIME_PATTERN: Regex = Regex::new("\n .*[0-9].*.[0-9].*Goodbye\n").unwrap();
    static ref PER_TICK_TIME_PATTERN: Regex = Regex::new("avg: [0-9]*.* ms").unwrap();
    //Regexes include ; and : which are not user inputtable via a map.
    static ref MAP_VERSION_MATCH_PATTERN: Regex = Regex::new(r": Map version \d{1,2}\.\d{2,3}\.\d{2,3}").unwrap();
    static ref VERBOSE_COLUMN_HEADER_MATCH_PATTERN: Regex = Regex::new("tick,.*,*\n").unwrap();
    static ref VERBOSE_DATA_ROW_MATCH_PATTERN: Regex = Regex::new("^t[0-9]*[0-9],[0-9]").unwrap();
    static ref CURRENT_RESAVE_PORT: Mutex<u32> = Mutex::new(31498);
}

#[derive(Debug, Clone)]
pub struct SimpleBenchmarkParam {
    pub name: String,
    pub path: PathBuf,
    pub ticks: u32,
    pub runs: u32,
    pub sha256: String,
    pub persist_data_to_db: PersistDataToDB,
    pub collection_id: u32,
}

impl SimpleBenchmarkParam {
    pub fn new(
        map_path: PathBuf,
        ticks: u32,
        runs: u32,
        persist_data_to_db: PersistDataToDB,
        sha256: String,
    ) -> SimpleBenchmarkParam {
        SimpleBenchmarkParam {
            name: map_path.file_name().unwrap().to_string_lossy().to_string(),
            path: map_path,
            ticks,
            runs,
            persist_data_to_db,
            sha256,
            collection_id: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PersistDataToDB {
    True,
    False,
}

#[derive(Debug)]
struct BenchmarkDurationOverhead {
    initialization_time: f64,
    per_tick_time: f64,
    per_run_overhead_time: f64,
    overall_time: f64,
}

impl Default for BenchmarkDurationOverhead {
    fn default() -> Self {
        Self {
            initialization_time: 0.0,
            per_tick_time: 0.0,
            per_run_overhead_time: 0.0,
            overall_time: 0.0,
        }
    }
}

fn parse_logline_time_to_f64(find_match_in_this_str: &str, regex: Regex) -> Option<f64> {
    match regex.captures(find_match_in_this_str) {
        Some(x) => {
            match GENERIC_NUMERIC_TIMESTAMP_PATTERN.captures(&x[0]).unwrap()[0].parse::<f64>() {
                Ok(y) => return Some(y),
                Err(e) => {
                    eprintln!("Internal error occurred, could not parse {} to a f64!", e);
                    return None;
                }
            }
        }
        None => {
            eprintln!("Internal error, maybe Factorio exited early from outside interference? (parsing line timestamp)");
            eprintln!("Trying to match {:?}", find_match_in_this_str);
            return None;
        }
    };
}

fn validate_benchmark_set_parameters(set: &BenchmarkSet) {
    //don't care about pattern anymore
    assert!(!set.maps.is_empty());
    assert!(set.ticks > 0);
    assert!(set.runs > 0);
}

fn parse_stdout_for_errors(stdout: &str) {
    if let Some(e) = GENERIC_FACTORIO_ERROR_MATCH_PATTERN.captures(stdout) {
        eprintln!("An error was detected when trying to run Factorio");
        eprintln!("{:?}", &e[0]);
        exit(1);
    }
}

pub fn run_benchmarks_multiple(sets: HashMap<String, BenchmarkSet>) {
    download_benchmark_deps_parallel(&sets);
    for (name, set) in sets {
        validate_benchmark_set_parameters(&set);
        let save_directory = if let Some(subdir) = &set.save_subdirectory {
            fbh_save_dl_dir().join(subdir)
        } else {
            fbh_save_dl_dir()
        };
        for map in &set.maps {
            let fpath = save_directory.join(&map.name);
            assert!(fpath.exists());
        }
        assert!(fbh_mod_use_dir().is_dir());
        run_factorio_benchmarks_from_set(&name, set);
    }
}

fn parse_stdout_for_benchmark_time_breakdown(
    bench_data_stdout: &str,
    ticks: u32,
    runs: u32,
) -> Option<BenchmarkDurationOverhead> {
    let mut benchmark_time: BenchmarkDurationOverhead = BenchmarkDurationOverhead::default();
    benchmark_time.initialization_time =
        parse_logline_time_to_f64(bench_data_stdout, INITIALIZATION_TIME_PATTERN.clone())?;
    benchmark_time.per_tick_time =
        parse_logline_time_to_f64(bench_data_stdout, PER_TICK_TIME_PATTERN.clone())?;
    benchmark_time.overall_time =
        parse_logline_time_to_f64(bench_data_stdout, TOTAL_TIME_PATTERN.clone())?;
    let time_spent_in_benchmarks = benchmark_time.overall_time - benchmark_time.initialization_time;
    if time_spent_in_benchmarks <= 0.0 {
        return None;
    } else {
        //ticks are in ms, convert to sec
        let tick_cumulative_time_per_run = benchmark_time.per_tick_time * f64::from(ticks) / 1000.0;
        benchmark_time.per_run_overhead_time =
            (time_spent_in_benchmarks / f64::from(runs)) - tick_cumulative_time_per_run;
    }
    Some(benchmark_time)
}

fn run_factorio_benchmarks_from_set(set_name: &str, set: BenchmarkSet) {
    let mut map_durations: Vec<BenchmarkDurationOverhead> = Vec::new();
    let mut initial_error_check_params = Vec::new();
    let mut set_params = Vec::new();
    let save_directory = if let Some(subdir) = &set.save_subdirectory {
        fbh_save_dl_dir().join(subdir)
    } else {
        fbh_save_dl_dir()
    };
    if let Ok(dir_list) = std::fs::read_dir(fbh_mod_use_dir()) {
        for dir_entry_result in dir_list {
            if let Ok(dir_entry) = dir_entry_result {
                match std::fs::remove_file(dir_entry.path()) {
                    Ok(_) => (),
                    _ => {
                        eprintln!("Failed to remove a mod from the staging directory!");
                        exit(1);
                    }
                }
            }
        }
    }
    for indiv_mod in &set.mods {
        let mod_filename = if indiv_mod.file_name.is_empty() {
            format!("{}_{}.zip", indiv_mod.name, indiv_mod.version)
        } else {
            indiv_mod.file_name.clone()
        };
        let cached_mods_dir = fbh_mod_dl_dir().join(&mod_filename);
        let mods_use_dir = fbh_mod_use_dir().join(&mod_filename);
        assert!(cached_mods_dir.exists());
        match std::fs::write(&mods_use_dir, read(&cached_mods_dir).unwrap()) {
            Ok(_m) => (),
            _ => {
                eprintln!("Failed to copy mod {:?} for use.", &mod_filename);
                exit(1);
            }
        }
    }
    for map in &set.maps {
        initial_error_check_params.push(SimpleBenchmarkParam::new(
            save_directory.join(&map.name),
            NUMBER_ERROR_CHECKING_TICKS,
            NUMBER_ERROR_CHECKING_RUNS,
            PersistDataToDB::False,
            map.sha256.clone(),
        ));
        set_params.push(SimpleBenchmarkParam::new(
            save_directory.join(&map.name),
            set.ticks,
            set.runs,
            PersistDataToDB::True,
            map.sha256.clone(),
        ));
    }
    for param in initial_error_check_params {
        println!("Checking errors for map: {:?}", param.path);
        let this_duration = run_benchmark_single_map(param, None, &set.mods);
        if let Some(duration) = this_duration {
            map_durations.push(duration)
        }
    }

    let mut expected_total_game_initialization_time = 0.0;
    let mut expected_total_tick_time = 0.0;
    let mut expected_total_benchmarking_run_overhead = 0.0;
    for a_duration in map_durations {
        expected_total_tick_time +=
            a_duration.per_tick_time * f64::from(set.ticks) / 1000.0 * f64::from(set.runs);
        expected_total_benchmarking_run_overhead +=
            a_duration.per_run_overhead_time * f64::from(set.runs);
        expected_total_game_initialization_time += a_duration.initialization_time;
    }
    let expected_total_duration = expected_total_tick_time
        + expected_total_game_initialization_time
        + expected_total_benchmarking_run_overhead;
    let now = Instant::now();
    let hrs = (expected_total_duration / 3600.0) as u64;
    let mins = ((expected_total_duration % 3600.0) / 60.0) as u64;
    let secs = (expected_total_duration % 3600.0) % 60.0;
    println!("Measured overhead: ticks {:.*}s, runs {:.*}s, initialization {:.*}s",
        3, expected_total_tick_time,
        3, expected_total_benchmarking_run_overhead,
        3, expected_total_game_initialization_time,

    );
    println!("Benchmark efficiency ({:.*}%)", 3, (expected_total_tick_time/expected_total_duration)*100.0);

    // 0 pad 2 characters if no decimals wanted
    // 0 pad 6 characters for 3 decimal place seconds, since '.' counts as a character too.
    println!(
        "Expecting benchmarks to take: {}:{:02}:{:06.3}",
        hrs, mins, secs
    );

    let mut collection_data = CollectionData::default();
    collection_data.benchmark_name = set_name.to_string();

    let info = FACTORIO_INFO.clone();
    collection_data.factorio_version = info.version;
    collection_data.os = info.operating_system;
    collection_data.executable_type = info.platform;
    collection_data.cpuid = query_system_cpuid();

    for param in set_params {
        run_benchmark_single_map(param, Some(&mut collection_data), &set.mods);
    }

    let total_duration = now.elapsed().as_secs_f64();
    let hrs = (total_duration / 3600.0) as u64;
    let mins = ((total_duration % 3600.0) / 60.0) as u64;
    let secs = (total_duration % 3600.0) % 60.0;
    println!("Benchmarks took: {}:{:02}:{:06.3}", hrs, mins, secs);
    upload_to_db(collection_data);
}

fn run_benchmark_single_map(
    params: SimpleBenchmarkParam,
    collection_data: Option<&mut CollectionData>,
    mods: &[Mod],
) -> Option<BenchmarkDurationOverhead> {
    //tick is implied in timings dump
    let mut bench_data = BenchmarkData::default();
    {
        bench_data.map_name = params.name;
        bench_data.runs = params.runs;
        bench_data.ticks = params.ticks;
        bench_data.map_hash = params.sha256;
    }

    let verbose_timings =
        "wholeUpdate,gameUpdate,circuitNetworkUpdate,transportLinesUpdate,\
         fluidsUpdate,entityUpdate,mapGenerator,electricNetworkUpdate,logisticManagerUpdate,\
         constructionManagerUpdate,pathFinder,trains,trainPathFinder,commander,chartRefresh,\
         luaGarbageIncremental,chartUpdate,scriptUpdate";

    let run_bench_cmd = Command::new(get_executable_path())
        .arg("--benchmark")
        .arg(&params.path)
        .arg("--benchmark-ticks")
        .arg(params.ticks.to_string())
        .arg("--benchmark-runs")
        .arg(params.runs.to_string())
        .arg("--benchmark-verbose")
        .arg(verbose_timings)
        .arg("--mod-directory")
        .arg(fbh_mod_use_dir().to_str().unwrap())
        .output()
        .expect("");

    if let Ok(entries) = std::fs::read_dir(fbh_mod_use_dir()) {
        // Number of entries == Count of all mods that should be enabled + the mod-list.json file
        // Thus, stubtract one to account for mod-list.json
        assert_eq!(entries.count() - 1, mods.len());
    }

    let bench_data_stdout_raw = String::from_utf8_lossy(&run_bench_cmd.stdout).replace("\r", "");
    let regex = &Regex::new(params.path.file_name().unwrap().to_str().unwrap()).unwrap();
    let captures = regex.captures(&bench_data_stdout_raw);
    let bench_data_stdout = match captures {
        Some(m) => bench_data_stdout_raw.replace(&m[0], "\n"),
        _ => bench_data_stdout_raw.to_string(),
    };
    parse_stdout_for_errors(&bench_data_stdout);

    let benchmark_times = if params.persist_data_to_db == PersistDataToDB::False {
        parse_stdout_for_benchmark_time_breakdown(&bench_data_stdout, params.ticks, params.runs)
    } else {
        None
    };

    let mut run_index = 0;
    let mut line_index = 0;

    let mut verbose_data: Vec<String> = Vec::with_capacity((params.ticks * params.runs) as usize);
    for line in bench_data_stdout.lines() {
        let mut line: String = line.to_string();
        if params.persist_data_to_db == PersistDataToDB::True
            && VERBOSE_DATA_ROW_MATCH_PATTERN.is_match(&line)
        {
            if line_index % params.ticks == 0 {
                run_index += 1;
            }
            line.push_str(&format!("{}", run_index));
            verbose_data.push(line.replace('t', ""));
            line_index += 1;
            assert!(run_index > 0);
        }
    }
    if params.persist_data_to_db == PersistDataToDB::True {
        //We should have as many lines as ticks have been performed
        assert_eq!((params.ticks * params.runs) as usize, verbose_data.len());
        bench_data.verbose_data = verbose_data;
        let collection_data = collection_data.unwrap();
        collection_data.mods = mods.to_vec();
        collection_data.benchmarks.push(bench_data);
    }
    benchmark_times
}
