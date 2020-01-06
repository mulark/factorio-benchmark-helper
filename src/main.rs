#![allow(clippy::needless_return)]
#[macro_use]
extern crate lazy_static;
extern crate bincode;
extern crate clap;
extern crate directories;
extern crate glob;
extern crate regex;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate sha2;

use crate::procedure_file::get_metas_from_meta;
use crate::procedure_file::get_sets_from_meta;
use crate::procedure_file::read_meta_from_file;
use crate::procedure_file::write_meta_to_file;
use crate::util::recompress_saves_parallel;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::exit;

mod benchmark_runner;
use benchmark_runner::run_benchmarks_multiple;

mod procedure_file;
mod util;
use util::{
    add_options_and_parse, get_download_links_from_google_drive_by_filelist, get_mod_info,
    get_saves_directory, prompt_until_allowed_val, prompt_until_allowed_val_in_range,
    prompt_until_empty_str, read_benchmark_set_from_file, trim_newline,
    write_benchmark_set_to_file, BenchmarkSet, Map, Mod, ProcedureFileKind, ProcedureKind,
    UserArgs,
};

lazy_static! {
    static ref MOD_VERSION_MATCHER: Regex = Regex::new(r"_\{1,3}.\{1,4}.\{1,4}").unwrap();
}

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
}

