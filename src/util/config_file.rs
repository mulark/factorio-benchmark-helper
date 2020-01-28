use std::path::PathBuf;
use std::fs::OpenOptions;
use ini::Ini;
use std::io::Write;
use crate::util::common::CONFIG_FILE_VERSION;
use crate::util::fbh_paths::fbh_config_file;

lazy_static!{
    pub static ref CONFIG_FILE_SETTINGS: ForwardCompatibilityConfigSettings = load_forward_compatiblity_config_settings();
}

#[derive(Debug, Default)]
pub struct ForwardCompatibilityConfigSettings {
    cfg_file_version: u32,
    pub use_steam_version: bool,
    pub factorio_path: Option<PathBuf>,
    pub erase_preview_image: bool,
    pub b2_backblaze_key_id: String,
    pub b2_backblaze_application_key: String,
    pub travis_ci_b2_keyid: String,
    pub travis_ci_b2_applicationkey: String,
}

pub fn load_forward_compatiblity_config_settings() -> ForwardCompatibilityConfigSettings {
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
    let maybe_path = i.get_from_or::<&str>(None, "factorio-path", "");
    if maybe_path.is_empty() {
        settings.factorio_path = None;
    } else {
        settings.factorio_path = Some(PathBuf::from(maybe_path));
    }
    settings.erase_preview_image = i
        .get_from_or::<&str>(None, "erase-preview-image", "true")
        .parse()
        .unwrap_or(true);
    settings.b2_backblaze_key_id = i
        .get_from_or::<&str>(None, "b2-backblaze-keyID", "")
        .to_string();
    settings.b2_backblaze_application_key = i
        .get_from_or::<&str>(None, "b2-backblaze-applicationKey", "")
        .to_string();
    settings.travis_ci_b2_keyid = i
        .get_from_or::<&str>(None, "TRAVIS_CI_B2_KEYID", "")
        .to_string();
    settings.travis_ci_b2_applicationkey = i
        .get_from_or::<&str>(None, "TRAVIS_CI_B2_APPLICATIONKEY", "")
        .to_string();
    settings
}

pub fn fbh_write_config_file() -> Result<(), std::io::Error> {
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
                prev_or_default_settings.factorio_path.unwrap_or_default().to_string_lossy()
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
            if !prev_or_default_settings.travis_ci_b2_keyid.is_empty() {
                writeln!(file)?;
                writeln!(file, "; For test use only")?;
                writeln!(
                    file,
                    "TRAVIS_CI_B2_KEYID={}",
                    prev_or_default_settings.travis_ci_b2_keyid
                )?;
                writeln!(
                    file,
                    "TRAVIS_CI_B2_APPLICATIONKEY={}",
                    prev_or_default_settings.travis_ci_b2_applicationkey
                )?;
            }
        }
    }
    Ok(())
}
