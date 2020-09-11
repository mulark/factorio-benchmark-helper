use megabase_index_incrementer::FactorioVersion;
use crate::util::{factorio_save_directory, fbh_save_dl_dir, sha256sum};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::thread::JoinHandle;
use std::convert::TryInto;

lazy_static! {
    static ref WHITELISTED_DOMAINS: Vec<String> = vec!(
        String::from("drive.google.com"),
        String::from("forums.factorio.com"),
        String::from(".backblazeb2.com")
    );
}

#[derive(Debug, Serialize, Deserialize, Clone, Ord, Eq, PartialOrd)]
pub struct Map {
    pub name: String,
    #[serde(skip)]
    pub path: PathBuf,
    #[serde(default)]
    pub min_compatible_version: FactorioVersion,
    pub sha256: String,
    pub download_link: String,
}

impl Map {
    pub fn new(path: &PathBuf, sha256: &str, download_link: &str) -> Map {
        Map {
            name: path.file_name().unwrap().to_string_lossy().to_string(),
            path: path.to_path_buf(),
            sha256: sha256.to_string(),
            min_compatible_version: "0.0.0".try_into().unwrap(),
            download_link: download_link.to_string(),
        }
    }
}

impl PartialEq for Map {
    fn eq(&self, cmp: &Self) -> bool {
        if self.sha256 == cmp.sha256 {
            return true;
        }
        false
    }
}

pub fn fetch_map_deps_parallel(
    maps: &[Map],
    handles: &mut Vec<JoinHandle<()>>,
    save_subdirectory: Option<PathBuf>,
) {
    let mut unique_maps: Vec<_> = maps.to_vec();
    unique_maps.sort();
    unique_maps.dedup();
    for map in unique_maps {
        let save_subdirectory = save_subdirectory.clone();
        handles.push(std::thread::spawn(move ||
            {
                let mut sha256;
                let (filepath, alt_filepath) =
                (
                    fbh_save_dl_dir().join(&save_subdirectory.as_ref().unwrap_or(&PathBuf::new())).join(&map.name),
                    factorio_save_directory().join(&save_subdirectory.unwrap_or_default()).join(&map.name)
                );
                if let Some(extension) = filepath.extension() {
                    if extension == "zip" {
                        if !filepath.is_file() && alt_filepath.is_file() {
                            match std::fs::create_dir_all(filepath.parent().unwrap()) {
                                Ok(_) => (),
                                Err(e) => {
                                    eprintln!("Error: We found a map inside the \
                                        Factorio save directory, but failed to \
                                        create a folder to copy it."
                                    );
                                    eprintln!("Error details: {}", e);
                                    eprintln!("Source: {:?}, Dest: {:?}", alt_filepath, filepath);
                                    exit(1);
                                },
                            }
                            match std::fs::copy(&alt_filepath, &filepath) {
                                Ok(_) => (),
                                Err(e) => {
                                    eprintln!("Error: We found a map inside the \
                                        Factorio save directory, but failed to \
                                        copy it to the cache folder."
                                    );
                                    eprintln!("Error details: {}", e);
                                    eprintln!("Source: {:?}, Dest: {:?}", alt_filepath, filepath);
                                    exit(1);
                                },
                            }
                        }
                        if !filepath.is_file() {
                            println!("Could not find map in cache or Factorio save directory, doing download.");
                            download_save(&map.name, map.download_link, &filepath);
                        } else {
                            println!("Found an existing map, checking sha256sum... {:?}", &filepath);
                            sha256 = sha256sum(&filepath);
                            if sha256 == map.sha256 && map.sha256 != "" {
                                println!("Found correct sha256sum, skipping download.");
                            } else {
                                println!("Found mismatched or empty sha256sum, performing download.");
                                download_save(&map.name, map.download_link, &filepath);
                            }
                        }
                    } else {
                        eprintln!("Expected map \"{}\" to have a .zip extension!", &map.name);
                        exit(1);
                    }
                } else {
                    eprintln!("Expected map \"{}\" to have a .zip extension!", &map.name);
                    exit(1);
                }
                if filepath.is_file() {
                    sha256 = sha256sum(&filepath);
                    if sha256 != map.sha256 {
                        eprintln!("We downloaded map {} but it doesn't match the sha256sum we have on file?", &map.name);
                        eprintln!("sha256 in config: {}", map.sha256);
                        eprintln!("sha256 of downloaded map: {}", sha256);
                    }
                    println!("Finished downloading map {}", &map.name);
                }
            }
        ));
    }
}

fn download_save(save_name: &str, url: String, to_save_to_path: &PathBuf) {
    if url.is_empty() {
        eprintln!(
            "Could not download map {} because a download link was not defined!",
            save_name
        );
        exit(1);
    }
    let mut whitelisted_url = false;
    for domain in WHITELISTED_DOMAINS.iter() {
        if url.contains(domain) {
            whitelisted_url = true;
        }
    }
    if !whitelisted_url {
        println!(
            "Warning, downloads from this domain have not been verified to work.\n{}",
            url
        );
    }
    let resp = ureq::get(&url).call();
    if resp.status() == 200 {
        if to_save_to_path.exists() {
            match std::fs::remove_file(&to_save_to_path) {
                Ok(()) => (),
                Err(e) => {
                    eprintln!("A failure occured when trying to remove already existing map with mismatched hash");
                    eprintln!("{}", e);
                    exit(1);
                }
            }
        }
        if let Err(e) =
            std::fs::create_dir_all(&to_save_to_path.parent().unwrap())
        {
            eprintln!("Could not create nested subdirectories in the Factorio Benchmark Helper cache directory");
            eprintln!("{}", e);
            exit(1);
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(to_save_to_path)
            .unwrap();
        let mut buf = Vec::new();
        resp.into_reader().read_to_end(&mut buf).unwrap();
        file.write_all(&buf).unwrap();
    } else {
        eprintln!(
            "Error: We recieved a bad response during a map download. Status code: {}",
            resp.status()
        );
        exit(1);
    }
}
