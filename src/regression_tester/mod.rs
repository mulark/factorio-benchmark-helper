//! Module for running regression tests against Factorio versions.
mod headless_downloader;

mod regression_db;

use crate::regression_tester::regression_db::put_testcase_to_db;
use crate::regression_tester::regression_db::get_scenarios;
use std::collections::HashMap;
use crate::util::query_system_cpuid;
use crate::benchmark_runner::parse_stdout_for_verbose_data;
use crate::benchmark_runner::parse_stdout_for_execution_time;
use crate::benchmark_runner::parse_stdout_for_factorio_version;
use crate::benchmark_runner::run_factorio_benchmark;
use crate::util::fbh_mod_use_dir;
use crate::benchmark_runner::SimpleBenchmarkParams;
use crate::regression_tester::headless_downloader::get_unpacked_executables;
use crate::regression_tester::headless_downloader::unpack_headless_version;
use crate::regression_tester::headless_downloader::get_local_headless_versions;
use megabase_index_incrementer::MegabaseMetadata;
use megabase_index_incrementer::FactorioVersion;
use std::collections::HashSet;
use std::path::PathBuf;
use crate::util::sha256sum;
use crate::util::factorio_save_directory;
use ureq::Agent;
use std::io;
use std::io::Read;
use megabase_index_incrementer::Megabases;
use crate::regression_tester::regression_db::put_scenario_to_db;
use crate::regression_tester::headless_downloader::download_nonlocal_versions;

lazy_static! {
    /// The subfolder where any applicable megabases are to be stored.
    static ref REGRESSION_TEST_SUBFOLDER: PathBuf
        = factorio_save_directory().join("regression-test");
    static ref MEGABASES: Megabases = fetch_megabase_list().unwrap();
}

const RECIPE_VERSIONS: [FactorioVersion; 3] = [
    FactorioVersion::new(0,16,51),
    FactorioVersion::new(0,17,0),
    FactorioVersion::new(0,17,60),
];

/// A single map, ran within one execution of the program, across many different
/// Factorio Versions.
#[derive(Default, Debug, Clone)]
pub struct RegressionScenario {
    /// The optionally present id of this scenario in the database. None until we
    /// know it.
    pub db_id: Option<u32>,
    pub map_name: String,
    pub factorio_version: FactorioVersion,
    pub platform: String,
    pub cpuid: String,
    pub sha256: String,
    pub author: String,
    /// The versions for which this scenario has testcases. None unless queried.
    pub versions: Option<Vec<FactorioVersion>>,
    pub test_instances: Vec<RegressionTestInstance>,
}

/// A single map ran in a single version of Factorio.
#[derive(Default, Debug, Clone)]
pub struct RegressionTestInstance {
    pub factorio_version: FactorioVersion,
    pub runs: u32,
    pub ticks: u32,
    pub execution_time: f64,
    pub verbose_data: Vec<String>,
}

