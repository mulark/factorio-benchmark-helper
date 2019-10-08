extern crate directories;
extern crate ini;

use ini::Ini;
use std::path::PathBuf;
use directories::{ProjectDirs, BaseDirs};

/* Factorio Benchmark Helper Paths */
pub fn get_fbh_data_path() -> PathBuf {
    //Data paths for this program, not Factorio
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

fn get_config_file() -> PathBuf {
    return get_fbh_data_path().join("config.ini");
}

pub fn setup_config_file(regen_config_file: bool) {
    if !get_config_file().is_file() || regen_config_file == true
    {
        input_config_data(get_config_file());
    }
}

fn input_config_data(file: PathBuf) {
    let mut conf = Ini::new();
    conf.clear();
    conf.with_section(None::<String>)
        .set("use-steam-version","true");
    conf.with_section(None::<String>)
        .set("factorio-path","");
    conf.with_section(None::<String>)
        .set("preferred_units","ms");
    conf.write_to_file(&file).expect("");
}

pub fn read_config_file(file: PathBuf,key: &str) -> Option::<String> {
    match Ini::load_from_file(file) {
        Ok(i) => return i.get_from(None::<String>, key).map(String::from),
        Err(_e) => return None,
    };
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

fn get_factorio_path() -> PathBuf {
    let use_steam_dir: bool = read_config_file(get_config_file(), "use-steam-version").unwrap_or_default().parse::<bool>().unwrap_or(true);
    if use_steam_dir {
        let base_dir = BaseDirs::new().unwrap();
        let probable_steam_path;
        if cfg!(Windows) {
            probable_steam_path = PathBuf::from("C:")
                .join("Program Files (x86)")
                .join("Steam")
                .join("steamapps")
                .join("common")
                .join("Factorio")
                .join("");
        } else {
            probable_steam_path = base_dir.home_dir()
                .join(".local")
                .join("share")
                .join("Steam")
                .join("steamapps")
                .join("common")
                .join("Factorio")
                .join("");
        }
        return probable_steam_path;
    } else {
        return PathBuf::new();
    }

}

pub fn get_executable_path() -> PathBuf {
    return get_factorio_path().join("bin")
        .join("x64")
        .join("factorio");
}

fn get_factorio_rw_directory() -> PathBuf {
    let ini_path = get_factorio_path().join("config-path.cfg");
    let use_system_rw_directories: bool = read_config_file(ini_path, "use-system-read-write-directories").unwrap_or(String::new()).parse::<bool>().unwrap_or(false);

    if use_system_rw_directories {
        if cfg!(Windows) {
            //%appdata%\Roaming\
            return BaseDirs::new().unwrap().data_dir().join("Factorio").join("");
        } else {
            //~/.factorio/
            return BaseDirs::new().unwrap().home_dir().join(".factorio").join("");
        }
    } else {
        return get_factorio_path();
    }
}

pub fn get_saves_directory() -> PathBuf {
    return get_factorio_rw_directory().join("saves").join("");
}
