use crate::util::config_file::fbh_write_config_file;
use crate::procedure_file::update_master_json;
use crate::util::{generic_read_configuration_setting, setup_database};
use directories::ProjectDirs;
use ini::Ini;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;


pub fn initialize() -> Result<(), std::io::Error> {
    if !fbh_data_path().exists() {
        std::fs::create_dir_all(fbh_data_path())?;
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
    if !fbh_procedure_directory().exists() {
        std::fs::create_dir(fbh_procedure_directory())?;
    }
    // Will write config file with forward compatibility if needed
    fbh_write_config_file()?;
    if !fbh_results_database().exists() {
        setup_database(true);
    }
    update_master_json();
    Ok(())
}

pub fn fbh_procedure_directory() -> PathBuf {
    fbh_data_path().join("procedures").join("")
}

pub fn fbh_procedure_json_master_file() -> PathBuf {
    fbh_procedure_directory().join("master.json")
}

pub fn fbh_procedure_json_local_file() -> PathBuf {
    fbh_procedure_directory().join("local.json")
}


pub fn fbh_data_path() -> PathBuf {
    //Data paths for this program, not Factorio
    if let Some(projdir) = ProjectDirs::from("", "", "factorio-benchmark-helper") {
        projdir.data_dir().to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(m) => m.join("factorio-benchmark-helper"),
            Err(e) => panic!(e.to_string()),
        }
    }
}

pub fn fbh_results_database() -> PathBuf {
    fbh_data_path().join("results.db")
}

pub fn fbh_cache_path() -> PathBuf {
    fbh_data_path().join("cache").join("")
}

pub fn fbh_mod_dl_dir() -> PathBuf {
    fbh_cache_path().join("mods").join("")
}

pub fn fbh_mod_use_dir() -> PathBuf {
    fbh_mod_dl_dir().join("active").join("")
}

pub fn fbh_save_dl_dir() -> PathBuf {
    fbh_cache_path().join("saves").join("")
}

pub fn fbh_config_file() -> PathBuf {
    fbh_data_path().join("config.ini")
}

fn fbh_read_configuration_setting(key: &str) -> Option<String> {
    generic_read_configuration_setting(fbh_config_file(), key)
}
