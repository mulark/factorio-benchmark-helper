extern crate regex;

use std::fs::read;
use crate::util::{
    get_executable_path,
    BenchmarkSet,
    fetch_benchmark_deps_parallel,
    fbh_save_dl_dir,
    fbh_mod_dl_dir,
    fbh_mod_use_dir,
    upload_collection,
    upload_benchmark,
    upload_verbose,
};
use regex::Regex;
use std::path::PathBuf;
use std::process::Command;
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
}

#[derive(Debug, Clone)]
pub struct SimpleBenchmarkParam {
    pub name: String,
    pub path: PathBuf,
    pub ticks: u32,
    pub runs: u32,
    pub sha256: String,
    pub persist_data_to_db: bool,
    pub collection_id: u32,
}

impl SimpleBenchmarkParam {
    pub fn new(map_path: PathBuf, ticks: u32, runs: u32, persist_data_to_db: bool, sha256: String) -> SimpleBenchmarkParam {
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
            eprintln!("Internal error, maybe Factorio exited early from outside interference?");
            return None;
        }
    };
}

fn validate_benchmark_set(set: &BenchmarkSet) {
    //don't care about pattern anymore
    assert!(!set.maps.is_empty());
    assert!(set.ticks > 0);
    assert!(set.runs > 0);
}

fn parse_stdout_for_errors(stdout: &str) {
    if let Some(e) = GENERIC_FACTORIO_ERROR_MATCH_PATTERN.captures(stdout) {
        eprintln!("An error was detected when trying to run Factorio");
        eprintln!("{:?}", &e[0]);
        std::process::exit(1);
    }
}

pub fn run_benchmarks(procedure: BenchmarkSet) {
    validate_benchmark_set(&procedure);
    fetch_benchmark_deps_parallel(procedure.clone());
    for map in &procedure.maps {
        let fpath = fbh_save_dl_dir().join(map.name.clone());
        assert!(fpath.exists());
    }
    assert!(fbh_mod_use_dir().is_dir());
    if let Ok(dir_list) = std::fs::read_dir(fbh_mod_use_dir()) {
        for file in dir_list {
            if let Ok(f) = file {
                match std::fs::remove_file(f.path()) {
                    Ok(_) => (),
                    _ => {
                        eprintln!("Failed to remove a mod from the staging directory!");
                        std::process::exit(1);
                    }
                }
            }
        }
    }
    for indiv_mod in &procedure.mods {
        let fpath = fbh_mod_dl_dir().join(indiv_mod.name.clone());
        assert!(fpath.exists());
        match std::fs::write(fbh_mod_use_dir().join(indiv_mod.name.clone()), &read(&fpath).unwrap()) {
            Ok(_m) => (),
            _ => {
                eprintln!("Failed to copy mod {:?} for use.", indiv_mod.name);
                std::process::exit(1);
            }
        }
    }
    run_benchmarks_multiple_maps(procedure);
}

fn parse_stdout_for_benchmark_time_breakdown(bench_data_stdout: &str, ticks: u32, runs: u32) -> Option<BenchmarkDurationOverhead> {
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
    return Some(benchmark_time);
}

fn run_benchmarks_multiple_maps(set: BenchmarkSet) {
    let mut map_durations: Vec<BenchmarkDurationOverhead> = Vec::new();
    let mut initial_error_check_params = Vec::new();
    let mut set_params = Vec::new();
    for map in &set.maps {
        initial_error_check_params.push(
            SimpleBenchmarkParam::new(
                fbh_save_dl_dir().join(&map.name),
                NUMBER_ERROR_CHECKING_TICKS,
                NUMBER_ERROR_CHECKING_RUNS,
                false,
                map.sha256.clone(),
            )
        );
        set_params.push(
            SimpleBenchmarkParam::new(
                fbh_save_dl_dir().join(&map.name),
                set.ticks,
                set.runs,
                true,
                map.sha256.clone(),
            )
        );
    }
    for param in initial_error_check_params {
        println!("Checking errors for map: {:?}", param.path);
        let this_duration = run_benchmark_single_map(param, 0);
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
    println!("Measured overhead: ticks {:.*}s ({:.*}%), runs {:.*}s, initialization {:.*}s",
        3, expected_total_tick_time,
        3, (expected_total_tick_time/expected_total_duration)*100.0,
        3, expected_total_benchmarking_run_overhead,
        3, expected_total_game_initialization_time);
    println!(
        "Expecting benchmarks to take: {}:{:02}:{:.3}",
        hrs, mins, secs
    );
    let collection_id = upload_collection();
    for mut param in set_params {
        param.collection_id = collection_id;
        run_benchmark_single_map(param, collection_id);
    }
    let total_duration = now.elapsed().as_secs_f64();
    let hrs = (total_duration / 3600.0) as u64;
    let mins = ((total_duration % 3600.0) / 60.0) as u64;
    let secs = (total_duration % 3600.0) % 60.0;
    println!(
        "Benchmarks took: {}:{:02}:{:.3}",
        hrs, mins, secs
    );
}

fn run_benchmark_single_map(params: SimpleBenchmarkParam, collection_id: u32) -> Option<BenchmarkDurationOverhead> {
    //tick is implied in timings dump
    let verbose_timings =
        "wholeUpdate,gameUpdate,circuitNetworkUpdate,transportLinesUpdate,\
         fluidsUpdate,entityUpdate,mapGenerator,electricNetworkUpdate,logisticManagerUpdate,\
         constructionManagerUpdate,pathFinder,trains,trainPathFinder,commander,chartRefresh,\
         luaGarbageIncremental,chartUpdate,scriptUpdate";
    let benchmark_id = if params.persist_data_to_db { upload_benchmark(params.clone()) } else { 0 };
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
    let bench_data_stdout_raw = String::from_utf8_lossy(&run_bench_cmd.stdout);
    let regex = &Regex::new(params.path.file_name().unwrap().to_str().unwrap()).unwrap();
    let captures = regex.captures(&bench_data_stdout_raw);
    let bench_data_stdout = match captures {
        Some(m) => bench_data_stdout_raw.replace(&m[0], "\n"),
        _ => bench_data_stdout_raw.to_string(),
    };
    parse_stdout_for_errors(&bench_data_stdout);

    let benchmark_times =
        parse_stdout_for_benchmark_time_breakdown(&bench_data_stdout, params.ticks, params.runs);

    let mut run_index = 0;
    let mut line_index = 0;

    let mut verbose_data: Vec<String> = Vec::with_capacity((params.ticks * params.runs) as usize);
    for line in bench_data_stdout.lines() {
        let mut line: String = line.to_string();
        if params.persist_data_to_db && VERBOSE_DATA_ROW_MATCH_PATTERN.is_match(&line) {
            if line_index % 1000 == 0 {
                run_index += 1;
            }
            line.push_str(&format!("{},{}\n", run_index, benchmark_id));
            verbose_data.push(line.to_string());
            line_index += 1;
            assert!(run_index > 0);
        }
    }
    if params.persist_data_to_db {
        //We should have as many lines as ticks have been performed
        assert_eq!((params.ticks * params.runs) as usize, verbose_data.len());
        upload_verbose(verbose_data, benchmark_id, collection_id);
    }
    return benchmark_times;
}