fn execute_from_args(mut args: &mut UserArgs) {
    if args.interactive {
        println!("Selected interactive mode.");
    }
    if !(args.commit_flag
        || args.run_benchmark
        || args.run_meta
        || args.create_benchmark
        || args.create_meta)
    {
        if args.interactive {
            println!("Choose a suitable course of action.");
            println!("1: Commit a benchmark or meta set to the master.json file from the local.json file.");
            println!("2: Run a benchmark.");
            println!("3: Run a metabenchmark.");
            println!("4: Create a new benchmark.");
            println!("5: Create a new metabenchmark.");
            match prompt_until_allowed_val(&[1, 2, 3, 4, 5]) {
                1 => args.commit_flag = true,
                2 => args.run_benchmark = true,
                3 => args.run_meta = true,
                4 => args.create_benchmark = true,
                5 => args.create_meta = true,
                _ => {
                    unreachable!("How did you match to this after getting an allowed value?");
                }
            }
        } else {
            eprintln!("You provided args but didn't pick commit/benchmark/meta/create-benchmark/create-meta or interactive!");
            eprintln!("Without one of these options there's nothing to do.");
            exit(1);
        }
    }
    if args.commit_flag {
        perform_commit(&mut args);
    } else if args.run_benchmark {
        let benchmark_sets_to_run = convert_args_to_benchmark_run(&mut args);
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

fn perform_commit(args: &mut UserArgs) {
    if args.commit_name.is_none() || args.commit_type.is_none() {
        if args.commit_type.is_none() {
            if args.interactive {
                println!("Interactively committing to master.json");
                println!("Please enter the type of set you wish to commit to the master.json file. Allowed values: benchmark, meta");
                args.commit_type = Some(prompt_until_allowed_val(&[
                    ProcedureKind::Benchmark,
                    ProcedureKind::Meta,
                ]));
            } else {
                eprintln!("Cannot commit to master.json because commit type is nothing!");
                exit(1);
            }
        }
        if args.commit_name.is_none() {
            if args.interactive {
                println!(
                    "Selected type {:?}, now enter a valid name for this type.",
                    args.commit_type.as_ref().unwrap()
                );
                println!("The available sets are: ");
            } else {
                eprintln!("Cannot commit to master.json because commit name is nothing!");
                exit(1);
            }
        }
    }
    let commit_name = args.commit_name.as_ref().unwrap();
    let commit_type = args.commit_type.as_ref().unwrap();
    if commit_type == &ProcedureKind::Benchmark {
        if let Some(benchmark_set) =
            read_benchmark_set_from_file(commit_name, ProcedureFileKind::Local)
        {
            write_benchmark_set_to_file(
                commit_name,
                benchmark_set,
                args.overwrite,
                ProcedureFileKind::Master,
                false,
            );
            println!(
                "Successfully committed {:?} to the master json file... Now submit a PR :)",
                commit_name
            );
            exit(0);
        } else {
            eprintln!("Failed to commit benchmark set {:?} to master, because that benchmark set doesn't exist in local!", commit_name);
            exit(1);
        }
    } else if commit_type == &ProcedureKind::Meta {
        if let Some(meta_set) = read_meta_from_file(commit_name, ProcedureFileKind::Local) {
            if args.commit_recursive {
                println!("Selected recursive, committing all members of this meta");
                let meta_sets =
                    get_metas_from_meta(commit_name.to_string(), ProcedureFileKind::Local);
                let benchmark_sets =
                    get_sets_from_meta(commit_name.to_string(), ProcedureFileKind::Local);
                for (name, set) in benchmark_sets {
                    write_benchmark_set_to_file(
                        &name,
                        set,
                        args.overwrite,
                        ProcedureFileKind::Master,
                        args.interactive,
                    )
                }
                for meta in meta_sets {
                    if let Some(members) = read_meta_from_file(&meta, ProcedureFileKind::Local) {
                        write_meta_to_file(
                            &meta,
                            members,
                            args.overwrite,
                            ProcedureFileKind::Master,
                        )
                    }
                }
            }
            write_meta_to_file(
                commit_name,
                meta_set,
                args.overwrite,
                ProcedureFileKind::Master,
            );
        } else {
            eprintln!("Failed to commit meta set {:?} to master, because that meta set doesn't exist in local!", commit_name);
            exit(1);
        }
    } else {
        unreachable!(
            "Commit type is neither meta or benchmark! We should have caught this eariler."
        );
    }
}

fn convert_args_to_benchmark_run(args: &mut UserArgs) -> HashMap<String, BenchmarkSet> {
    if args.benchmark_set_name.is_none() {
        if args.interactive {
            println!("Available benchmarks to run: ");

            println!("Enter name of a benchmark you wish to run.");
            prompt_until_empty_str(false);
        } else {
            unreachable!("Cannot have gotten here without interactive or --benchmark");
        }
    }
    let name = args.benchmark_set_name.as_ref().unwrap().to_owned();
    let mut hash_map = HashMap::default();
    let local = read_benchmark_set_from_file(&name, ProcedureFileKind::Local);
    let master = read_benchmark_set_from_file(&name, ProcedureFileKind::Master);
    if local.is_some() || master.is_some() {
        if local.is_some() && master.is_some() && local.clone().unwrap() != master.clone().unwrap()
        {
            println!("WARN: benchmark with name {:?} is present in both local and master, and they differ.", &name);
            println!("WARN: benchmark is being ran from master.json");
        }
        let procedure = if master.is_some() { master } else { local };
        hash_map.insert(name, procedure.unwrap());
        hash_map
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
        if local.is_some() && master.is_some() && local.unwrap() != master.clone().unwrap()
        {
            println!("WARN: meta set with name {:?} is present in both local and master, and they differ.", &name);
            println!("WARN: meta set is being ran from master.json");
        }
        let meta_src_file = if master.is_some() {
            ProcedureFileKind::Master
        } else {
            ProcedureFileKind::Local
        };
        get_sets_from_meta(name, meta_src_file)
    } else {
        eprintln!(
            "Could not find meta benchmark set with the name: {:?}",
            &name
        );
        exit(1);
    }
}

fn create_benchmark_from_args(args: &UserArgs) {
    let set_name;
    let map_pattern;
    let mut google_drive_folder = String::from("");
    let map_paths;
    let mod_list;
    let mut benchmark = BenchmarkSet::default();

    if args.benchmark_set_name.is_some() {
        set_name = args.benchmark_set_name.as_ref().unwrap().clone();
    } else if args.interactive {
        println!("No benchmark set name was defined, enter a benchmark set name.");
        set_name = prompt_until_empty_str(false);
    } else {
        unreachable!("Failed to create a benchmark set because no name was defined!");
    }

    if args.pattern.is_some() {
        map_pattern = args.pattern.as_ref().unwrap().clone();
    } else if args.interactive {
        println!("No map pattern was defined, enter a pattern to search for maps, empty for all.");
        map_pattern = prompt_until_empty_str(true);
    } else {
        println!("WARN: A map pattern was not explictly defined, selecting all available maps.");
        map_pattern = String::from("");
    }
    map_paths = get_map_paths_from_pattern(&map_pattern);
    if map_paths.is_empty() {
        eprintln!("Supplied pattern found no maps!");
        exit(1);
    } else {
        println!("Found the following maps:");
        for map in &map_paths {
            println!("{:?}", map);
        }
    }
    let resave = args.resave;
    let handle = std::thread::spawn(move || {
        recompress_saves_parallel(map_paths.clone(), resave)
    });

    if args.ticks.is_some() {
        benchmark.ticks = args.ticks.unwrap();
    } else if args.interactive {
        println!("Enter the number of ticks for this benchmark set.");
        benchmark.ticks = prompt_until_allowed_val_in_range(1..std::u32::MAX);
    } else {
        eprintln!("You must define a number of ticks!");
        exit(1);
    }

    if args.runs.is_some() {
        benchmark.runs = args.runs.unwrap();
    } else if args.interactive {
        println!("Enter the number of runs for this benchmark set.");
        benchmark.runs = prompt_until_allowed_val_in_range(1..std::u32::MAX);
    } else {
        eprintln!("You must define a number of runs!");
        exit(1);
    }

    if args.google_drive_folder.is_some() {
        google_drive_folder = args.google_drive_folder.as_ref().unwrap().clone();
    } else if args.interactive {
        println!("Enter a shared google drive folder containing the maps of this benchmark set. (optional)");
        google_drive_folder = prompt_until_empty_str(true);
    }
    handle_map_dl_links(args, &google_drive_folder, &mut benchmark);

    if args.mods_dirty.is_some() {
        mod_list = process_mod_list(args.mods_dirty.as_ref().unwrap().clone());
        benchmark.mods = mod_list;
    } else if args.interactive {
        println!("Enter a comma separated list of mods, empty for vanilla. Special response \"__CURRENT__\" will add currently enabled mods.");
        let raw_mod_list = prompt_until_empty_str(true);
        benchmark.mods = process_mod_list(raw_mod_list);
    }

    if !args.interactive && args.resave {
        println!("Finalizing resaving...");
    }
    let path_to_sha256_tuple = handle.join().unwrap();
    for (a_map, the_hash) in path_to_sha256_tuple {
        let map_struct = Map::new(a_map.file_name().unwrap().to_str().unwrap(), &the_hash, "");
        benchmark.maps.push(map_struct);
    }

    assert!(!set_name.is_empty());
    assert!(!benchmark.maps.is_empty());
    assert!(benchmark.runs > 0);
    assert!(benchmark.ticks > 0);
    write_benchmark_set_to_file(
        &set_name,
        benchmark,
        args.overwrite,
        ProcedureFileKind::Local,
        args.interactive,
    );
}

fn process_mod_list(raw_mod_list: String) -> Vec<Mod> {
    let mut found_mods = Vec::new();
    let mod_tuples = slice_mods_from_csv(&raw_mod_list);
    for (name, vers) in mod_tuples {
        if name == "__CURRENT__" {
            println!("it's a __CURRENT__! not yet implemented!",);
        } else {
            match get_mod_info(&name, &vers) {
                Some(m) => (found_mods.push(m)),
                _ => (eprintln!("Error! Could not download mod {}", name)),
            }
        }
    }
    found_mods
}

fn slice_mods_from_csv(s: &str) -> Vec<(String, String)> {
    let mut vals = Vec::new();
    if s.is_empty() {
        return vals;
    }
    for indiv_mod in s.split(',') {
        let mut indiv_mod_owned = indiv_mod.to_owned();
        if indiv_mod_owned.ends_with('_') {
            indiv_mod_owned.push('_');
        }
        let sliced_indiv_mod: Vec<_> = indiv_mod_owned.split('_').collect();
        let mod_name;
        let mod_version;
        if sliced_indiv_mod.len() < 2 {
            mod_name = sliced_indiv_mod[0].to_string();
            mod_version = "".to_string();
            vals.push((mod_name, mod_version));
        } else {
            mod_name = sliced_indiv_mod[0..(sliced_indiv_mod.len() - 1)].join("_");
            mod_version = sliced_indiv_mod[sliced_indiv_mod.len() - 1].to_string();
            vals.push((mod_name, mod_version));
        }
    }
    vals
}

fn handle_map_dl_links(args: &UserArgs, google_drive_folder: &str, benchmark: &mut BenchmarkSet) {
    if !google_drive_folder.is_empty() {
        if !google_drive_folder.starts_with("https://drive.google.com/drive/") {
            eprintln!("Google Drive URL didn't match expected format!");
            exit(1);
        }
        let map_names = benchmark.maps.clone().into_iter().map(|x| x.name).collect();
        if let Some(resp) =
            get_download_links_from_google_drive_by_filelist(map_names, &google_drive_folder)
        {
            for (fname, dl_link) in resp {
                for mut map in &mut benchmark.maps {
                    if map.name == fname {
                        map.download_link = dl_link.clone();
                    }
                }
            }
            for map in &benchmark.maps {
                if map.download_link.is_empty() {
                    println!("WARN: you specified a google drive folder but we didn't find the map {:?} in it!", map.name);
                }
            }
        }
    } else if args.interactive {
        println!("Specify map downloads individually?");
        if prompt_until_allowed_val(&["y".to_string(), "n".to_string()]) == "y" {
            for mut map in &mut benchmark.maps {
                println!("Please enter a download link for the file {}", map.name);
                let dl_link = prompt_until_empty_str(true);
                map.download_link = dl_link;
            }
        };
    } else {
        println!("WARN: google drive folder was not provided, skipping..");
    }
}

fn create_meta_from_args(args: &UserArgs) {
    let meta_set_name;
    let mut meta_set_members = Vec::new();
    if args.meta_set_name.is_some() {
        meta_set_name = args.meta_set_name.as_ref().unwrap().to_owned();
    } else if args.interactive {
        println!("Enter a name for this new meta set.");
        meta_set_name = prompt_until_empty_str(false);
    } else {
        unreachable!("Meta set name was none, and interactive mode was off!");
    }
    if args.meta_set_members.is_some() {
        meta_set_members = slice_members_from_csv(&args.meta_set_members.as_ref().unwrap());
    } else if args.interactive {
        println!("Enter a comma separated list of benchmark/meta sets.");
        meta_set_members = slice_members_from_csv(&prompt_until_empty_str(false));
    }
    if meta_set_members.is_empty() {
        eprintln!("No members contained within this meta set!");
        exit(1);
    }
    write_meta_to_file(
        &meta_set_name,
        meta_set_members,
        args.overwrite,
        ProcedureFileKind::Local,
    );
}

pub fn slice_members_from_csv(s: &str) -> Vec<String> {
    let mut vals = Vec::new();
    for member in s.split(',') {
        vals.push(member.trim().to_string());
    }
    vals
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
