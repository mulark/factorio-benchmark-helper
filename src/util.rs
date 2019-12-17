extern crate directories;
extern crate ini;
extern crate raw_cpuid;
extern crate regex;
extern crate reqwest;
extern crate sha1;
extern crate sha2;

mod database;
use crate::procedure_file::is_known_map_hash;
use crate::procedure_file::write_known_map_hash;
use core::fmt::Debug;
use core::str::FromStr;
pub use database::{setup_database, upload_to_db};
use std::collections::HashMap;
use std::io::stdin;
use std::ops::Range;
use std::process::exit;
mod fbh_paths;
pub use fbh_paths::{
    fbh_cache_path, fbh_config_file, fbh_data_path, fbh_known_hash_file, fbh_mod_dl_dir,
    fbh_mod_use_dir, fbh_procedure_json_local_file, fbh_procedure_json_master_file,
    fbh_read_configuration_setting, fbh_results_database, fbh_save_dl_dir,
    initialize,
};
use sha2::Digest;
use std::fs::File;
use std::io::Read;

pub mod performance_results;

pub use crate::procedure_file::{
    print_all_procedures, read_benchmark_set_from_file, read_meta_from_file,
    write_benchmark_set_to_file, write_meta_to_file, BenchmarkSet, ProcedureFileKind,
    ProcedureKind,
};
use directories::BaseDirs;
use ini::Ini;
use regex::Regex;
use std::path::PathBuf;
mod args;
pub use args::{add_options_and_parse, UserArgs};
mod mod_dl;
pub use mod_dl::{fetch_mod_deps_parallel, get_mod_info, Mod};
mod map_dl;
pub use map_dl::{fetch_map_deps_parallel, get_download_links_from_google_drive_by_filelist, Map};

lazy_static! {
    #[derive(Debug)]
    pub static ref FACTORIO_INFO: FactorioInfo = get_factorio_info();
    static ref FACTORIO_EXECUTABLE_VERSION_LINE: Regex = Regex::new(r"Version: \d{1,2}\.\d{2,3}\.\d{2,3}.*\n").unwrap();
    //If Factorio ever goes to 3/4/4 digits for their versioning, this will break.
}

pub fn download_benchmark_deps_parallel(sets: &HashMap<String, BenchmarkSet>) {
    let mut handles = Vec::new();
    let mut mods = Vec::new();
    let mut maps = Vec::new();
    for set in sets.values() {
        for indiv_mod in set.mods.clone() {
            mods.push(indiv_mod);
        }
        for indiv_map in set.maps.clone() {
            maps.push(indiv_map);
        }
    }
    maps.sort();
    maps.dedup();
    mods.sort();
    mods.dedup();

    fetch_mod_deps_parallel(&mods, &mut handles);
    fetch_map_deps_parallel(&maps, &mut handles);

    for handle in handles {
        handle.join().expect("");
    }
}

pub fn generic_read_configuration_setting(file: PathBuf, key: &str) -> Option<String> {
    match Ini::load_from_file(file) {
        Ok(i) => {
            return i.get_from::<String>(None, key).map(String::from);
        }
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
        if cfg!(target_os = "linux") {
            base_dir
                .home_dir()
                .join(".local")
                .join("share")
                .join("Steam")
                .join("steamapps")
                .join("common")
                .join("Factorio")
                .join("")
        } else {
            PathBuf::from("C:\\")
                .join("Program Files (x86)")
                .join("Steam")
                .join("steamapps")
                .join("common")
                .join("Factorio")
                .join("")
        }
    } else {
        match fbh_read_configuration_setting("factorio-path")
            .unwrap_or_default()
            .parse::<PathBuf>()
        {
            Ok(path) => {
                if path.is_dir() {
                    path
                } else {
                    eprintln!("Could not resolve path from config file to a valid directory of a Factorio install");
                    exit(1);
                }
            }
            Err(_e) => {
                eprintln!("Could not resolve path from config file to a valid directory of a Factorio install");
                exit(1);
            }
        }
    }
}

pub fn get_executable_path() -> PathBuf {
    if cfg!(target_os = "linux") {
        get_factorio_path().join("bin").join("x64").join("factorio")
    } else {
        get_factorio_path()
            .join("bin")
            .join("x64")
            .join("factorio.exe")
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
            BaseDirs::new()
                .unwrap()
                .data_dir()
                .join("Factorio")
                .join("")
        } else {
            // ~/.factorio/
            BaseDirs::new()
                .unwrap()
                .home_dir()
                .join(".factorio")
                .join("")
        }
    } else {
        get_factorio_path()
    }
}

pub fn get_saves_directory() -> PathBuf {
    get_factorio_rw_directory().join("saves").join("")
}

#[derive(Default, Clone)]
pub struct FactorioInfo {
    pub version: String,
    pub operating_system: String,
    pub platform: String,
}

fn get_factorio_info() -> FactorioInfo {
    //Don't call this, use FACTORIO_VERSION instead
    let line = FACTORIO_EXECUTABLE_VERSION_LINE
        .captures(&String::from_utf8_lossy(
            &std::process::Command::new(get_executable_path())
                .arg("--version")
                .output()
                .expect("")
                .stdout,
        ))
        .unwrap()[0]
        .to_string();
    let split = line.split_whitespace();

    let mut info_holder = FactorioInfo::default();
    for (i, s) in split.enumerate() {
        match i {
            1 => (info_holder.version = s.to_string()),
            4 => {
                ({
                    info_holder.operating_system = s.to_string();
                    info_holder.operating_system.pop();
                })
            }
            5 => {
                ({
                    info_holder.platform = s.to_string();
                    info_holder.platform.pop();
                })
            }
            _ => (),
        }
    }
    info_holder
}

