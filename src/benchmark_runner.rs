extern crate regex;

use crate::performance_results::collection_data::BenchmarkData;
use crate::performance_results::collection_data::CollectionData;
use crate::performance_results::collection_data::Mod;
use crate::performance_results::database::upload_to_db;
use crate::util::sha256sum;
use megabase_index_incrementer::FactorioVersion;

use crate::util::{
    download_benchmark_deps_parallel, factorio_executable_path, fbh_mod_dl_dir,
    fbh_mod_use_dir, fbh_save_dl_dir, query_system_cpuid, BenchmarkSet,
    FACTORIO_INFO,
};
use regex::Regex;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fs::read;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::exit;
use std::process::Command;
use std::sync::Mutex;
use std::time::Instant;

static NUMBER_ERROR_CHECKING_TICKS: u32 = 250;
static NUMBER_ERROR_CHECKING_RUNS: u32 = 3;

const STANDARD_VERBOSE_TIMINGS: &str = "wholeUpdate,gameUpdate,\
    circuitNetworkUpdate,transportLinesUpdate,fluidsUpdate,entityUpdate,\
    mapGenerator,electricNetworkUpdate,logisticManagerUpdate,\
    constructionManagerUpdate,pathFinder,trains,trainPathFinder,commander,\
    chartRefresh,luaGarbageIncremental,chartUpdate,scriptUpdate";

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
    static ref VERBOSE_RUN_MARKER_REGEX: Regex = Regex::new("^run ([0-9].*):").unwrap();
    static ref CURRENT_RESAVE_PORT: Mutex<u32> = Mutex::new(31498);
    static ref FACTORIO_VERSION_MATCH_PATTERN: Regex = Regex::new(r"; Factorio ([0-9]*)\.([0-9]*)\.([0-9]*) ").unwrap();
}

#[derive(Debug, Clone)]
pub struct SimpleBenchmarkParams {
    pub map_path: PathBuf,
    pub ticks: u32,
    pub runs: u32,
    pub mod_directory: PathBuf,
    pub mods: Vec<Mod>,
}

impl SimpleBenchmarkParams {
    pub fn new(
        map_path: PathBuf,
        ticks: u32,
        runs: u32,
    ) -> SimpleBenchmarkParams {
        SimpleBenchmarkParams {
            map_path,
            ticks,
            runs,
            mod_directory: fbh_mod_use_dir(),
            mods: Vec::new(),
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

fn parse_logline_time_to_f64(
    find_match_in_this_str: &str,
    regex: &Regex,
) -> Option<f64> {
    match regex.captures(find_match_in_this_str) {
        Some(x) => {
            match GENERIC_NUMERIC_TIMESTAMP_PATTERN.captures(&x[0]).unwrap()[0]
                .parse::<f64>()
            {
                Ok(y) => Some(y),
                Err(e) => {
                    eprintln!(
                        "Internal error occurred, could not parse {} to a f64!",
                        e
                    );
                    None
                }
            }
        }
        None => {
            eprintln!(
                "Internal error, maybe Factorio exited early from outside \
                interference? (parsing line timestamp)"
            );
            eprintln!("Trying to match {:?}", find_match_in_this_str);
            None
        }
    }
}

fn validate_benchmark_set_parameters(set: &BenchmarkSet) {
    assert!(!set.maps.is_empty());
    assert!(set.ticks > 0);
    assert!(set.runs > 0);
}

/// Parses the stdout of a Factorio benchmark for any errors.
fn parse_stdout_for_errors(stdout: &str) {
    if let Some(e) = GENERIC_FACTORIO_ERROR_MATCH_PATTERN.captures(stdout) {
        eprintln!("An error was detected when trying to run Factorio");
        eprintln!("{:?}", &e[0]);
        exit(1);
    }
}

/// Runs multiple benchmark sets, each of which might contain different
/// maps/mods/durations.
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
    stdout: &str,
) -> Option<BenchmarkDurationOverhead> {
    let parsed = parse_stdout_into_benchmark_data(&stdout);
    let mut benchmark_time: BenchmarkDurationOverhead =
        BenchmarkDurationOverhead::default();
    benchmark_time.initialization_time =
        parse_logline_time_to_f64(stdout, &INITIALIZATION_TIME_PATTERN)?;
    benchmark_time.per_tick_time =
        parse_logline_time_to_f64(stdout, &PER_TICK_TIME_PATTERN)?;
    benchmark_time.overall_time =
        parse_logline_time_to_f64(stdout, &TOTAL_TIME_PATTERN)?;
    let time_spent_in_benchmarks =
        benchmark_time.overall_time - benchmark_time.initialization_time;
    if time_spent_in_benchmarks <= 0.0 {
        return None;
    } else {
        //ticks are in ms, convert to sec
        let tick_cumulative_time_per_run =
            benchmark_time.per_tick_time * f64::from(parsed.ticks) / 1000.0;
        benchmark_time.per_run_overhead_time = (time_spent_in_benchmarks
            / f64::from(parsed.runs))
            - tick_cumulative_time_per_run;
    }
    Some(benchmark_time)
}

/// Runs benchmarks on the saves provided in the set. First performs a short
/// error checking pass, and then runs the set's specified parameters.
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
        initial_error_check_params.push(SimpleBenchmarkParams::new(
            save_directory.join(&map.name),
            NUMBER_ERROR_CHECKING_TICKS,
            NUMBER_ERROR_CHECKING_RUNS,
        ));
        set_params.push(SimpleBenchmarkParams::new(
            save_directory.join(&map.name),
            set.ticks,
            set.runs,
        ));
    }
    for param in initial_error_check_params {
        let stdout =
            run_factorio_benchmark(&factorio_executable_path(), &param);
        if let Some(stdout) = stdout {
            parse_stdout_for_errors(&stdout);
            let time_breakdown =
                parse_stdout_for_benchmark_time_breakdown(&stdout);
            if let Some(time) = time_breakdown {
                map_durations.push(time);
            }
        }
    }

