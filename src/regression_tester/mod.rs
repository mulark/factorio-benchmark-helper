//! Module for running regression tests against Factorio versions.
mod headless_downloader;

mod megabase_downloader;

use megabase_index_incrementer::MegabaseMetadata;
use std::collections::HashSet;
use std::path::PathBuf;
use crate::util::sha256sum;
use crate::util::factorio_save_directory;
use ureq::Agent;
use std::io;
use std::io::Read;
use megabase_index_incrementer::Megabases;
use crate::regression_tester::headless_downloader::download_nonlocal_versions;

lazy_static! {
    /// The subfolder where any applicable megabases are to be stored.
    static ref REGRESSION_TEST_SUBFOLDER: PathBuf
        = factorio_save_directory().join("regression-test");
    static ref MEGABASES: Megabases = fetch_megabase_list().unwrap();

}

/// Runs regression tests against Factorio
pub fn run_regression_tests() {
    if let Ok(validated) = fetch_files() {
        let mut restricted = MEGABASES.saves.clone();
        restricted.retain(|save| validated.contains(&save.sha256));

    }
}

fn fetch_files() -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let client = Agent::new();
    let jh = {
        std::thread::spawn(move || {
            // Gather all available Factorio headless versions.
            download_nonlocal_versions(&client);
        })
    };
    let mut valid_shas = HashSet::new();
    let mut jhs = Vec::new();

    let entries = std::fs::read_dir(&*REGRESSION_TEST_SUBFOLDER)?;
    let mut files_on_disk = HashSet::new();
    for file in entries {
        let path = file?.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "zip" {
                    if let Some(fname) = path.file_name() {
                        files_on_disk.insert(fname.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    for save in &*MEGABASES.saves {
        if files_on_disk.contains(&save.name) {
            // check sha
            {
                let save = save.clone();
                jhs.push(std::thread::spawn(move || {
                    let sha = sha256sum(&*REGRESSION_TEST_SUBFOLDER.join(&save.name));
                    if sha != save.sha256 {
                        download_single_save(&save)
                    } else {
                        Ok(sha)
                    }
                }));
            }
        } else if save.download_link_mirror.is_some() {
            // download it, then check sha
            {
                let save = save.clone();
                jhs.push(std::thread::spawn(move || {
                    download_single_save(&save)
                }));
            }
        } else {
            eprintln!("Warning, unable to use {} because no mirrored \
                download link is defined.", save.name
            );
            eprintln!("Please download the map at {} and then move it to {:?} to test with this map.",
                save.source_link, *REGRESSION_TEST_SUBFOLDER);
            eprintln!("Continuing anyway...");
        }
    }

    for jh in jhs {
        if let Ok(sha) = jh.join().unwrap() {
            valid_shas.insert(sha);
        }
    }
    jh.join().unwrap();

    Ok(valid_shas)
}

fn download_single_save(save: &MegabaseMetadata) -> Result<String, io::Error> {
    if let Some(mirror) = &save.download_link_mirror {
        let resp = ureq::get(mirror).call();
        let sha = if resp.status() == 200 {
            let p = &*REGRESSION_TEST_SUBFOLDER.join(&save.name);
            let mut buf = Vec::new();
            let mut reader = resp.into_reader();
            let mut sha = String::new();
            if reader.read_to_end(&mut buf).is_ok()
            && std::fs::write(p, buf).is_ok() {
                sha = sha256sum(p);
            }
            sha
        } else {
            eprintln!("Could not download file {}, {:?}", save.name, resp);
            return Err(io::Error::new(io::ErrorKind::Other, "Could not download file"));
        };
        if sha != save.sha256 {
            return Err(io::Error::new(io::ErrorKind::Other, "Downloaded file didn't match sha256 previously recorded"));
        }
        return Ok(sha);
    }
    Err(io::Error::new(io::ErrorKind::Other, "No download link was defined"))
}

/// Downloads and parses the technicalfactorio megabase index.
fn fetch_megabase_list() -> Result<Megabases, Box<dyn std::error::Error>> {
    let resp = ureq::get("https://raw.githubusercontent.com/technicalfactorio/\
        technicalfactorio/master/megabase_index_incrementer/megabases.json")
        .call();
    if resp.status() == 200 {
        let s = resp.into_string()?;
        Ok(serde_json::from_str(&s)?)
    } else {
        eprintln!("Could not download listing of megabases");
        std::process::exit(1);
    }
}
