extern crate directories;
extern crate ini;
extern crate regex;
extern crate reqwest;
extern crate sha1;
extern crate sha2;

mod fbh_paths;
use sha2::Digest;
use std::fs::File;
use std::io::Read;
pub use fbh_paths::{
    fbh_cache_path, fbh_config_file, fbh_data_path, fbh_mod_dl_dir, fbh_mod_use_dir,
    fbh_procedure_json_local_file, fbh_procedure_json_master_file, fbh_read_configuration_setting,
    fbh_results_database, fbh_save_dl_dir, initialize,
};
use reqwest::Response;
use std::thread::JoinHandle;

pub use crate::procedure_file::{create_procedure_interactively, BenchmarkSet, ProcedureFileKind, write_procedure_to_file, read_procedure_from_file};
use directories::{BaseDirs, ProjectDirs};
use ini::Ini;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
mod mod_dl;
pub use mod_dl::{Mod, fetch_mod_deps_parallel};
mod map_dl;
pub use map_dl::{Map, fetch_map_deps_parallel, get_download_links_from_google_drive_by_filelist};

lazy_static! {
    #[derive(Debug)]
    pub static ref FACTORIO_VERSION: String = get_factorio_version().replace("Version: ","");
    static ref FACTORIO_EXECUTABLE_VERSION_LINE: Regex = Regex::new(r"Version: \d{1,2}\.\d{2,3}\.\d{2,3}").unwrap();
    //If Factorio ever goes to 3/4/4 digits for their versioning, this will break.

}

pub fn fetch_benchmark_deps_parallel(set: BenchmarkSet) {
    //TODO println!("Fetching benchmark dependencies for this benchmark set: {}", set.name);
    let mut handles = Vec::new();
    fetch_mod_deps_parallel(set.mods, &mut handles);
    fetch_map_deps_parallel(set.maps, &mut handles);
    for handle in handles {
        handle.join().expect("");
    }
}

pub fn generic_read_configuration_setting(file: PathBuf, key: &str) -> Option<String> {
    match Ini::load_from_file(file) {
        Ok(i) => return i.get_from(None::<String>, key).map(String::from),
        Err(_e) => return None,
    };
}

fn get_factorio_path() -> PathBuf {
    let use_steam_dir: bool = fbh_read_configuration_setting("use-steam-version")
        .unwrap_or_default()
        .parse::<bool>()
        .unwrap_or(true);
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
            base_dir
                .home_dir()
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
            }
            Err(_e) => {
                eprintln!("Could not resolve path from config file to a valid directory of a Factorio install");
                std::process::exit(1);
            }
        }
    }
}

pub fn get_executable_path() -> PathBuf {
    if cfg!(Windows) {
        return get_factorio_path()
            .join("bin")
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
    let use_system_rw_directories: bool =
        generic_read_configuration_setting(ini_path, "use-system-read-write-directories")
            .unwrap_or_default()
            .parse::<bool>()
            .unwrap_or(true);
    if use_system_rw_directories {
        if cfg!(Windows) {
            // %appdata%\Roaming\
            return BaseDirs::new()
                .unwrap()
                .data_dir()
                .join("Factorio")
                .join("");
        } else {
            // ~/.factorio/
            return BaseDirs::new()
                .unwrap()
                .home_dir()
                .join(".factorio")
                .join("");
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

pub fn sha256sum(file_path: PathBuf) -> String {
    let mut f = File::open(file_path).unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();
    format!("{:x}", sha2::Sha256::digest(&buf))
}

pub fn bulk_sha256(paths: Vec<PathBuf>) -> Vec<(PathBuf, String)> {
    let mut handle_holder = Vec::new();
    let mut path_sha256_tuple_holder = Vec::new();
    for path in paths {
        handle_holder.push(std::thread::spawn(move || {
            let sha256 = sha256sum(path.clone());
            (path, sha256)
        }));
    }
    for handle in handle_holder {
        let res_tuple = handle.join().unwrap();
        path_sha256_tuple_holder.push(res_tuple);
    }
    path_sha256_tuple_holder
}