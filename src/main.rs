#![allow(clippy::needless_return)]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate clap;
extern crate directories;
extern crate getopts;
extern crate glob;
extern crate regex;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate sha2;

use crate::util::trim_newline;
use std::collections::HashMap;
use crate::procedure_file::read_meta_from_file;
use crate::procedure_file::write_meta_to_file;
use crate::procedure_file::get_sets_from_meta;
use crate::benchmark_runner::run_benchmarks;
use crate::util::bulk_sha256;
use sha2::Digest;
use std::env;
use std::fs::read;
use std::io::{stdin};
use std::path::PathBuf;
use std::process::exit;

mod benchmark_runner;
mod procedure_file;
mod util;
use util::{
    add_options_and_parse,
    BenchmarkSet,
    get_download_links_from_google_drive_by_filelist,
    get_saves_directory,
    Map,
    print_all_procedures,
    ProcedureFileKind,
    prompt_until_allowed_vals,
    read_procedure_from_file,
    UserArgs,
    write_procedure_to_file,
    prompt_for_mods,
};

const FACTORIO_BENCHMARK_HELPER_VERSION: &str = env!("CARGO_PKG_VERSION");
const FACTORIO_BENCHMARK_HELPER_NAME: &str = env!("CARGO_PKG_NAME");

fn main() {
    match util::initialize() {
        Ok(_) => (),
        Err(e) => {
            println!("Failed to initialize Factorio Benchmark Helper");
            panic!(e);
        }
    }
    let mut parsed_args = add_options_and_parse();
    execute_from_args(&mut parsed_args);
    //println!("{:?}", stuff);
//    let mut params = UserArgs::default();
//    parse_args(&mut params);
}
// Precedence of exclusive execution
// commit
// run benchmark
// run metabenchmark
// create benchmark
// create metabenchmark

fn execute_from_args(args: &mut UserArgs) {
    if args.interactive {
        println!("Selected interactive mode.");
    }
    if !(args.commit_flag || args.run_benchmark || args.run_meta || args.create_benchmark || args.create_meta) {
        if args.interactive {
            println!("Choose a suitable course of action.");
            println!("1: Commit a benchmark or meta set to the master.json file from the local.json file.");
            println!("2: Run a benchmark.");
            println!("3: Run a metabenchmark.");
            println!("4: Create a new benchmark.");
            println!("5: Create a new metabenchmark.");
            match prompt_until_allowed_vals(&[1,2,3,4,5]) {
                Some(m) => match m {
                    1 => {
                        args.commit_flag = true;
                    },
                    2 => {
                        args.run_benchmark = true;
                    },
                    3 => {
                        args.run_meta = true
                    },
                    4 => {
                        args.create_benchmark = true
                    },
                    5 => {
                        args.create_meta = true
                    },
                    _ => {
                        eprintln!("Unrecognized option {:?}", m);
                        exit(1);
                    }
                },
                _ => {
                    eprintln!("You supplied something that isn't an interger.");
                    exit(1);
                }
            }
        } else {
            eprintln!("You provided args but didn't pick commit/benchmark/meta/create-benchmark/create-meta or interactive!");
            eprintln!("Without one of these options there's nothing to do.");
            exit(1);
        }
    }
    if args.commit_flag {
        perform_commit(&args);
    } else if args.run_benchmark {
        let benchmark_sets_to_run = convert_args_to_benchmark_run(&args);
        run_benchmarks_multiple(benchmark_sets_to_run);
    } else if args.run_meta {
        let benchmark_sets_to_run = convert_args_to_meta_benchmark_runs(&args);
        run_benchmarks_multiple(benchmark_sets_to_run);
    } else if args.create_benchmark {
        create_benchmark_from_args(&args);
    } else if args.create_meta {
        create_meta_from_args(&args);
    }

}

fn run_benchmarks_multiple(multiple_sets: HashMap<String, BenchmarkSet>) {

}

