
extern crate directories;
extern crate ini;
extern crate reqwest;
extern crate sha1;
extern crate sha2;
extern crate regex;

mod fbh_paths;
use reqwest::Response;
use std::thread::JoinHandle;
pub use fbh_paths::{
    initialize,
    fbh_init_config_file,
    fbh_data_path,
    fbh_config_file,
    fbh_cache_path,
    fbh_mod_dl_dir,
    fbh_mod_use_dir,
    fbh_read_configuration_setting,
    fbh_save_dl_dir,
    fbh_results_database
};

pub use crate::procedure_file::{BenchmarkSet,ModSet};
use serde::{Deserialize, Serialize};
use ini::Ini;
use std::path::PathBuf;
use directories::{ProjectDirs, BaseDirs};
use regex::Regex;
mod mod_dl;
pub use mod_dl::{fetch_mod_deps_parallel, prompt_for_mods};
mod map_dl;
pub use map_dl::{fetch_map_deps_parallel, get_download_link_from_google_drive_by_filename};

lazy_static!{
    #[derive(Debug)]
    pub static ref FACTORIO_VERSION: String = get_factorio_version().replace("Version: ","");
    static ref FACTORIO_EXECUTABLE_VERSION_LINE: Regex = Regex::new(r"Version: \d{1,2}\.\d{2,3}\.\d{2,3}").unwrap();
    //If Factorio ever goes to 3/4/4 digits for their versioning, this will break.

}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Mod {
    name: String,
    version: String,
    sha1: String,
}

impl Mod {
    pub fn new(name: &str, version: &str, hash: &str) -> Mod {
        Mod {
            name: name.to_string(),
            version: version.to_string(),
            sha1: hash.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Map {
    name: String,
    sha256: String,
    download_link: String,
    ticks: u32,
    runs: u32,
}

impl Map {
    pub fn new(name: &str, hash: &str, download_link: &str, ticks: u32, runs: u32) -> Map {
        return Map {
            name: name.to_string(),
            sha256: hash.to_string(),
            download_link: download_link.to_string(),
            ticks,
            runs,
        }
    }
}


pub fn fetch_benchmark_deps_parallel(set: BenchmarkSet) {
    println!("Fetching benchmark dependencies for this benchmark set: {}", set.name);
    let mut handles = Vec::new();
    fetch_mod_deps_parallel(set.mod_groups, &mut handles);
    fetch_map_deps_parallel(set.maps, &mut handles);
    for handle in handles {
        handle.join().expect("");
    }
}

pub fn generic_read_configuration_setting(file: PathBuf,key: &str) -> Option::<String> {
    match Ini::load_from_file(file) {
        Ok(i) => return i.get_from(None::<String>, key).map(String::from),
        Err(_e) => return None,
    };
}

fn get_factorio_path() -> PathBuf {
    let use_steam_dir: bool = fbh_read_configuration_setting("use-steam-version").unwrap_or_default().parse::<bool>().unwrap_or(true);
    if use_steam_dir {
        let base_dir = BaseDirs::new().unwrap();
        let probable_steam_path = if cfg!(Windows) {
            PathBuf::from("C:")
                .join("Program Files (x86)")
                .join("Steam")
                .join("steamapps")
                .join("common")
                .join("Factorio")
                .join("")
        } else {
            base_dir.home_dir()
                .join(".local")
                .join("share")
                .join("Steam")
                .join("steamapps")
                .join("common")
                .join("Factorio")
                .join("")
        };
        return probable_steam_path;
    } else {
        match fbh_read_configuration_setting("factorio-path").unwrap_or_default().parse::<PathBuf>() {
            Ok(path) => {
                if path.is_dir() {
                    return path;
                } else {
                    eprintln!("Could not resolve path from config file to a valid directory of a Factorio install");
                    std::process::exit(1);
                }
            },
            Err(_e) => {
                eprintln!("Could not resolve path from config file to a valid directory of a Factorio install");
                std::process::exit(1);
            },
        }
    }
}

pub fn get_executable_path() -> PathBuf {
    if cfg!(Windows) {
        return get_factorio_path().join("bin")
            .join("x64")
            .join("factorio.exe");
    } else {
        return get_factorio_path().join("bin")
            .join("x64")
            .join("factorio");
    }
}

fn get_factorio_rw_directory() -> PathBuf {
    let ini_path = get_factorio_path().join("config-path.cfg");
    let use_system_rw_directories: bool = generic_read_configuration_setting(ini_path, "use-system-read-write-directories").unwrap_or_default().parse::<bool>().unwrap_or(true);
    if use_system_rw_directories {
        if cfg!(Windows) {
            // %appdata%\Roaming\
            return BaseDirs::new().unwrap().data_dir().join("Factorio").join("");
        } else {
            // ~/.factorio/
            return BaseDirs::new().unwrap().home_dir().join(".factorio").join("");
        }
    } else {
        return get_factorio_path();
    }
}

pub fn get_saves_directory() -> PathBuf {
    return get_factorio_rw_directory().join("saves").join("");
}

fn get_factorio_version() -> String {
    //Don't call this, use FACTORIO_VERSION instead
    FACTORIO_EXECUTABLE_VERSION_LINE.captures(&String::from_utf8_lossy(&std::process::Command::new(get_executable_path())
        .arg("--version")
        .output()
        .expect("")
        .stdout))
    .unwrap()[0]
    .to_string()
}