/// Runs regression tests against Factorio
/// A value of `true` in clean will run all available versions against all maps
/// A value of `false` will only run new maps and/or new versions.
pub fn run_regression_tests(clean: bool, single_map_path: Option<&PathBuf>) {
    println!("Attempting to run regression tests");

    let already_ran_scenarios = if !clean {
        get_scenarios().unwrap_or_default()
    } else {
        HashMap::new()
    };


    if let Ok(validated) = fetch_files() {
        println!("Fetched all files");
        let mut megabases_to_run = MEGABASES.saves.clone();
        megabases_to_run.retain(|save| validated.contains(&save.sha256));
        if let Some(single_map) = single_map_path {
            megabases_to_run.clear();
            megabases_to_run.push(megabase_index_incrementer::populate_metadata(single_map).unwrap());
        }
        println!("Using set of files {:#?}", megabases_to_run);
        let mut least_seen_version = FactorioVersion::new(100,100,100);
        for save in &megabases_to_run {
            if save.factorio_version < least_seen_version {
                least_seen_version = save.factorio_version;
            }
        }
        if let Ok(mut headless_versions) = get_local_headless_versions() {
            headless_versions.retain(|(vers, _paths)| vers >= &least_seen_version);
            let mut version_unpacking_jhs = vec![];
            for headless_tuple in headless_versions {
                let version = headless_tuple.0;
                version_unpacking_jhs.push(std::thread::spawn(move || {
                    unpack_headless_version(version)
                }));
            }
            for jh in version_unpacking_jhs {
                let _ = jh.join().unwrap();
            }
            println!("Unpacked all headless versions");
        }

        // The quantity of versions each recipe can be tested in, based on
        // available unpacked factorio executables.
        let mut recipe_versions: HashMap<FactorioVersion, u32> = HashMap::new();

        if let Ok(unpacked) = get_unpacked_executables() {
            for (fv, _p) in &unpacked {
                let mut fv_recipe = FactorioVersion::default();
                for recipe in RECIPE_VERSIONS.iter() {
                    if recipe > fv {
                        break;
                    }
                    fv_recipe = *recipe;
                }
                let recipe_ct = recipe_versions.entry(fv_recipe).or_insert(0u32);
                *recipe_ct += 1;
            }
            for save in megabases_to_run {
                let mut fv_recipe = FactorioVersion::default();
                for recipe in RECIPE_VERSIONS.iter() {
                    if recipe > &save.factorio_version {
                        break;
                    }
                    fv_recipe = *recipe;
                }
                // Skip testing saves if theres 0-1 recipe compatible versions
                // available.
                if let Some(num_exes) = recipe_versions.get(&fv_recipe) {
                    if num_exes <= &1 {
                        continue;
                    }
                } else {
                    continue;
                }
                println!("Running save {:?}", save);
                let mut scenario = RegressionScenario {
                    db_id: None,
                    author: save.author.unwrap_or_default(),
                    cpuid: query_system_cpuid(),
                    factorio_version: save.factorio_version,
                    platform: "linux64 headless".to_owned(),
                    map_name: save.name.clone(),
                    sha256: save.sha256,
                    versions: None,
                    test_instances: vec![],
                };
                for factorio_install in &unpacked {
                    if factorio_install.0 < save.factorio_version {
                        continue;
                    }
                    if !clean {
                        if let Some(entry) = already_ran_scenarios.get(&scenario.sha256) {
                            if entry.author == scenario.author
                                    && entry.factorio_version == scenario.factorio_version
                                    && entry.cpuid == scenario.cpuid
                                    && entry.platform == scenario.platform
                                    && entry.map_name == scenario.map_name {
                                if let Some(vers_tested_before) = &entry.versions {
                                    if vers_tested_before.contains(&factorio_install.0) {
                                        println!("Skipping testing {} with version {} \
                                        as we already have a testcase for it"
                                        , scenario.map_name, factorio_install.0.to_string());
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    println!("In version {}", factorio_install.0.to_string());
                    let param = SimpleBenchmarkParams {
                        map_path: if let Some(map_path) = single_map_path {
                            map_path.clone()
                        } else {
                            REGRESSION_TEST_SUBFOLDER.join(&save.name)
                        },
                        mod_directory: fbh_mod_use_dir(),
                        mods: vec![],
                        runs: 10,
                        ticks: 100,
                    };
                    if let Some(stdout) = run_factorio_benchmark(&factorio_install.1, &param) {
                        let parsed_fv = parse_stdout_for_factorio_version(&stdout);
                        if Some(factorio_install.0) != parsed_fv {
                            eprintln!("Error, Factorio version {:?} didn't match what was supposed to be ran, {:?}",
                                Some(factorio_install.0), parsed_fv);
                            std::process::exit(1);
                        }
                        let instance = RegressionTestInstance {
                            factorio_version: factorio_install.0,
                            runs: param.runs,
                            ticks: param.ticks,
                            execution_time: parse_stdout_for_execution_time(&stdout).unwrap_or_default(),
                            verbose_data: parse_stdout_for_verbose_data(&stdout),
                        };

                        scenario.test_instances.push(instance);
                    }
                }
                if !already_ran_scenarios.contains_key(&scenario.sha256) || clean {
                    put_scenario_to_db(scenario);
                } else if let Some(preexist ) = already_ran_scenarios.get(&scenario.sha256) {
                    let scenario_id = preexist.db_id.unwrap();
                    for testcase in scenario.test_instances {
                        put_testcase_to_db(testcase, scenario_id);
                    }
                }
            }
        }
    } else {
        eprintln!("Error fetching files");
    }
}

/// Fetches all headless versions of Factorio from factorio.com, and all saves
/// defined in the technicalfactorio megabase index.
/// Returns a vector of the sha256sums of the saves downloaded.
fn fetch_files() -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    println!("Fetching files");
    let client = Agent::new();
    let jh = {
        std::thread::spawn(move || {
            // Gather all available Factorio headless versions.
            download_nonlocal_versions(&client);
        })
    };
    let mut valid_shas = HashSet::new();
    let mut jhs = Vec::new();

    let mut files_on_disk = HashSet::new();
    if REGRESSION_TEST_SUBFOLDER.exists() {
        let entries = std::fs::read_dir(&*REGRESSION_TEST_SUBFOLDER)?;
        for file in entries {
            let path = file?.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "zip" {
                        if let Some(fname) = path.file_name() {
                            files_on_disk.insert(fname.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
        println!("Read files from disk");
    }

    for save in &*MEGABASES.saves {
        if files_on_disk.contains(&save.name) {
            // check sha
            {
                let save = save.clone();
                jhs.push(std::thread::spawn(move || {
                    let sha = sha256sum(&*REGRESSION_TEST_SUBFOLDER.join(&save.name));
                    if sha != save.sha256 {
                        download_single_save(&save)
                    } else {
                        Ok(sha)
                    }
                }));
            }
        } else if save.download_link_mirror.is_some() {
            // download it, then check sha
            {
                let save = save.clone();
                jhs.push(std::thread::spawn(move || {
                    download_single_save(&save)
                }));
            }
        } else {
            eprintln!("Warning, unable to use {} because no mirrored \
                download link is defined.", save.name
            );
            eprintln!("Please download the map at {} and then move it to {:?} to test with this map.",
                save.source_link, *REGRESSION_TEST_SUBFOLDER);
            eprintln!("Continuing anyway...");
        }
    }

    for jh in jhs {
        if let Ok(sha) = jh.join().unwrap() {
            valid_shas.insert(sha);
        }
    }
    jh.join().unwrap();

    Ok(valid_shas)
}

fn download_single_save(save: &MegabaseMetadata) -> Result<String, io::Error> {
    if !REGRESSION_TEST_SUBFOLDER.exists() {
        std::fs::create_dir_all(&*REGRESSION_TEST_SUBFOLDER)?;
    }
    if let Some(mirror) = &save.download_link_mirror {
        let resp = ureq::get(mirror).call();
        let sha = if resp.status() == 200 {
            let p = &*REGRESSION_TEST_SUBFOLDER.join(&save.name);
            let mut buf = Vec::new();
            let mut reader = resp.into_reader();
            let mut sha = String::new();
            if reader.read_to_end(&mut buf).is_ok()
            && std::fs::write(p, buf).is_ok() {
                sha = sha256sum(p);
            }
            sha
        } else {
            eprintln!("Could not download file {}, {:?}", save.name, resp);
            return Err(io::Error::new(io::ErrorKind::Other, "Could not download file"));
        };
        if sha != save.sha256 {
            return Err(io::Error::new(io::ErrorKind::Other, "Downloaded file didn't match sha256 previously recorded"));
        }
        return Ok(sha);
    }
    Err(io::Error::new(io::ErrorKind::Other, "No download link was defined"))
}

/// Downloads and parses the technicalfactorio megabase index.
fn fetch_megabase_list() -> Result<Megabases, Box<dyn std::error::Error>> {
    let resp = ureq::get("https://raw.githubusercontent.com/technicalfactorio/\
        technicalfactorio/master/megabase_index_incrementer/megabases.json")
        .call();
    if resp.status() == 200 {
        let s = resp.into_string()?;
        Ok(serde_json::from_str(&s)?)
    } else {
        eprintln!("Could not download listing of megabases");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_stdout_parse_factorio_version() {
        let snippet = "   0.000 2020-07-27 23:12:46; Factorio 0.18.32 (build 52799, linux64, headless)\
        \n   0.010 Operating system: Linux (Arch rolling)";
        assert_eq!(parse_stdout_for_factorio_version(snippet), Some(FactorioVersion::new(0,18,32)));
    }
}