fn perform_commit(args: &UserArgs) {
    if args.commit_name.is_none() && args.commit_type.is_none() {
        if args.interactive {
            println!("Performing a commit to the master.json file.");
        }
    }
    let commit_name = args.commit_name.as_ref().unwrap();
    let commit_type = args.commit_type.as_ref().unwrap();
    if commit_type == "benchmark" {
        if let Some(benchmark_set) =  read_procedure_from_file(commit_name, ProcedureFileKind::Local) {
            write_procedure_to_file(commit_name, benchmark_set, args.overwrite, ProcedureFileKind::Master);
            println!("Successfully committed {:?} to the master json file... Now submit a PR :)", commit_name);
            exit(0);
        } else {
            eprintln!("Failed to commit benchmark set {:?} to master, because that benchmark set doesn't exist in local!", commit_name);
            exit(1);
        }
    } else if commit_type == "meta" {
        if let Some(meta_set) = read_meta_from_file(commit_name, ProcedureFileKind::Local) {
            write_meta_to_file(commit_name, meta_set, args.overwrite, ProcedureFileKind::Master);
        } else {
            eprintln!("Failed to commit meta set {:?} to master, because that meta set doesn't exist in local!", commit_name);
            exit(1);
        }
    } else {
        eprintln!("Commit type is neither meta or benchmark! We should have caught this eariler.");
        exit(1);
    }
}

fn convert_args_to_benchmark_run(args: &UserArgs) -> HashMap<String, BenchmarkSet> {
    let name = args.benchmark_set_name.as_ref().unwrap().to_owned();
    let mut hash_map = HashMap::default();
    let local = read_procedure_from_file(&name, ProcedureFileKind::Local);
    let master = read_procedure_from_file(&name, ProcedureFileKind::Master);
    if local.is_some() || master.is_some() {
        if local.is_some() && master.is_some() && local.clone().unwrap() != master.clone().unwrap() {
            println!("WARN: benchmark with name {:?} is present in both local and master, and they differ.", &name);
            println!("WARN: benchmark is being ran from master.json");
        }
        let procedure = if master.is_some() { master } else { local };
        hash_map.insert(name, procedure.unwrap());
        return hash_map;
    } else {
        eprintln!("Could not find benchmark with the name: {:?}", &name);
        exit(1);
    }
}

fn convert_args_to_meta_benchmark_runs(args: &UserArgs) -> HashMap<String, BenchmarkSet> {
    let name = args.meta_set_name.as_ref().unwrap().to_owned();
    let local = read_meta_from_file(&name, ProcedureFileKind::Local);
    let master = read_meta_from_file(&name, ProcedureFileKind::Master);
    if local.is_some() || master.is_some() {
        if local.is_some() && master.is_some() && local.clone().unwrap() != master.clone().unwrap() {
            println!("WARN: meta set with name {:?} is present in both local and master, and they differ.", &name);
            println!("WARN: meta set is being ran from master.json");
        }
        let meta_src_file = if master.is_some() { ProcedureFileKind::Master } else { ProcedureFileKind::Local };
        get_sets_from_meta(name, meta_src_file)
    } else {
        eprintln!("Could not find meta benchmark set with the name: {:?}", &name);
        exit(1);
    }
}

fn create_benchmark_from_args(args: &UserArgs) {

}

fn create_meta_from_args(args: &UserArgs) {

}

fn parse_args(mut user_args: &mut UserArgs) {
    let args: Vec<String> = env::args().collect();
    let mut options = getopts::Options::new();
    //add_options(&mut options);

    create_benchmark_procedure_interactive(&mut user_args);

    create_benchmark_procedure(&user_args);

    exit(0);

}

fn print_version() {
    println!("{} {}", FACTORIO_BENCHMARK_HELPER_NAME, FACTORIO_BENCHMARK_HELPER_VERSION);
    exit(0);
}

