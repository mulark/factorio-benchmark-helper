extern crate directories;
extern crate ini;
extern crate reqwest;
extern crate sha1;
extern crate sha2;

mod fbh_paths;
use reqwest::Response;
use std::thread::JoinHandle;
pub use fbh_paths::{
    fbh_initialize,
    fbh_init_config_file,
    fbh_data_path,
    fbh_config_file,
    fbh_cache_path,
    fbh_mod_dl_dir,
    fbh_read_configuration_setting,
    fbh_save_dl_dir,
    fbh_results_database
};

pub use crate::procedure_file::{BenchmarkSet,ModSet};
use serde::{Deserialize, Serialize};
use ini::Ini;
use std::path::PathBuf;
use directories::{ProjectDirs, BaseDirs};
mod mod_dl;
use mod_dl::{fetch_mod_deps_parallel};
mod map_dl;
use map_dl::{fetch_map_deps_parallel};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Mod {
    name: String,
    version: String,
    sha1: String,
    alt_download_link: String,
}

impl Mod {
    pub fn new(name: &str, version: &str, hash: &str) -> Mod {
        Mod {
            name: name.to_string(),
            version: version.to_string(),
            sha1: hash.to_string(),
            alt_download_link: "".to_string()
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

fn get_factorio_path() -> PathBuf {
    let use_steam_dir: bool = fbh_read_configuration_setting(fbh_config_file(), "use-steam-version").unwrap_or_default().parse::<bool>().unwrap_or(true);
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
        match fbh_read_configuration_setting(fbh_config_file(), "factorio-path").unwrap_or_default().parse::<PathBuf>() {
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
    let use_system_rw_directories: bool = fbh_read_configuration_setting(ini_path, "use-system-read-write-directories").unwrap_or_default().parse::<bool>().unwrap_or(true);

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
