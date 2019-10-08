extern crate regex;

use regex::Regex;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Instant};
use super::database::{self, DatabaseUpload};

static NUMBER_ERROR_CHECKING_TICKS: u32 = 300;
static NUMBER_ERROR_CHECKING_RUNS: u32 = 2;

lazy_static! {
    static ref GENERIC_FACTORIO_ERROR_MATCH_PATTERN: Regex = Regex::new("\n*[Ee]rror.*\n").unwrap();
    static ref GENERIC_NUMERIC_TIMESTAMP_PATTERN: Regex = Regex::new(r"\d+\.\d{3}").unwrap();
    static ref INITIALIZATION_TIME_PATTERN: Regex = Regex::new("\n .*[0-9].*.[0-9].*Factorio initialised\n").unwrap();
    static ref TOTAL_TIME_PATTERN: Regex = Regex::new("\n .*[0-9].*.[0-9].*Goodbye\n").unwrap();
    static ref PER_TICK_TIME_PATTERN: Regex = Regex::new("avg: [0-9]*.* ms").unwrap();
}

#[derive(Debug)]
pub struct BenchmarkParams {
    pub match_pattern: String,
    //ticks done as vector to allow 1 map to run for various durations.
    pub ticks: u32,
    pub runs: u32,
    pub maps: Vec<PathBuf>,
}