fn create_benchmark_procedure(user_args: &UserArgs) {
    let mut benchmark_builder = BenchmarkSet::default();
    if user_args.pattern.is_none() {
        println!("WARN: Did not explictly set a --pattern, selecting all maps.");
    }
    let current_map_paths = get_map_paths_from_pattern(&user_args.pattern.as_ref().unwrap_or(&"".to_string()));
    if current_map_paths.is_empty() {
        eprintln!("Supplied pattern found no maps!");
        exit(1);
    }
    let path_to_sha256_tuple = bulk_sha256(current_map_paths.clone());
    for (a_map, the_hash) in path_to_sha256_tuple {
        let map_struct = Map::new(a_map.file_name().unwrap().to_str().unwrap(), &the_hash, "");
        benchmark_builder.maps.push(map_struct);
    }
    if let Some(t) = &user_args.ticks {
        if *t == 0 {
            eprintln!("Ticks aren't allowed to be 0!");
            exit(1);
        }
        benchmark_builder.ticks = *t;
    } else {
        eprintln!("Missing argument (u32): --ticks");
        exit(1);
    }
    if let Some(r) = &user_args.runs {
        if *r == 0 {
            eprintln!("Runs aren't allowed to be 0!");
            exit(1);
        }
        benchmark_builder.runs = *r;
    } else {
        eprintln!("Missing argument (u32): --runs");
        exit(1);
    }
    if let Some(url) = &user_args.google_drive_folder {
        if !url.starts_with("https://drive.google.com/drive/") {
            eprintln!("Google Drive URL didn't match expected format!");
            exit(1);
        }
        if let Some(resp) = get_download_links_from_google_drive_by_filelist(current_map_paths, &user_args.google_drive_folder.as_ref().unwrap()) {
            for (fname, dl_link) in resp {
                for mut map in &mut benchmark_builder.maps {
                    if map.name == fname {
                        map.download_link = dl_link.clone();
                    }
                }
            }
            for map in &benchmark_builder.maps {
                if map.download_link.is_empty() {
                    println!("WARN: you specified a google drive folder but we didn't find the map {:?} in it!", map.name);
                }
            }
        }
    } else {
        println!("WARN: no google drive folder specified, no download links will be populated.");
    }
    assert!(user_args.benchmark_set_name.is_some());
    assert!(!benchmark_builder.maps.is_empty());
    assert!(benchmark_builder.runs > 0);
    assert!(benchmark_builder.ticks > 0);
    write_procedure_to_file(
        &user_args.benchmark_set_name.as_ref().unwrap(),
        benchmark_builder,
        user_args.overwrite,
        ProcedureFileKind::Local
    );

}

fn create_benchmark_procedure_interactive(user_args: &mut UserArgs) {
    let mut benchmark_builder = BenchmarkSet::default();
    let mut pattern = String::new();
    if let Some(p) = &user_args.pattern {
        pattern = p.to_string();
    }
    retrieve_pattern(&mut pattern);
    let current_map_paths = get_map_paths_from_pattern(&pattern);
    if user_args.ticks.is_none() {
        println!("Enter the number of ticks to test per run... [1000]");
        prompt_for_nonzero_u32(&mut benchmark_builder.ticks, 1000);
    } else {
        benchmark_builder.ticks = user_args.ticks.unwrap();
        println!(
            "Ticks supplied from arguments... {}",
            benchmark_builder.ticks
        );
    }
    if user_args.runs.is_none() {
        println!("Enter the number of times to benchmark each map... [1]");
        prompt_for_nonzero_u32(&mut benchmark_builder.runs, 1);
    } else {
        benchmark_builder.ticks = user_args.ticks.unwrap();
        println!(
            "Runs supplied from arguments... {}",
            benchmark_builder.ticks
        );
    }
    println!("Benchmark with mods? [y/N]");
    println!("If you do not specify any mods, vanilla is implied.");
    let mut input = String::new();
    if let Ok(_m) = stdin().read_line(&mut input) {
        trim_newline(&mut input);
        if input.to_lowercase() == "y" {
                        benchmark_builder.mods = prompt_for_mods();
        }
    }
    /*input.clear();
    println!("Upload maps to google drive? NOT IMPLEMENTED");
    if let Ok(_m) = std::io::stdin().read_line(&mut input) {
        trim_newline(&mut input);;
        if input.to_lowercase() == "y" {

        }
    }*/
    input.clear();
    println!("Provide Google Drive shared folder url that contains maps or hit enter to skip.");
    if let Ok(_m) = stdin().read_line(&mut input) {
        trim_newline(&mut input);
        if input.contains("drive.google.com") {
            get_download_links_from_google_drive_by_filelist(current_map_paths, &input);
        } else {
            for path in &current_map_paths {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                let this_dl_link = prompt_dl_link_indiv_map(&name);
                benchmark_builder.maps.push(Map::new(&name,"",&this_dl_link));
            }
        }
    }
    for mut map in benchmark_builder.maps {
        if map.sha256.is_empty() {
            map.sha256 = format!("{:x}", sha2::Sha256::digest(
                &read(&get_saves_directory().join(&map.name)).unwrap()
            ));
        }
    }
}