pub fn sha256sum(file_path: &PathBuf) -> String {
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
            let sha256 = sha256sum(&path);
            (path, sha256)
        }));
    }
    for handle in handle_holder {
        let res_tuple = handle.join().unwrap();
        path_sha256_tuple_holder.push(res_tuple);
    }
    path_sha256_tuple_holder
}

pub fn query_system_cpuid() -> String {
    let cpuid = raw_cpuid::CpuId::new();
    cpuid
        .get_extended_function_info()
        .as_ref()
        .map_or_else(
            || "n/a",
            |extfuninfo| extfuninfo.processor_brand_string().unwrap_or("unreadable"),
        )
        .trim()
        .to_string()
}

pub fn trim_newline(s: &mut String) {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
}

pub fn prompt_until_empty_str(allow_first_empty: bool) -> String {
    let mut input = String::new();
    let mut last_input = String::new();
    let mut is_first = true;
    println!("Enter value, provide empty response to confirm.");
    loop {
        input.clear();
        stdin().read_line(&mut input).expect("");
        input = input.trim().to_owned();
        trim_newline(&mut input);
        if input.is_empty() && allow_first_empty && is_first {
            return input;
        }
        if input.is_empty() && !last_input.is_empty() {
            return last_input;
        }
        last_input = input.clone();
        is_first = false;
    }
}

pub fn prompt_until_allowed_val<T: FromStr + PartialEq + Debug>(allowed_vals: &[T]) -> T {
    let mut input = String::new();
    loop {
        input.clear();
        stdin().read_line(&mut input).expect("");
        input = input.trim().to_owned();
        trim_newline(&mut input);
        input.to_lowercase();
        if let Ok(m) = input.parse::<T>() {
            if allowed_vals.contains(&m) {
                return m;
            }
        }
        eprintln!(
            "Unrecognized option {:?}.\tAllowed values are: {:?}. Note: case insensitive.",
            input, allowed_vals
        );
    }
}

pub fn prompt_until_allowed_val_in_range<T: FromStr + PartialEq + PartialOrd + Debug>(
    range: Range<T>,
) -> T {
    let mut input = String::new();
    loop {
        input.clear();
        stdin().read_line(&mut input).expect("");
        input = input.trim().to_owned();
        trim_newline(&mut input);
        input.to_lowercase();
        if let Ok(m) = input.parse::<T>() {
            if range.contains(&m) {
                return m;
            }
        }
        eprintln!(
            "Unrecognized option {:?}.\t Must be in range {:?}.",
            input, range
        );
    }
}

fn path_of_7z_install() -> Option<PathBuf> {
    let exe_name = if cfg!(target_os = "linux") {
        "7z"
    } else {
        "7z.exe"
    };

    let mut found_path: Option<PathBuf> = None;

    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            let full_path = path.join(&exe_name);
            if full_path.is_file() {
                found_path = Some(full_path);
                break;
            }
        }
    }
    if found_path.is_none() && !cfg!(target_os = "linux") {
        let possible_other = PathBuf::from("C:\\")
            .join("Program Files")
            .join("7-Zip")
            .join("7z.exe");
        if possible_other.exists() {
            found_path = Some(possible_other);
        }
    }
    found_path
}

pub fn recompress_save(save: &PathBuf) {
    if save.exists() {
        if let Some(ext) = save.extension() {
            if ext == "zip" {
                if let Some(exe_7z) = path_of_7z_install() {
                    println!("Recompressing save {:?}", &save);

                    // Delete the preview image, saving 100-800KB from the few samples I've seen
                    if let Ok(mut process) = std::process::Command::new(&exe_7z)
                        .arg("d")
                        .arg(&save)
                        .arg("preview.jpg")
                        .arg("preview.png")
                        .arg("-r")
                        .stdout(std::process::Stdio::null())
                        .spawn()
                    {
                        if let Ok(exit_code) = process.wait() {
                            if !exit_code.success() {
                                unreachable!("7z is installed, save exists, but removing preview image from save failed!");
                            }
                        }
                    }

                    let decompress_dir = fbh_cache_path().join("resave");
                    std::fs::remove_dir_all(&decompress_dir).ok();
                    let mut process = std::process::Command::new(&exe_7z)
                        .arg("x")
                        .arg(format!("-o{}", &decompress_dir.to_string_lossy()))
                        .arg(&save)
                        .stdout(std::process::Stdio::null())
                        .spawn()
                        .expect("");
                    process.wait().unwrap();

                    std::fs::remove_file(&save).ok();
                    let mut process = std::process::Command::new(&exe_7z)
                        .arg("a")
                        .arg(&save)
                        .arg(format!(
                            "{}/{}",
                            decompress_dir.to_string_lossy(),
                            save.file_stem().unwrap().to_str().unwrap()
                        ))
                        .stdout(std::process::Stdio::null())
                        .spawn()
                        .expect("");
                    process.wait().unwrap();
                    std::fs::remove_dir_all(&decompress_dir.join(save.file_stem().unwrap())).ok();
                }
            }
        }
    }
}

pub fn recompress_saves_parallel(saves: Vec<PathBuf>, resave: bool) -> Vec<(PathBuf, String)> {
    let mut hash_holder = Vec::new();
    let mut handles: Vec<_> = Vec::new();

    for save in saves {
        handles.push(std::thread::spawn(move || {
            let pre_sha256 = sha256sum(&save);
            if path_of_7z_install().is_some() && !is_known_map_hash(&pre_sha256) && resave  {
                recompress_save(&save);
                let post_sha256 = sha256sum(&save);
                write_known_map_hash(&post_sha256);
                (save, post_sha256)
            } else {
                (save, pre_sha256)
            }
        }));
    }
    for handle in handles {
        hash_holder.push(handle.join().unwrap());
    }

    hash_holder
}
