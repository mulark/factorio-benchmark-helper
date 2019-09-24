extern crate ini;
extern crate directories;

use std::path::PathBuf;
use ini::Ini;
use super::common;

pub fn setup_config_file(regen_config_file: bool) {
    let config_file_path: PathBuf = common::get_data_path().join("config.ini");
    if !config_file_path.is_file() || regen_config_file == true
    {
        input_config_data(&config_file_path);
    }
}

fn input_config_data( file: &PathBuf) {
    let mut conf = Ini::new();
    conf.clear();
    conf.with_section(None::<String>)
        .set("preferred_units","ms");
    conf.with_section(Some("Linux".to_owned()))
        .set("factorio_folder","")
        .set("a","");
    conf.with_section(Some("Windows".to_owned()))
        .set("factorio_folder","%APPDATA%\\");
    conf.write_to_file(&file).expect("");
}

pub fn read_config_file(key: &str) -> Option::<String> {
    let ini = match Ini::load_from_file(get_config_file()) {
        Ok(i) => i,
        Err(e) => panic!(e.to_string()),
    };
    let val = ini.get_from(None::<String>, key).map(String::from);
    return val;
}

pub fn read_config_file_platform_specific_value(key: &str) -> Option::<String> {
    let ini = match Ini::load_from_file(get_config_file()) {
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

fn get_config_file() -> PathBuf {
    return common::get_data_path().join("config.ini");
}
