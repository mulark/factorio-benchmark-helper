extern crate regex;

use crate::util::query_system_cpuid;
use crate::util::FACTORIO_INFO;
use crate::util::performance_results::{CollectionData, BenchmarkData};
use std::sync::Mutex;
use std::fs::remove_dir_all;
use std::fs::read_to_string;
use std::io::Write;
use std::fs::remove_file;
use crate::util::fbh_resave_dir;
use std::collections::HashMap;
use std::process::exit;
use std::fs::{read, File};
use crate::util::{
    get_executable_path,
    BenchmarkSet,
    download_benchmark_deps_parallel,
    fbh_save_dl_dir,
    fbh_mod_dl_dir,
    fbh_mod_use_dir,
    upload_to_db,
};
use regex::Regex;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Instant, Duration};
#[cfg(target_os = "linux")]
use nix::unistd::Pid;
#[cfg(target_os = "linux")]
use nix::sys::signal::{kill, Signal};
#[cfg(target_os = "linux")]
use nix::sys::wait::WaitStatus;

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
    pub fn new(map_path: PathBuf, ticks: u32, runs: u32, persist_data_to_db: PersistDataToDB, sha256: String) -> SimpleBenchmarkParam {
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

#[cfg(target_os = "linux")]
pub fn auto_resave(file_to_resave: PathBuf) -> Result<bool,std::io::Error> {
    println!("resaving {:?}", file_to_resave);
    if !cfg!(target_os = "linux") {
        panic!("auto_resave is not supported on Windows!");
    }
    if !fbh_resave_dir().exists() {
        std::fs::create_dir(fbh_resave_dir())?;
    }
    let local_config_file_path = fbh_resave_dir().join(format!("{}{}", file_to_resave.file_name().unwrap().to_str().unwrap(), ".ini"));
    let mut local_config_file = File::create(&local_config_file_path).unwrap();
    let local_write_dir = fbh_resave_dir().join(file_to_resave.file_name().unwrap()).join("");
    let local_mods_dir = local_write_dir.join("mods");
    let local_logfile = local_write_dir.join("factorio-current.log");
    writeln!(local_config_file, "[path]")?;
    writeln!(local_config_file, "read-data=__PATH__system-read-data__")?;
    writeln!(local_config_file, "write-data={}", local_write_dir.to_str().unwrap())?;
    writeln!(local_config_file, "[other]")?;
    writeln!(local_config_file, "autosave-compression-level=maximum")?;
    let port: u32;
    {
        let mut data = CURRENT_RESAVE_PORT.lock().unwrap();
        *data += 1;
        port = *data;
    }
    writeln!(local_config_file, "port={}", port)?;
    let child = Command::new(get_executable_path())
        .arg("--config")
        .arg(&local_config_file_path)
        .arg("--start-server")
        .arg(&file_to_resave)
        .arg("--mod-directory")
        .arg(local_mods_dir)
        .stdout(Stdio::null())
        .spawn()?;
    let pid = Pid::from_raw(child.id() as i32);
    let mut clean = false;
    std::thread::sleep(Duration::from_millis(500));
    let mut file_text;
    let expire = Instant::now() + Duration::from_millis(30000);
    loop {
        //keep reading logfile until it's safe to send a SIGINT, or we fail, or we timeout.
        if Instant::now() > expire {
            //Incase the logfile never exists or never contains the lines we're looking for
            eprintln!("Timed out during busy loop waiting for log file to become ready.");
            exit(1);
        }
        if local_logfile.exists() {
            file_text = read_to_string(&local_logfile).unwrap();
            if file_text.contains("Loading script.dat") {
                break;
            }
            if file_text.contains("Error") || file_text.contains("Failed") {
                eprintln!("An error was detected trying to resave maps.");
                eprintln!("Here is the factorio output for this moment.");
                eprintln!("{}", file_text);
                exit(1);
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    if let Ok(()) = kill(pid, Signal::SIGINT) {
        let mut do_timeout = true;
        let mut last_line_content: String = "".to_string();
        while !last_line_content.contains("Goodbye") {
            let local_logfile = local_logfile.clone();
            if local_logfile.exists() {
                let read_buf = read_to_string(local_logfile).unwrap();
                let logfile_contents: Vec<_> = read_buf.lines().collect();
                last_line_content = logfile_contents[logfile_contents.len() - 1].to_string();
            }
            if last_line_content.contains("Saving progress:") {
                do_timeout = false;
            }
            if do_timeout && Instant::now() > expire {
                //if we have passed the timeout threshold and did not see the game being saved, exit uncleanly.
                eprintln!("A resave thread timed out");
                break;
            }
            if last_line_content.contains("Goodbye") {
                clean = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        if clean {
            remove_dir_all(local_write_dir)?;
            remove_file(local_config_file_path)?;
        } else {
            eprintln!("Child did not cleanly exit for {:?}", file_to_resave);
            if let Ok(WaitStatus::StillAlive) = nix::sys::wait::waitpid(pid, None) {
                let res = kill(pid, Signal::SIGKILL);
                println!("{:?}", res);
            } else {
                panic!("Wasnt stillalive?");
            }
        }
    }
    Ok(clean)
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
    for (name, mut set) in sets {
        validate_benchmark_set_parameters(&set);
        for map in &set.maps {
            let fpath = fbh_save_dl_dir().join(map.name.clone());
            assert!(fpath.exists());
        }
        assert!(fbh_mod_use_dir().is_dir());
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
        for indiv_mod in &mut set.mods {
            if indiv_mod.file_name.is_empty() {
                indiv_mod.file_name = format!("{}_{}.zip", indiv_mod.name, indiv_mod.version);
            }
            let fpath = fbh_mod_dl_dir().join(&indiv_mod.file_name);
            assert!(fpath.exists());
            match std::fs::write(fbh_mod_use_dir().join(&indiv_mod.file_name), &read(&fpath).unwrap()) {
                Ok(_m) => (),
                _ => {
                    eprintln!("Failed to copy mod {:?} for use.", indiv_mod.file_name);
                    exit(1);
                }
            }
        }
        run_factorio_benchmarks_from_set(&name, set);
    }
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
    Some(benchmark_time)
}

fn run_factorio_benchmarks_from_set(set_name: &str, set: BenchmarkSet) {
    let mut map_durations: Vec<BenchmarkDurationOverhead> = Vec::new();
    let mut initial_error_check_params = Vec::new();
    let mut set_params = Vec::new();
    for map in &set.maps {
        initial_error_check_params.push(
            SimpleBenchmarkParam::new(
                fbh_save_dl_dir().join(&map.name),
                NUMBER_ERROR_CHECKING_TICKS,
                NUMBER_ERROR_CHECKING_RUNS,
                PersistDataToDB::False,
                map.sha256.clone(),
            )
        );
        set_params.push(
            SimpleBenchmarkParam::new(
                fbh_save_dl_dir().join(&map.name),
                set.ticks,
                set.runs,
                PersistDataToDB::True,
                map.sha256.clone(),
            )
        );
    }
    for param in initial_error_check_params {
        println!("Checking errors for map: {:?}", param.path);
        let this_duration = run_benchmark_single_map(param, None);
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
        "Expecting benchmarks to take: {}:{:02}:{:02.3}",
        hrs, mins, secs
    );

    let mut collection_data = CollectionData::default();
    collection_data.benchmark_name = set_name.to_string();

    let (version, os, exe_type) = FACTORIO_INFO.clone();
    collection_data.factorio_version = version;
    collection_data.os = os;
    collection_data.executable_type = exe_type;
    collection_data.cpuid = query_system_cpuid();

    for param in set_params {
        run_benchmark_single_map(param, Some(&mut collection_data));
    }

    let total_duration = now.elapsed().as_secs_f64();
    let hrs = (total_duration / 3600.0) as u64;
    let mins = ((total_duration % 3600.0) / 60.0) as u64;
    let secs = (total_duration % 3600.0) % 60.0;
    println!(
        "Benchmarks took: {}:{:02}:{:02.3}",
        hrs, mins, secs
    );
    upload_to_db(collection_data);

}

fn run_benchmark_single_map(params: SimpleBenchmarkParam, collection_data: Option<&mut CollectionData>) -> Option<BenchmarkDurationOverhead> {
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
        if params.persist_data_to_db == PersistDataToDB::True && VERBOSE_DATA_ROW_MATCH_PATTERN.is_match(&line) {
            if line_index % 1000 == 0 {
                run_index += 1;
            }
            line.push_str(&format!("{}", run_index));
            verbose_data.push(line.replace('t',""));
            line_index += 1;
            assert!(run_index > 0);
        }
    }
    if params.persist_data_to_db == PersistDataToDB::True {
        //We should have as many lines as ticks have been performed
        assert_eq!((params.ticks * params.runs) as usize, verbose_data.len());
        bench_data.verbose_data = verbose_data;
        collection_data.unwrap().benchmarks.push(bench_data);
    }
    benchmark_times
}