    let mut expected_total_game_initialization_time = 0.0;
    let mut expected_total_tick_time = 0.0;
    let mut expected_total_benchmarking_run_overhead = 0.0;
    for a_duration in map_durations {
        expected_total_tick_time +=
            a_duration.per_tick_time * f64::from(set.ticks) / 1000.0
                * f64::from(set.runs);
        expected_total_benchmarking_run_overhead +=
            a_duration.per_run_overhead_time * f64::from(set.runs);
        expected_total_game_initialization_time +=
            a_duration.initialization_time;
    }
    let expected_total_duration = expected_total_tick_time
        + expected_total_game_initialization_time
        + expected_total_benchmarking_run_overhead;
    let now = Instant::now();
    let hrs = (expected_total_duration / 3600.0) as u64;
    let mins = ((expected_total_duration % 3600.0) / 60.0) as u64;
    let secs = (expected_total_duration % 3600.0) % 60.0;
    println!(
        "Measured overhead: ticks {:.*}s, runs {:.*}s, initialization {:.*}s",
        3,
        expected_total_tick_time,
        3,
        expected_total_benchmarking_run_overhead,
        3,
        expected_total_game_initialization_time,
    );
    println!(
        "Benchmark efficiency ({:.*}%)",
        3,
        (expected_total_tick_time / expected_total_duration) * 100.0
    );

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
        let stdout =
            run_factorio_benchmark(&factorio_executable_path(), &param)
                .unwrap();
        parse_stdout_for_errors(&stdout);
        let bench_data = parse_stdout_into_benchmark_data(&stdout);
        collection_data.benchmarks.push(bench_data);
        collection_data.mods.extend(param.mods.clone());
    }

    let total_duration = now.elapsed().as_secs_f64();
    let hrs = (total_duration / 3600.0) as u64;
    let mins = ((total_duration % 3600.0) / 60.0) as u64;
    let secs = (total_duration % 3600.0) % 60.0;
    println!("Benchmarks took: {}:{:02}:{:06.3}", hrs, mins, secs);
    upload_to_db(collection_data);
}

pub fn parse_stdout_for_verbose_data(stdout: &str) -> Vec<String> {
    let mut verbose_data = vec![];
    let mut run_idx: u32 = 0;

    for line in stdout.lines() {
        if VERBOSE_RUN_MARKER_REGEX.is_match(line) {
            run_idx = VERBOSE_RUN_MARKER_REGEX.captures(line).unwrap()[1]
                .parse()
                .unwrap();
        }
        if VERBOSE_DATA_ROW_MATCH_PATTERN.is_match(line) {
            let mut line = line.to_owned();
            line.push_str(&format!("{}", run_idx));
            verbose_data.push(line.replace('t', ""));
            assert!(
                run_idx > 0,
                "Failed to get a run idx?, stdout: {}\nline: {}",
                stdout,
                line
            );
        }
    }
    verbose_data
}

/// Parses stdout and structures it into a BenchmarkData
fn parse_stdout_into_benchmark_data(stdout: &str) -> BenchmarkData {
    trace!("stdout: {}", stdout);
    let verbose_data = parse_stdout_for_verbose_data(&stdout);
    let mut ticks = 0;
    let mut runs = 0;
    let mut map_path = PathBuf::new();

    for line in stdout.lines() {
        if line.contains("Program arguments:") {
            trace!("Processing line: {}", line);
            //TODO broken on maps with spaces in name, probably
            let mut splits = line.split_whitespace().peekable();
            while let Some(word) = splits.next() {
                trace!("Processing word: {}", word);
                if word == "\"--benchmark\"" {
                    trace!("peeking {} from {}", splits.peek().unwrap(), word);
                    if let Some(peek_word) = splits.peek() {
                        map_path = map_path.join(peek_word.replace("\"", ""));
                    }
                }
                if word.contains("--benchmark-ticks") {
                    if let Some(peek_word) = splits.peek() {
                        ticks =
                            peek_word.replace("\"", "").parse::<u32>().unwrap();
                    }
                }
                if word.contains("--benchmark-runs") {
                    if let Some(peek_word) = splits.peek() {
                        runs =
                            peek_word.replace("\"", "").parse::<u32>().unwrap();
                    }
                }
            }
            break;
        }
    }
    info!("Found map {:?}", map_path);

    let map_name = if let Some(file_name) = map_path.file_name() {
        file_name.to_string_lossy().to_string()
    } else {
        panic!("No filename specified for {:?}", map_path);
    };

    let map_hash = sha256sum(map_path);

    info!("Found map hash {:?}", map_hash);

    BenchmarkData {
        map_name,
        map_hash,
        runs,
        ticks,
        verbose_data,
    }
}