impl Default for BenchmarkParams {
    fn default() -> Self {
        Self{
            match_pattern: "".to_string(),
            ticks: 0,
            runs: 0,
            maps: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct BenchmarkDuration {
    initialization_time: f64,
    per_tick_time: f64,
    per_run_overhead_time: f64,
    overall_time: f64,
}

impl Default for BenchmarkDuration {
    fn default() -> Self {
        Self{
            initialization_time: 0.0,
            per_tick_time: 0.0,
            per_run_overhead_time: 0.0,
            overall_time: 0.0,
        }
    }
}

fn parse_logline_time_to_f64(find_match_in_this_str: &str, regex: Regex) -> Option<f64>{
    match regex.captures(find_match_in_this_str) {
        Some(x) => {
            match GENERIC_NUMERIC_TIMESTAMP_PATTERN.captures(&x[0]).unwrap()[0].parse::<f64>() {
                Ok(y) => return Some(y),
                Err(e) => {
                    println!("Internal error occurred, could not parse {} to a f64!", e);
                    return None;
                }
            }
        },
        None => {
            println!("Internal error, maybe Factorio failed to initialize or exited early?");
            return None;
        }
    };
}

fn validate_benchmark_params(params: &BenchmarkParams) {
    //don't care about pattern
    assert!(params.maps.len() > 0);
    for map in &params.maps {
        assert!(&map.exists());
    }
    assert!(params.ticks > 0);
    assert!(params.runs > 0);
}

fn parse_stdout_for_errors(stdout: &str) {
    match GENERIC_FACTORIO_ERROR_MATCH_PATTERN.captures(stdout) {
        Some(e) => {
            println!("An Error was detected when trying to run Factorio");
            println!("{:?}",e);
            std::process::exit(1);
        },
        _ => (),
    }
}

fn parse_stdout_for_benchmark_time(bench_data_stdout: &str, ticks: u32, runs: u32) -> Option<BenchmarkDuration> {
    let mut benchmark_time: BenchmarkDuration = BenchmarkDuration::default();
    benchmark_time.initialization_time = parse_logline_time_to_f64(bench_data_stdout, INITIALIZATION_TIME_PATTERN.clone())?;
    benchmark_time.per_tick_time = parse_logline_time_to_f64(bench_data_stdout, PER_TICK_TIME_PATTERN.clone())?;
    benchmark_time.overall_time = parse_logline_time_to_f64(bench_data_stdout, TOTAL_TIME_PATTERN.clone())?;
    let time_spent_in_benchmarks = benchmark_time.overall_time - benchmark_time.initialization_time;
    if time_spent_in_benchmarks <= 0.0 {
        return None
    } else {
        //ticks are in ms, convert to sec
        let tick_cumulative_time_per_run = benchmark_time.per_tick_time * ticks as f64 / 1000.0;
        benchmark_time.per_run_overhead_time = (time_spent_in_benchmarks / runs as f64) - tick_cumulative_time_per_run;
    }

    return Some(benchmark_time);
}

pub fn run_benchmarks(params: &BenchmarkParams, executable_path: &PathBuf) {
    validate_benchmark_params(params);
    let mut map_durations: Vec<BenchmarkDuration> = Vec::new();
    for map in &params.maps {
        println!("Checking errors for map: {}", map.to_string_lossy());
        let this_duration = run_benchmark_single_map(&map, NUMBER_ERROR_CHECKING_TICKS, NUMBER_ERROR_CHECKING_RUNS, &executable_path, false);
        match this_duration {
            Some(duration) => map_durations.push(duration),
            None => (),
        }
    }

    let mut expected_total_game_initialization_time = 0.0;
    let mut expected_total_tick_time = 0.0;
    let mut expected_total_benchmarking_run_overhead = 0.0;
    for a_duration in map_durations {
        expected_total_tick_time += a_duration.per_tick_time * params.ticks as f64 / 1000.0 * params.runs as f64;
        expected_total_benchmarking_run_overhead += a_duration.per_run_overhead_time * params.runs as f64;
        expected_total_game_initialization_time += a_duration.initialization_time;
    }
    let expected_total_duration = expected_total_tick_time + expected_total_game_initialization_time + expected_total_benchmarking_run_overhead;
    let now = Instant::now();
    println!("Expecting benchmarks to take: {:.*}s", 3, expected_total_duration);
    println!("Overhead: ticks {:.*}s, runs {:.*}s, initialization {:.*}s", 3, expected_total_tick_time, 3, expected_total_benchmarking_run_overhead, 3, expected_total_game_initialization_time);
    for map in &params.maps {
        run_benchmark_single_map(&map, params.ticks, params.runs, &executable_path, true);
    }
    println!("Took {}s to get here.", now.elapsed().as_secs_f64());
}

fn run_benchmark_single_map(map: &PathBuf, ticks: u32, runs: u32, executable_path: &PathBuf, upload_result: bool) -> Option::<BenchmarkDuration> {
    //tick is implied in timings dump
    let verbose_timings = "wholeUpdate,gameUpdate,circuitNetworkUpdate,transportLinesUpdate,
        fluidsUpdate,entityUpdate,mapGenerator,electricNetworkUpdate,logisticManagerUpdate,
        constructionManagerUpdate,pathFinder,trains,trainPathFinder,commander,chartRefresh,
        luaGarbageIncremental,chartUpdate,scriptUpdate";
    let run_bench_cmd = Command::new(executable_path)
        .arg("--benchmark")
        .arg(map)
        .arg("--benchmark-ticks")
        .arg(ticks.to_string())
        .arg("--benchmark-runs")
        .arg(runs.to_string())
        .arg("--benchmark-verbose")
        .arg(verbose_timings)
        .output()
        .expect("");
    let bench_data_stdout = String::from_utf8_lossy(&run_bench_cmd.stdout);

    //unwrap() should be safe since &str slices are const
    let verbose_column_header_match_pattern = Regex::new("tick,.*,*\n").unwrap();
    let verbose_data_row_match_pattern = Regex::new("^t[0-9]*[0-9],[0-9]").unwrap();

    parse_stdout_for_errors(&bench_data_stdout);
    let benchmark_times = parse_stdout_for_benchmark_time(&bench_data_stdout, ticks, runs);

    let mut run_index = 0;
    let mut line_index = 0;
    let mut column_headers: String = verbose_column_header_match_pattern.captures(&bench_data_stdout).unwrap()[0].to_string();
    column_headers.pop(); //Remove \n
    column_headers.push_str("run_index,");
    column_headers.push_str("benchmark_id,\n");

    let mut verbose_data: Vec<String> = Vec::with_capacity((ticks * runs) as usize);
    for line in bench_data_stdout.lines() {
        let mut line: String = line.to_string();
        if GENERIC_FACTORIO_ERROR_MATCH_PATTERN.is_match(&line) {
            eprintln!("An error was detected when trying to run a benchmark");
            eprintln!("{}",line);
            panic!();
        }
        if upload_result {
            if verbose_data_row_match_pattern.is_match(&line) {
                if line_index % 1000 == 0 {
                    run_index = run_index + 1;
                }
                line.push_str(&format!("{},",run_index));
                verbose_data.push(line.to_string());
                line_index = line_index + 1;
                assert!(run_index > 0);
            }
        }
    }
    if upload_result {
        assert_eq!((ticks * runs) as usize, verbose_data.len());
        let db_input = DatabaseUpload{table_name:String::from("benchmark_verbose"), table_columns:column_headers, data_rows:verbose_data};
        database::put_data_to_db(db_input);
    }
    return benchmark_times;
}
