/*
Factorio Benchmark Helper Paths
usually lives in ~/.local/share/factorio_benchmark_helper/
*/

use crate::database::setup_database;
use directories::{ProjectDirs, BaseDirs};
use std::path::PathBuf;
use std::fs::{OpenOptions};
use std::io::Write;
use ini::Ini;
use crate::util::{generic_read_configuration_setting};

pub fn initialize() -> Result<(),std::io::Error> {
    if !fbh_data_path().exists() {
        std::fs::create_dir(fbh_data_path())?;
    }
    if !fbh_cache_path().exists() {
        std::fs::create_dir(fbh_cache_path())?;
    }
    if !fbh_mod_dl_dir().exists() {
        std::fs::create_dir(fbh_mod_dl_dir())?;
    }
    if !fbh_save_dl_dir().exists() {
        std::fs::create_dir(fbh_save_dl_dir())?;
    }
    if !fbh_mod_use_dir().exists() {
        std::fs::create_dir(fbh_mod_use_dir())?;
    }
    if !fbh_data_path().join("config.ini").exists() {
        fbh_init_config_file()?;
    }
    if !fbh_results_database().exists() {
        setup_database(true);
    }
    Ok(())
}

pub fn fbh_init_config_file() -> Result<(),std::io::Error> {
    if let Ok(mut file) = OpenOptions::new()
        .write(true)
        .create(true)
        .open(fbh_data_path().join("config.ini")) {
            writeln!(file, "; Comments begin with a semicolon ';'")?;
            writeln!(file, "; Default value for property is commented out")?;
            writeln!(file, "; use_steam_version=true")?;
            writeln!(file)?;
            writeln!(file, "; The path to Factorio, if Steam version is not utilized or could not be found")?;
            writeln!(file, "; factorio_path=")?;
            writeln!(file)?;
            writeln!(file, "; To procure a file listing of a google drive folder (even publically shared ones), this API key must be provided")?;
            writeln!(file, "; No API key is needed for normal operations, like downloading dependencies")?;
            writeln!(file, "; google-drive-api-key=")?;
    }
    Ok(())
}

pub fn fbh_data_path() -> PathBuf {
    //Data paths for this program, not Factorio
    if let Some(projdir) = ProjectDirs::from("","","factorio_benchmark_helper") {
        return projdir.data_dir().to_path_buf();
    } else {
        match std::env::current_dir() {
            Ok(m) => return m.join("factorio_benchmark_helper"),
            Err(e) => panic!(e.to_string()),
        }
    }
}

pub fn fbh_results_database() -> PathBuf {
    return fbh_data_path().join("results.db");
}


pub fn fbh_cache_path() -> PathBuf {
    fbh_data_path().join("cache")
        .join("")
}

pub fn fbh_mod_dl_dir() -> PathBuf {
    fbh_cache_path().join("mods")
        .join("")
}

pub fn fbh_mod_use_dir() -> PathBuf {
    fbh_mod_dl_dir().join("active")
        .join("")
}

pub fn fbh_save_dl_dir() -> PathBuf {
    fbh_cache_path().join("saves")
        .join("")
}

pub fn fbh_config_file() -> PathBuf {
    let f = fbh_data_path().join("config.ini");
    return f
}

pub fn fbh_read_configuration_setting(key: &str) -> Option<String> {
    generic_read_configuration_setting(fbh_config_file(), key)
}
