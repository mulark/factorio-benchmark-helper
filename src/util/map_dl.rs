
use sha2::{Digest};
use std::fs::{read,OpenOptions};
use std::thread::JoinHandle;
use crate::util::{
    fbh_save_dl_dir,
    Map,
    get_saves_directory,
};

pub fn fetch_map_deps_parallel(maps: Vec<Map>, handles: &mut Vec::<JoinHandle<()>>) {
    for map in maps {
        handles.push(std::thread::spawn(move ||
            {
                let mut sha256;
                let filepath = fbh_save_dl_dir().join(&map.name);
                let alt_filepath = get_saves_directory().join(&map.name);
                if let Some(extension) = filepath.extension() {
                    if extension == "zip" {
                        if !filepath.is_file() && alt_filepath.is_file() {
                            match std::fs::copy(&alt_filepath, &filepath) {
                                Ok(_) => (),
                                Err(e) => {
                                    eprintln!("Error: We found a map inside the Factorio save directory, but failed to copy it to the cache.");
                                    eprintln!("Error details: {}", e);
                                    std::process::exit(1);
                                },
                            }
                        }
                        if !filepath.is_file() {
                            println!("Could not find map in cache or Factorio save directory, doing download.");
                            download_save(&map.name, map.download_link);
                        } else {
                            println!("Found an existing map, checking sha256sum.", );
                            sha256 = format!("{:x}", sha2::Sha256::digest(
                                &read(&fbh_save_dl_dir().join(&filepath)).unwrap()
                            ));
                            if sha256 == map.sha256 && map.sha256 != "" {
                                println!("Found cached map with correct hash, skipping download.");
                            } else {
                                println!("Found mismatched or empty sha256sum, performing download.");
                                download_save(&map.name, map.download_link);
                            }
                        }
                    } else {
                        eprintln!("Expected map \"{}\" to have a .zip extension!", &map.name);
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("Expected map \"{}\" to have a .zip extension!", &map.name);
                    std::process::exit(1);
                }
                if filepath.is_file() {
                    sha256 = format!("{:x}", sha2::Sha256::digest(
                        &read(&fbh_save_dl_dir().join(&filepath)).unwrap()
                    ));
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

fn download_save(save_name: &str, url: String) {
    if url.is_empty() {
        eprintln!("Could not download map {} because a download link was not defined!", save_name);
        std::process::exit(1);
    }
    let mut resp = match reqwest::get(&url) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to download map: {}", save_name);
            eprintln!("Error details: {}", e);
            std::process::exit(1);
        },
    };
    if resp.status().as_u16() == 200 {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(fbh_save_dl_dir().join(save_name))
            .unwrap();
        match resp.copy_to(&mut file) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Failed to write file to {:?}!", file);
                eprintln!("Error details: {}", e);
                std::process::exit(1);
            },
        }
    } else {
        eprintln!("Error: We recieved a bad response. Status code: {}", resp.status().as_u16());
        std::process::exit(1);
    }
}
