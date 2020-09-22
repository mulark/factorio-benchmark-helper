use crate::performance_results::database::setup_database;
use crate::procedure_file::update_master_json;
use crate::util::config_file::fbh_write_config_file;
use directories::ProjectDirs;
use std::path::PathBuf;
use std::fs::File;
use simplelog::LevelFilter;

pub fn initialize() -> Result<(), std::io::Error> {
    if !fbh_data_path().exists() {
        std::fs::create_dir_all(fbh_data_path())?;
    }
    let config = simplelog::ConfigBuilder::new()
        .build();
    simplelog::WriteLogger::init(
        LevelFilter::Trace,
        config,
        File::create(fbh_data_path().join("fbh.log"))?
    ).expect("Another global logger already init?");

    info!("Logging initialized");
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
    if !fbh_regression_testing_dir().exists() {
        std::fs::create_dir(fbh_regression_testing_dir())?;
    }
    if !fbh_regression_headless_storage().exists() {
        std::fs::create_dir(fbh_regression_headless_storage())?;
    }
    // Will write config file with forward compatibility if needed
    fbh_write_config_file()?;
    if !fbh_results_database().exists() {
        setup_database(true, &fbh_results_database());
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

/// The data path for fbh data storage. Probably
/// ~/.local/share/factorio-benchmark-helper
pub fn fbh_data_path() -> PathBuf {
    //Data paths for this program, not Factorio
    if let Some(projdir) =
        ProjectDirs::from("", "", "factorio-benchmark-helper")
    {
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

pub fn fbh_regression_testing_dir() -> PathBuf {
    fbh_data_path().join("regression-testing").join("")
}

pub fn fbh_regression_headless_storage() -> PathBuf {
    fbh_regression_testing_dir().join("headless").join("")
}

pub fn fbh_unpacked_headless_storage() -> PathBuf {
    fbh_regression_testing_dir().join("unpacked").join("")
}
