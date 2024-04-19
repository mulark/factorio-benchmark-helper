#[macro_use]
extern crate lazy_static;
extern crate bincode;
extern crate clap;
extern crate directories;
#[macro_use]
extern crate log;
extern crate percent_encoding;
extern crate regex;
extern crate serde;
extern crate serde_json;
extern crate sha2;

mod performance_results;

use crate::procedure_file::print_all_benchmarks;

use crate::backblaze::upload_files_to_backblaze;
use crate::benchmark_runner::determine_saved_factorio_version;
use crate::performance_results::collection_data::Mod;
use crate::procedure_file::get_metas_from_meta;
use crate::procedure_file::get_sets_from_meta;
use crate::procedure_file::read_meta_from_file;
use crate::procedure_file::write_meta_to_file;
use crate::regression_tester::run_regression_tests;
use crate::util::fbh_save_dl_dir;
use crate::util::hash_saves;
use crate::util::prompt_until_existing_folder_path;
use regex::Regex;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::exit;

#[cfg(target_os = "linux")]
mod regression_tester;

mod backblaze;

mod benchmark_runner;
use benchmark_runner::run_benchmarks_multiple;

mod procedure_file;
mod util;
use util::{
    add_options_and_parse, factorio_save_directory, get_mod_info,
    prompt_until_allowed_val, prompt_until_allowed_val_in_range,
    prompt_until_empty_str, read_benchmark_set_from_file,
    write_benchmark_set_to_file, BenchmarkSet, ProcedureFileKind,
    ProcedureKind, UserArgs,
};

lazy_static! {
    static ref MOD_VERSION_MATCHER: Regex =
        Regex::new(r"_\{1,3}.\{1,4}.\{1,4}").unwrap();
}