fn prompt_dl_link_indiv_map(name: &str) -> String {
    let mut input = String::new();
    println!("Enter a valid download link for the save {}", name);
    if let Ok(_m) = stdin().read_line(&mut input) {
        trim_newline(&mut input);
        if let Ok(some_resp) = reqwest::get(&input) {
            if some_resp.status().is_success() {
                return input;
            }
        }
    }
    String::new()
}

fn retrieve_pattern(pattern: &mut String) {
    let mut input = String::new();
    if pattern.is_empty() {
        println!("Enter a map pattern to match...");
        if let Ok(_m) = stdin().read_line(&mut input) {
            trim_newline(&mut input);
        }
    } else {
        println!("Pattern supplied from args... {}", pattern);
    }
    let mut found_map_paths;
    let mut cont = true;
    while cont {
        println!("You selected pattern {}, the maps found are:", input);
        found_map_paths = get_map_paths_from_pattern(&input.clone());
        for m in &mut found_map_paths {
            println!("{:?}", m);
        }
        println!("Hit enter to confirm or enter a new pattern.");
        if let Ok(_m) = stdin().read_line(&mut input) {
            trim_newline(&mut input);
            if input.is_empty() {
                cont = false;
            } else {
                *pattern = input.clone();
                input.clear();
            }
        }
    }
}

fn prompt_for_nonzero_u32(numeric_field: &mut u32, default: u32) {
    let mut input = "".to_string();
    while *numeric_field == 0 {
        stdin().read_line(&mut input).expect("");
        trim_newline(&mut input);
        if input.is_empty() {
            *numeric_field = default;
        } else {
            match input.parse::<u32>() {
                // 0 is allowed but due to while looping it won't be used.
                Ok(p) => *numeric_field = p,
                _ => {
                    println!("{:?} is not a valid parameter", input);
                    input.clear();
                }
            }
        }
    }
}

fn get_map_paths_from_pattern(initial_input: &str) -> Vec<PathBuf> {
    let mut input = initial_input.to_string();
    let mut map_paths = Vec::new();
    let save_directory = get_saves_directory();
    assert!(save_directory.is_dir());
    if input == "*" {
        trim_newline(&mut input);
    }
    if !input.is_empty() {
        input.push_str("*");
    }
    let combined_pattern = &format!("{}*{}", save_directory.to_string_lossy(), input);
    let try_pattern = glob::glob(combined_pattern);
    if let Ok(m) = try_pattern {
        for item in m.filter_map(Result::ok) {
            if item.is_file() {
                if let Some(extension) = item.extension() {
                    if let Some("zip") = extension.to_str() {
                        map_paths.push(item);
                    }
                }
            }
        }
    }
    map_paths
}
