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

use crate::util::bulk_sha256;
use benchmark_runner::{run_benchmarks_multiple_maps, BenchmarkParams};
use getopts::Matches;
use reqwest::Response;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::env;
use std::fs::read;
use std::fs::File;
use std::io::{stdin, BufRead};
use std::io::{BufReader, Read};
use std::path::PathBuf;

mod benchmark_runner;
mod database;
mod procedure_file;
use database::BenchmarkResults;
mod util;
use util::{
    fbh_read_configuration_setting,
    //prompt_for_mods,
    get_download_links_from_google_drive_by_filelist,
    get_saves_directory,
    BenchmarkSet,
    Map,
    Mod,
    read_procedure_from_file,
    write_procedure_to_file,
    ProcedureFileKind,
};

const FACTORIO_BENCHMARK_HELPER_VERSION: &str = "0.0.1";

struct UserSuppliedArgs {
    new_benchmark_set_name: Option<String>,
    ticks: Option<u32>,
    runs: Option<u32>,
    pattern: Option<String>,
    help_target: Option<String>,
    overwrite_existing_procedure: bool,
    google_drive_folder: Option<String>,
    commit_name: Option<String>,
}

impl Default for UserSuppliedArgs {
    fn default() -> UserSuppliedArgs {
        UserSuppliedArgs {
            new_benchmark_set_name: None,
            ticks: None,
            runs: None,
            pattern: None,
            help_target: None,
            overwrite_existing_procedure: false,
            google_drive_folder: None,
            commit_name: None,
        }
    }
}

fn fetch_user_supplied_optargs(options: &Matches, user_args: &mut UserSuppliedArgs) {
    if let Ok(new_set_name) = options.opt_get::<String>("create-benchmark-procedure") {
        user_args.new_benchmark_set_name = new_set_name;
    }
    if let Ok(ticks) = options.opt_get::<u32>("ticks") {
        user_args.ticks = ticks;
    }
    if let Ok(runs) = options.opt_get::<u32>("runs") {
        user_args.runs = runs;
    }
    if let Ok(pattern) = options.opt_get::<String>("pattern") {
        user_args.pattern = pattern;
    }
    if let Ok(help_target) = options.opt_get::<String>("help") {
        user_args.help_target = help_target;
    }
    if let Ok(drive_url) = options.opt_get::<String>("google-drive-folder") {
        user_args.google_drive_folder = drive_url;
    }
    if let Ok(commit_name) = options.opt_get::<String>("commit") {
        user_args.commit_name = commit_name;
    }
}

