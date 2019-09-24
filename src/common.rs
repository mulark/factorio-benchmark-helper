extern crate directories;

use std::path::PathBuf;
use std::fs;
use super::config::read_config_file;
use directories::ProjectDirs;


pub fn get_data_path() -> PathBuf {
    if let Some(projdir) = ProjectDirs::from("","","factorio_benchmark_helper") {
        return projdir.data_dir().to_path_buf();
    }
    else
    {
        match std::env::current_dir() {
            Ok(m) => return m.join("factorio_benchmark_helper"),
            Err(e) => panic!(e.to_string()),
        }
    }
}

pub fn get_factorio_path() -> PathBuf {
    let sanitize = match read_config_file("factorio_path") {
        Some(m) => m,
        _ => find_steam_version(),
    };
    let possible_factorio_path: PathBuf = PathBuf::from(sanitize);
    if possible_factorio_path.is_dir() {
        let executable_name = if cfg!(Windows) {
            "factorio.exe"
        } else {
            "factorio"
        };
        let dir_content = match fs::read_dir(possible_factorio_path) {
            Ok(m) => m,
            Err(e) => panic!(e.to_string()),
        };

    }
    return PathBuf::new();
}

fn find_steam_version() -> String {

    return "test".to_string();
}