fn main() {
    let mut parsed_args = add_options_and_parse();
    match util::initialize() {
        Ok(_) => (),
        Err(e) => {
            println!("Failed to initialize Factorio Benchmark Helper");
            panic!("{}", e);
        }
    }
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
        || args.create_meta
        || args.regression_test)
    {
        if args.interactive {
            println!("Choose a suitable course of action.");
            println!(
                "1: Commit a benchmark or meta set to the master.json file \
                from the local.json file."
            );
            println!("2: Run a benchmark.");
            println!("3: Run a metabenchmark.");
            println!("4: Create a new benchmark.");
            println!("5: Create a new metabenchmark.");
            println!("6: Run a regression test.");
            match prompt_until_allowed_val(&[1, 2, 3, 4, 5, 6]) {
                1 => args.commit_flag = true,
                2 => args.run_benchmark = true,
                3 => args.run_meta = true,
                4 => args.create_benchmark = true,
                5 => args.create_meta = true,
                6 => {
                    args.regression_test = true;
                    println!(
                        "Selected regression test. Enter scope of benchmarks"
                    );
                    println!("1. Differential test (only runs benchmarks of new maps or Factorio versions)");
                    println!("2. Clean test (runs all maps and Factorio version combinations)");
                    println!("3. Single user provided map");
                    match prompt_until_allowed_val(&[1, 2, 3]) {
                        1 => args.regression_test_clean = false,
                        2 => args.regression_test_clean = true,
                        3 => {
                            args.regression_test_clean = true;
                            loop {
                                println!("Enter path to user provded map");
                                let p = prompt_until_empty_str(false);
                                let p = PathBuf::from(p);
                                if p.exists() {
                                    args.regression_test_path = Some(p);
                                    break;
                                } else {
                                    println!("Supplied path doesn't exist");
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                _ => {
                    unreachable!("How did you match to this after getting an allowed value?");
                }
            }
        } else {
            eprintln!(
                "You provided args but didn't pick \
                    commit/benchmark/meta/create-benchmark/create-meta/regression-test or \
		            interactive!"
            );
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
    } else if args.regression_test {
        run_regression_tests(
            args.regression_test_clean,
            args.regression_test_path.as_ref(),
        );
    }
}

fn perform_commit(args: &mut UserArgs) {
    if args.commit_name.is_none() || args.commit_type.is_none() {
        if args.commit_type.is_none() {
            if args.interactive {
                println!("Interactively committing to master.json");
                println!(
                    "Please enter the type of set you wish to commit to \
		                  the master.json file. Allowed values: benchmark, meta"
                );
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
    match commit_type {
        ProcedureKind::Benchmark => {
            if let Some(benchmark_set) = read_benchmark_set_from_file(
                commit_name,
                ProcedureFileKind::Local,
            ) {
                write_benchmark_set_to_file(
                    commit_name,
                    benchmark_set,
                    args.overwrite,
                    ProcedureFileKind::Master,
                    false.into(),
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
        }
        ProcedureKind::Meta => {
            if let Some(meta_set) =
                read_meta_from_file(commit_name, ProcedureFileKind::Local)
            {
                if args.commit_recursive {
                    println!(
                        "Selected recursive, committing all members of this meta"
                    );
                    let meta_sets = get_metas_from_meta(
                        commit_name.to_string(),
                        ProcedureFileKind::Local,
                    );
                    let benchmark_sets = get_sets_from_meta(
                        commit_name.to_string(),
                        ProcedureFileKind::Local,
                    );
                    for (name, set) in benchmark_sets {
                        write_benchmark_set_to_file(
                            &name,
                            set,
                            args.overwrite,
                            ProcedureFileKind::Master,
                            args.interactive.into(),
                        )
                    }
                    for meta in meta_sets {
                        if let Some(members) =
                            read_meta_from_file(&meta, ProcedureFileKind::Local)
                        {
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
        }
        ProcedureKind::Both => unreachable!(),
    }
}

fn convert_args_to_benchmark_run(
    args: &mut UserArgs,
) -> HashMap<String, BenchmarkSet> {
    if args.benchmark_set_name.is_none() && args.interactive {
        println!("Available benchmarks to run: ");
        print_all_benchmarks();
        println!("Enter name of a benchmark you wish to run.");
        let mut s = String::new();
        loop {
            std::io::stdin()
                .read_line(&mut s)
                .expect("Failed to read line from stdin");
            if read_benchmark_set_from_file(&s, ProcedureFileKind::Master)
                .is_some()
                || read_benchmark_set_from_file(&s, ProcedureFileKind::Local)
                    .is_some()
            {
                args.benchmark_set_name = Some(s);
                break;
            }
            eprintln!("Failed to find benchmark set with provided name");
            s.clear();
        }
    }
    let name = args.benchmark_set_name.take().unwrap();
    let mut hash_map = HashMap::default();
    let local = read_benchmark_set_from_file(&name, ProcedureFileKind::Local);
    let master = read_benchmark_set_from_file(&name, ProcedureFileKind::Master);
    if local.is_some() || master.is_some() {
        if master.is_some() && local.is_some() && master != local {
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

fn convert_args_to_meta_benchmark_runs(
    args: &UserArgs,
) -> HashMap<String, BenchmarkSet> {
    let name = args.meta_set_name.as_ref().unwrap().to_owned();
    let local = read_meta_from_file(&name, ProcedureFileKind::Local);
    let master = read_meta_from_file(&name, ProcedureFileKind::Master);
    if local.is_some() || master.is_some() {
        if local.is_some() && master.is_some() && local != master {
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
    let folder;
    let map_paths;
    let mod_list;
    let mut benchmark = BenchmarkSet::default();

    if args.benchmark_set_name.is_some() {
        set_name = args.benchmark_set_name.as_ref().unwrap().clone();
    } else if args.interactive {
        println!(
            "No benchmark set name was defined, enter a benchmark set name."
        );
        set_name = prompt_until_empty_str(false);
    } else {
        eprintln!(
            "Failed to create a benchmark set because no name was defined!"
        );
        exit(1);
    }

    if args.folder.is_some() {
        folder = args.folder.as_ref().unwrap().clone();
    } else if args.interactive {
        println!("No folder was defined, enter a relative folder in your saves directory, \
            or absolute directory, or empty for the saves directory.");
        folder = prompt_until_existing_folder_path(true);
    } else {
        unreachable!();
    }

    let holder = get_map_paths(&folder);
    map_paths = holder.0;
    benchmark.save_subdirectory = holder.1;
    if map_paths.is_empty() {
        eprintln!("Supplied folder found no maps! {:?}", &folder);
        exit(1);
    } else {
        println!("Found the following maps:");
        for map in map_paths.iter() {
            println!("{:?}", map);
        }
    }
    let mut maps_hashmap = hash_saves(&map_paths);

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

    handle_map_dl_links(args, &mut benchmark);

    if args.mods_dirty.is_some() {
        mod_list = process_mod_list(&args.mods_dirty.as_ref().unwrap());
        benchmark.mods = mod_list;
    } else if args.interactive {
        println!("Enter a comma separated list of mods, empty for vanilla. Special response \"__CURRENT__\" will add currently enabled mods.");
        let raw_mod_list = prompt_until_empty_str(true);
        benchmark.mods = process_mod_list(&raw_mod_list);
    }

    let save_subdirectory =
        benchmark.save_subdirectory.clone().unwrap_or_default();
    let subdir = save_subdirectory.to_str().unwrap().to_owned();

    println!("Finding save versions");
    let mut vers = Vec::new();
    for path in map_paths.iter() {
        let single_vers = determine_saved_factorio_version(&path);
        vers.push((path, single_vers));
    }
    println!("Finished determining the saved versions of each map.");
    println!("Attempting upload to Backblaze-b2...");

    match upload_files_to_backblaze(&subdir, &map_paths) {
        Ok(uploaded_files) => {
            println!("Finished uploading files");
            for (filepath, dl_link) in uploaded_files {
                let map = maps_hashmap.get_mut(&filepath).unwrap();
                map.download_link = dl_link;
            }
        }
        Err(msg) => {
            eprintln!("Failed to upload to backblaze");
            eprintln!("Reason: {}", msg);
            eprintln!("Continuing without populating the map_dl field...");
        }
    };

    for (path, vers) in vers {
        if let Some(map) = maps_hashmap.get_mut(path) {
            map.min_compatible_version = vers.unwrap_or_default();
        }
    }
    benchmark.maps = maps_hashmap.values().map(|x| x.to_owned()).collect();

    assert!(!set_name.is_empty());
    assert!(!benchmark.maps.is_empty());
    assert!(benchmark.runs > 0);
    assert!(benchmark.ticks > 0);

    println!("Writing benchmark json...");
    write_benchmark_set_to_file(
        &set_name,
        benchmark,
        args.overwrite,
        ProcedureFileKind::Local,
        args.interactive.into(),
    );
}

fn process_mod_list(raw_mod_list: &str) -> BTreeSet<Mod> {
    let mut found_mods = BTreeSet::new();
    let mod_tuples = slice_mods_from_csv(&raw_mod_list);
    for (name, vers) in mod_tuples {
        if name == "__CURRENT__" {
            println!("it's a __CURRENT__! not yet implemented!",);
        } else {
            match get_mod_info(&name, &vers) {
                Some(m) => {
                    found_mods.insert(m);
                }
                _ => {
                    eprintln!("Error! Could not download mod {}", name);
                    exit(1);
                }
            }
        };
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
            mod_name =
                sliced_indiv_mod[0..(sliced_indiv_mod.len() - 1)].join("_");
            mod_version =
                sliced_indiv_mod[sliced_indiv_mod.len() - 1].to_string();
            vals.push((mod_name, mod_version));
        }
    }
    vals
}

/// Handle adding map download links individually to each map if running interactively.
fn handle_map_dl_links(args: &UserArgs, benchmark: &mut BenchmarkSet) {
    if args.interactive {
        println!("Specify map downloads individually?");
        if prompt_until_allowed_val(&["y".to_string(), "n".to_string()]) == "y"
        {
            let mut maps_to_update = Vec::new();
            for map in &mut benchmark.maps.iter() {
                println!(
                    "Please enter a download link for the file {}",
                    map.name
                );
                let dl_link = prompt_until_empty_str(true);
                let mut map2 = map.clone();
                map2.download_link = dl_link;
                maps_to_update.push(map2);
            }
            benchmark.maps.clear();
            for m in maps_to_update {
                benchmark.maps.insert(m);
            }
        }
    }
}

fn create_meta_from_args(args: &UserArgs) {
    let meta_set_name;
    let mut meta_set_members = BTreeSet::new();
    if args.meta_set_name.is_some() {
        meta_set_name = args.meta_set_name.as_ref().unwrap().to_owned();
    } else if args.interactive {
        println!("Enter a name for this new meta set.");
        meta_set_name = prompt_until_empty_str(false);
    } else {
        unreachable!("Meta set name was none, and interactive mode was off!");
    }
    if args.meta_set_members.is_some() {
        meta_set_members =
            slice_members_from_csv(&args.meta_set_members.as_ref().unwrap());
    } else if args.interactive {
        println!("Enter a comma separated list of benchmark/meta sets.");
        meta_set_members =
            slice_members_from_csv(&prompt_until_empty_str(false));
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

fn slice_members_from_csv(s: &str) -> BTreeSet<String> {
    let mut vals = BTreeSet::new();
    for member in s.split(',') {
        vals.insert(member.trim().to_string());
    }
    vals
}

/// Gets the paths of all maps within the specified directory. Returns a Vec of
/// the paths of the found saves, and optionally a common subdirectory.
fn get_map_paths(dir: &PathBuf) -> (Vec<PathBuf>, Option<PathBuf>) {
    let mut map_paths = Vec::new();
    assert!(dir.is_dir());
    for item in std::fs::read_dir(dir).unwrap() {
        if let Ok(item) = item {
            if let Some(extension) = item.path().extension() {
                if let Some("zip") = extension.to_str() {
                    map_paths.push(item.path());
                }
            }
        }
    }
    let subdir = find_map_subdirectory(dir);
    move_maps_to_cache(&map_paths, &subdir);
    (map_paths, subdir)
}

fn find_map_subdirectory(dir: &PathBuf) -> Option<PathBuf> {
    assert!(dir.is_dir());
    if let Ok(stripped_path) = dir.strip_prefix(factorio_save_directory()) {
        Some(stripped_path.to_path_buf())
    } else {
        dir.file_name().map(PathBuf::from)
    }
}

/// Copy the given maps into the cache directory, nested within a new subdirectory
/// if provided.
fn move_maps_to_cache(map_paths: &[PathBuf], subdir: &Option<PathBuf>) {
    let save_to_dir = if let Some(subdir) = subdir {
        fbh_save_dl_dir().join(subdir)
    } else {
        fbh_save_dl_dir()
    };
    for path in map_paths {
        let dest_path = &save_to_dir.join(&path.file_name().unwrap());
        let parent = dest_path.parent().unwrap();
        if !parent.exists() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "Couldn't create a subdirectory {:?} due to {:?}",
                    parent, e
                );
                exit(1);
            }
        }
        if let Err(e) = std::fs::copy(&path, dest_path) {
            eprintln!("Failed to copy {:?} to {:?}", &path, &dest_path);
            eprintln!("Reason: {}", e);
            exit(1);
        } else {
            println!("Copied {:?} to {:?}", &path, &dest_path);
        }
    }
}

#[cfg(test)]
mod tests {}