fn main() {
    match util::initialize() {
        Ok(_) => (),
        Err(e) => {
            println!("Failed to initialize Factorio Benchmark Helper");
            panic!(e);
        }
    }
    //util::create_procedure_interactively();
    //database::put_data_to_db(BenchmarkResults::new())
    /*
        let m = Mod::new("creative-world-plus", "0.0.9", "e90da651af3eac017210b85dab5a09c15cf5aca8");
        let m4 = Mod::new("creative-world-plus", "0.0.9", "e90da651af3eac017210b85dab5a09c15cf5aca8");
        //let m2 = Mod::new("warptorio2_expansion", "0.0.35", "fc4e77dd57953bcf79570b38698bd5c2ea07af2b");
        //let m3 = Mod::new("warptorio2_expansion", "", "");
        let ms = ModSet {mods: vec!(m, m4)};
        let ma = Map::new("foobar.zip", "89e807c58e547f99915e184baac32cbf3e22b7191110580430e48f90a25be657", "https://forums.factorio.com/download/file.php?id=54562", 100, 100);
        let maps = vec!(ma);
        let bs = BenchmarkSet {maps, mod_groups: vec!(ms), name: "test".to_string(), pattern: "".to_string()};
        util::fetch_benchmark_deps_parallel(bs);
    */

    //procedure_file::set_json();
    let mut params = UserSuppliedArgs::default();
    parse_args(&mut params);
    //get_map_paths_and_append_to_params(&mut params);
    //run_benchmarks_multiple_maps(&params);
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

fn add_options(options: &mut getopts::Options) {
    options.optflag(
        "h",
        "help",
        "Prints this general help",
    );
    options.optflag("v", "version", "Print program version, then exits");
    options.optflag(
        "",
        "list",
        "Lists available benchmark/meta sets",
    );
    options.optflag("", "interactive", "Runs program interactively");
    options.optflag("", "overwrite", "Overwrite existing procedure if NAME supplied already exists.");
    options.opt(
        "",
        "pattern",
        "Limit benchmarks to maps that match PATTERN",
        "PATTERN",
        getopts::HasArg::Yes,
        getopts::Occur::Optional,
    );
    options.optopt(
        "",
        "ticks",
        "Runs benchmarks for TICKS duration per run",
        "TICKS",
    );
    options.optopt(
        "",
        "runs",
        "How many times should each map be benchmarked?",
        "TIMES",
    );
    options.opt(
        "",
        "create-benchmark-procedure",
        "Create a benchmark procedure named NAME",
        "NAME",
        getopts::HasArg::Yes,
        getopts::Occur::Optional,
    );
    options.opt(
        "",
        "google-drive-folder",
        "A link to a publically shared folder that contains the maps of the benchmark set you are creating.",
        "LINK",
        getopts::HasArg::Yes,
        getopts::Occur::Optional,
    );
    options.optopt(
        "",
        "commit",
        "Commits a benchmark to the master json file, staging it for upload to the git repository.",
        "NAME",
    );
}

fn parse_args(mut user_args: &mut UserSuppliedArgs) {
    let args: Vec<String> = env::args().collect();
    let mut options = getopts::Options::new();
    options.parsing_style(getopts::ParsingStyle::FloatingFrees);
    add_options(&mut options);

    if args.len() == 1 {
        println!("No arguments supplied!");
        println!("{}", options.short_usage("factorio-benchmark-helper"));
        std::process::exit(0);
    }
    let matched_options = match options.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => {
            println!("{}", options.short_usage("factorio-benchmark-helper"));
            eprintln!("{}", e);
            std::process::exit(0);
        }
    };

    fetch_user_supplied_optargs(&matched_options, &mut user_args);
    if matched_options.opt_present("help") {
        println!("{}", options.usage("factorio-benchmark-helper"));
        std::process::exit(0);
    }
    if matched_options.opt_present("version") {
        print_version();
        std::process::exit(0);
    }
    if matched_options.opt_present("commit") {
        if let Some(name) = &user_args.commit_name {
            if let Some(set) =  read_procedure_from_file(name, ProcedureFileKind::Local) {
                write_procedure_to_file(name, set, false, ProcedureFileKind::Master);
            }
        }
    }
    if matched_options.opt_present("list") {
        println!("Stub for --list");
    }
    if matched_options.opt_present("create-benchmark-procedure") {
        if matched_options.opt_present("overwrite") {
            user_args.overwrite_existing_procedure = true;
        }
        if matched_options.opt_present("interactive") {
            create_benchmark_procedure_interactive(&mut user_args);
        } else {
            create_benchmark_procedure(&user_args);
        }
    }

}

fn print_version() {
    println!("factorio-benchmark-helper {}", FACTORIO_BENCHMARK_HELPER_VERSION);
    std::process::exit(0);
}

fn create_benchmark_procedure(user_args: &UserSuppliedArgs) {
    let mut benchmark_builder = BenchmarkSet::default();
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
    }
    if let Some(r) = &user_args.runs {
        if *r == 0 {
            eprintln!("Runs aren't allowed to be 0!");
            std::process::exit(1);
        }
        benchmark_builder.runs = *r;
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
        }
    }
    assert!(user_args.new_benchmark_set_name.is_some());
    assert!(!benchmark_builder.maps.is_empty());
    assert!(benchmark_builder.runs > 0);
    assert!(benchmark_builder.ticks > 0);
    write_procedure_to_file(
        &user_args.new_benchmark_set_name.as_ref().unwrap(),
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
    println!("If you do not specify any mod sets, vanilla is implied.");
    let mut input = String::new();
    if let Ok(_m) = stdin().read_line(&mut input) {
        input.pop();
        if input.to_lowercase() == "y" {
            //            benchmark_builder.mods = prompt_for_mods();
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
