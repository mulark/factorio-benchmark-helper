#![allow(clippy::needless_return)]
#[macro_use]
extern crate lazy_static;

extern crate directories;
extern crate getopts;
extern crate glob;
extern crate regex;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate sha2;

use crate::benchmark_runner::run_benchmarks;
use crate::util::bulk_sha256;
use sha2::Digest;
use std::env;
use std::fs::read;
use std::io::{stdin};
use std::path::PathBuf;

mod benchmark_runner;
mod procedure_file;
mod util;
use util::{
    add_options,
    BenchmarkSet,
    fetch_user_supplied_optargs,
    get_download_links_from_google_drive_by_filelist,
    get_saves_directory,
    Map,
    print_all_procedures,
    ProcedureFileKind,
    read_procedure_from_file,
    UserSuppliedArgs,
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
    let mut params = UserSuppliedArgs::default();
    parse_args(&mut params);
}

/*
--list
    LISTING OF PROCEDURES
    LISTING OF METASETS
--create-benchmark-procedure Option<PROCEDURE_NAME>
    --interactive
        RUNS
        TICKS
        PATTERN
        MOD_LISTS
        UPLOAD_DIRECTORY
    or
    --runs RUNS
    --ticks TICKS
    --pattern PATTERN
    --upload GOOGLE_DRIVE_URL
--run-benchmark PROCEDURE

--run-meta-benchmark META_NAME
*/

fn parse_args(mut user_args: &mut UserSuppliedArgs) {
    let args: Vec<String> = env::args().collect();
    let mut options = getopts::Options::new();
    add_options(&mut options);
    if args.len() == 1 {
        println!("No arguments supplied!");
        println!("{}", options.usage(FACTORIO_BENCHMARK_HELPER_NAME));
        std::process::exit(0);
    }

    let matched_options = match options.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => {
            println!("{}", options.usage(FACTORIO_BENCHMARK_HELPER_NAME));
            eprintln!("{}", e);
            std::process::exit(0);
        }
    };

    fetch_user_supplied_optargs(&matched_options, &mut user_args);
    if matched_options.opt_present("help") {
        println!("{}", options.usage(FACTORIO_BENCHMARK_HELPER_NAME));
        std::process::exit(0);
    }
    if matched_options.opt_present("version") {
        print_version();
        std::process::exit(0);
    }
    if matched_options.opt_present("commit") {
        if let Some(name) = &user_args.benchmark_set_name {
            if let Some(set) =  read_procedure_from_file(name, ProcedureFileKind::Local) {
                write_procedure_to_file(name, set, user_args.overwrite_existing_procedure, ProcedureFileKind::Master);
                println!("Successfully commited {:?} to the master json file... Now submit a PR :)", name);
                std::process::exit(0);
            }
        }
    }
    if matched_options.opt_present("list") {
        print_all_procedures();
        std::process::exit(0);
    }
    if matched_options.opt_present("create-benchmark-procedure") {
        if matched_options.opt_present("interactive") {
            create_benchmark_procedure_interactive(&mut user_args);
        } else {
            create_benchmark_procedure(&user_args);
        }
        std::process::exit(0);
    }
    if matched_options.opt_present("benchmark") {
        let local = read_procedure_from_file(&user_args.benchmark_set_name.as_ref().unwrap(), ProcedureFileKind::Local);
        let master = read_procedure_from_file(&user_args.benchmark_set_name.as_ref().unwrap(), ProcedureFileKind::Master);
        if local.is_some() || master.is_some() {
            if local.is_some() && master.is_some() && local.clone().unwrap() != master.clone().unwrap() {
                println!("WARN: procedure with name {:?} is present in both local and master, and they differ.", user_args.benchmark_set_name);
                println!("WARN: procedure is being ran from master.json");
            }
            let procedure = if master.is_some() { master } else { local };
            run_benchmarks(procedure.unwrap());
        } else {
            eprintln!("Could not find benchmark with the name: {:?}", user_args.benchmark_set_name);
            std::process::exit(1);
        }
    }
}

fn print_version() {
    println!("{} {}", FACTORIO_BENCHMARK_HELPER_NAME, FACTORIO_BENCHMARK_HELPER_VERSION);
    std::process::exit(0);
}

fn create_benchmark_procedure(user_args: &UserSuppliedArgs) {
    let mut benchmark_builder = BenchmarkSet::default();
    if user_args.pattern.is_none() {
        println!("WARN: Did not explictly set a --pattern, selecting all maps.");
    }
    let current_map_paths = get_map_paths_from_pattern(&user_args.pattern.as_ref().unwrap_or(&"".to_string()));
    if current_map_paths.is_empty() {
        eprintln!("Supplied pattern found no maps!");
        std::process::exit(1);
    }
    let path_to_sha256_tuple = bulk_sha256(current_map_paths.clone());
    for (a_map, the_hash) in path_to_sha256_tuple {
        let map_struct = Map::new(a_map.file_name().unwrap().to_str().unwrap(), &the_hash, "");
        benchmark_builder.maps.push(map_struct);
    }
    if let Some(t) = &user_args.ticks {
        if *t == 0 {
            eprintln!("Ticks aren't allowed to be 0!");
            std::process::exit(1);
        }
        benchmark_builder.ticks = *t;
    } else {
        eprintln!("Missing argument (u32): --ticks");
        std::process::exit(1);
    }
    if let Some(r) = &user_args.runs {
        if *r == 0 {
            eprintln!("Runs aren't allowed to be 0!");
            std::process::exit(1);
        }
        benchmark_builder.runs = *r;
    } else {
        eprintln!("Missing argument (u32): --runs");
        std::process::exit(1);
    }
    if let Some(url) = &user_args.google_drive_folder {
        if !url.starts_with("https://drive.google.com/drive/") {
            eprintln!("Google Drive URL didn't match expected format!");
            std::process::exit(1);
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
        user_args.overwrite_existing_procedure,
        ProcedureFileKind::Local
    );

}

fn create_benchmark_procedure_interactive(user_args: &mut UserSuppliedArgs) {
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
        input.pop();
        if input.to_lowercase() == "y" {
                        benchmark_builder.mods = prompt_for_mods();
        }
    }
    /*input.clear();
    println!("Upload maps to google drive? NOT IMPLEMENTED");
    if let Ok(_m) = std::io::stdin().read_line(&mut input) {
        input.pop();
        if input.to_lowercase() == "y" {

        }
    }*/
    input.clear();
    println!("Provide Google Drive shared folder url that contains maps or hit enter to skip.");
    if let Ok(_m) = stdin().read_line(&mut input) {
        input.pop();
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
        input.pop();
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
            input.pop();
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
            input.pop();
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
        input.pop();
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
        input.pop();
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
