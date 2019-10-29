/*
Factorio Benchmark Helper Paths
usually lives in .local/share/factorio_benchmark_helper/
*/


use directories::{ProjectDirs, BaseDirs};
use std::path::PathBuf;
use std::fs::{OpenOptions};
use std::io::Write;
use ini::Ini;

pub fn fbh_initialize() -> Result<(),std::io::Error> {
    std::fs::create_dir(fbh_data_path())?;
    std::fs::create_dir(fbh_cache_path())?;
    std::fs::create_dir(fbh_mod_dl_dir())?;
    std::fs::create_dir(fbh_save_dl_dir())?;
    fbh_init_config_file()?;
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

pub fn fbh_save_dl_dir() -> PathBuf {
    fbh_cache_path().join("saves")
        .join("")
}

pub fn fbh_config_file() -> PathBuf {
    let f = fbh_data_path().join("config.ini");
    return f
}

pub fn fbh_read_configuration_setting(file: PathBuf,key: &str) -> Option::<String> {
    match Ini::load_from_file(file) {
        Ok(i) => return i.get_from(None::<String>, key).map(String::from),
        Err(_e) => return None,
    };
}

pub fn read_config_file_platform_specific_value(key: &str) -> Option::<String> {
    let ini = match Ini::load_from_file(fbh_config_file()) {
        Ok(i) => i,
        Err(e) => panic!(e.to_string()),
    };
    let section = if cfg!(Windows) {
        "Windows"
    } else {
        "Linux"
    };
    let val = ini.get_from(Some(section), key).map(String::from);
    return val;
}
