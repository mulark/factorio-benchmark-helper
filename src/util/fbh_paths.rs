use crate::procedure_file::update_master_json;
use crate::util::{generic_read_configuration_setting, setup_database};
use directories::ProjectDirs;
use ini::Ini;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

const CONFIG_FILE_VERSION: u32 = 1;

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
    fbh_init_config_file()?;
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

#[derive(Debug, Default)]
struct ForwardCompatibilityConfigSettings {
    cfg_file_version: u32,
    use_steam_version: bool,
    factorio_path: String,
    erase_preview_image: String,
    b2_backblaze_key_id: String,
    b2_backblaze_application_key: String,
}

fn load_forward_compatiblity_config_settings() -> ForwardCompatibilityConfigSettings {
    let mut settings = ForwardCompatibilityConfigSettings::default();
    if !fbh_config_file().is_file() {
        return settings;
    }
    // Safe to unwrap because of early return if file doesn't exist
    let i = Ini::load_from_file(fbh_config_file()).unwrap();
    settings.cfg_file_version = i
        .get_from_or::<&str>(None, "config-file-version", "0")
        .parse::<u32>()
        .unwrap_or_default();
    settings.use_steam_version = i
        .get_from_or::<&str>(None, "use-steam-version", "true")
        .parse::<bool>()
        .unwrap_or(true);
    settings.factorio_path = i.get_from_or::<&str>(None, "factorio-path", "").to_string();
    settings.erase_preview_image = i
        .get_from_or::<&str>(None, "erase-preview-image", "true")
        .to_string();
    settings.b2_backblaze_key_id = i
        .get_from_or::<&str>(None, "b2-backblaze-keyID", "")
        .to_string();
    settings.b2_backblaze_application_key = i
        .get_from_or::<&str>(None, "b2-backblaze-applicationKey", "")
        .to_string();
    settings
}

fn fbh_init_config_file() -> Result<(), std::io::Error> {
    let prev_or_default_settings = load_forward_compatiblity_config_settings();
    if prev_or_default_settings.cfg_file_version != CONFIG_FILE_VERSION {
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(fbh_config_file())
        {
            writeln!(file, "; Comments begin with a semicolon ';'")?;
            writeln!(file)?;
            writeln!(file, "; Property used for updating this config file")?;
            writeln!(file, "config-file-version={}", CONFIG_FILE_VERSION)?;
            writeln!(file)?;
            writeln!(
                file,
                "use-steam-version={}",
                prev_or_default_settings.use_steam_version
            )?;
            writeln!(file)?;
            writeln!(
                file,
                "; The path to Factorio, if Steam version is not used or could not be found"
            )?;
            writeln!(file, "; Required if use-steam-version is false")?;
            writeln!(
                file,
                "factorio-path={}",
                prev_or_default_settings.factorio_path
            )?;
            writeln!(file)?;
            writeln!(
                file,
                "; Erase the preview image from saves during a --create-benchmark"
            )?;
            writeln!(file, "; Saves a couple of bytes in the resulting save")?;
            writeln!(
                file,
                "; Uses 7z so that must be installed for it to succeed"
            )?;
            writeln!(
                file,
                "erase-preview-image={}",
                prev_or_default_settings.erase_preview_image
            )?;
            writeln!(file)?;
            writeln!(
                file,
                "; Backblaze keyID to allow automatic upload of saves to b2 Backblaze"
            )?;
            writeln!(
                file,
                "b2-backblaze-keyID={}",
                prev_or_default_settings.b2_backblaze_key_id
            )?;
            writeln!(file)?;
            writeln!(
                file,
                "; Backblaze application key to allow automatic upload of saves to b2 Backblaze"
            )?;
            writeln!(
                file,
                "b2-backblaze-applicationKey={}",
                prev_or_default_settings.b2_backblaze_application_key
            )?;
        }
    }
    Ok(())
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

pub fn fbh_read_configuration_setting(key: &str) -> Option<String> {
    generic_read_configuration_setting(fbh_config_file(), key)
}