pub fn parse_stdout_for_execution_time(stdout: &str) -> Option<f64> {
    let start =
        parse_logline_time_to_f64(&stdout, &INITIALIZATION_TIME_PATTERN)?;
    let end = parse_logline_time_to_f64(&stdout, &TOTAL_TIME_PATTERN)?;
    Some(end - start)
}

fn setup_mod_directory(
    mod_list: &[Mod],
    mod_dir: &Path,
) -> std::io::Result<()> {
    let _ignore_err = std::fs::remove_dir_all(fbh_mod_use_dir());
    for indiv_mod in mod_list {
        let p = mod_dir.join(&indiv_mod.file_name);
        if p.is_file() {
            let computed_sha1 = crate::util::sha1sum(&p);
            if computed_sha1 == indiv_mod.sha1 {
                std::fs::copy(p, mod_dir.join(&indiv_mod.file_name))?;
            } else {
                panic!("Mod {:?} had a mismatched checksum!", indiv_mod);
            }
        }
    }
    Ok(())
}

// Gets the Factorio Version a save was created in by running the save.
pub fn determine_saved_factorio_version(map_path: &Path) -> Option<FactorioVersion> {
    let param = SimpleBenchmarkParams {
        map_path: map_path.to_path_buf(),
        mod_directory: fbh_mod_use_dir(),
        mods: vec![],
        ticks: 1,
        runs: 1,
    };
    let stdout = run_factorio_benchmark(&factorio_executable_path(), &param)?;
    parse_stdout_for_save_version(&stdout)
}

/// Parses the output of a Factorio run for the specific Factorio Version of
/// the executable.
pub fn parse_stdout_for_factorio_version(
    stdout: &str,
) -> Option<FactorioVersion> {
    for line in stdout.lines() {
        if let Some(caps) = FACTORIO_VERSION_MATCH_PATTERN.captures(line) {
            if caps.len() == 4 {
                let fv = FactorioVersion {
                    major: caps[1].parse().ok()?,
                    minor: caps[2].parse().ok()?,
                    patch: caps[3].parse().ok()?,
                };
                return Some(fv);
            }
        }
    }
    None
}

fn parse_stdout_for_save_version(stdout: &str) -> Option<FactorioVersion> {
    for line in stdout.lines().rev() {
        if line.contains("Map version ") {
            // get rid of everything before the version
            let trim_begin = line.split("Map version ").nth(1)?;
            let version_str = trim_begin.split('-').next()?;
            let version = version_str.try_into().ok();
            return version;
        }
    }
    eprintln!("Failure parsing for save version");
    None
}

/// Given a path to a Factorio excutable and a path to a map, runs a Factorio
/// benchmark, optionally returning STDOUT.
pub fn run_factorio_benchmark<P: AsRef<std::ffi::OsStr>>(
    factorio_exe: P,
    params: &SimpleBenchmarkParams,
) -> Option<String> {
    if let Err(e) = setup_mod_directory(&params.mods, &params.mod_directory) {
        eprintln!("Failed to setup mod directory {}", e);
        return None;
    };
    let run_bench_cmd = Command::new(factorio_exe)
        .arg("--benchmark")
        .arg(&params.map_path)
        .arg("--benchmark-ticks")
        .arg(params.ticks.to_string())
        .arg("--benchmark-runs")
        .arg(params.runs.to_string())
        .arg("--benchmark-verbose")
        .arg(STANDARD_VERBOSE_TIMINGS)
        .arg("--mod-directory")
        .arg(&params.mod_directory)
        .output();
    if run_bench_cmd.is_err() {
        eprintln!(
            "An error occurred when attempting to run Factorio: {:?}",
            run_bench_cmd
        );
        None
    } else {
        let run_bench_cmd = run_bench_cmd.ok()?;
        Some(String::from_utf8_lossy(&run_bench_cmd.stdout).replace("\r", ""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::factorio_save_directory;
    #[test]
    fn test_determined_save_version() {
        let testpath = factorio_save_directory().join("copypasta tester.zip");
        let sv = determine_saved_factorio_version(&testpath).unwrap();
        assert_ne!(sv, FactorioVersion::new(0, 0, 0), "Failed to determine saved version.");
    }
}
