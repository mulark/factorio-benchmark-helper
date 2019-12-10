use crate::util::{
    fbh_read_configuration_setting, fbh_save_dl_dir, get_saves_directory, sha256sum,
};
use reqwest;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::process::exit;
use std::thread::JoinHandle;

lazy_static! {
    static ref WHITELISTED_DOMAINS: Vec<String> = vec!(
        String::from("drive.google.com"),
        String::from("forums.factorio.com"),
    );
}

#[derive(Debug, Serialize, Deserialize, Clone, Ord, Eq, PartialOrd)]
pub struct Map {
    pub name: String,
    pub sha256: String,
    pub download_link: String,
}

impl Map {
    pub fn new(name: &str, sha256: &str, download_link: &str) -> Map {
        Map {
            name: name.to_string(),
            sha256: sha256.to_string(),
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

#[derive(Debug, Deserialize)]
pub struct DriveFolderListing {
    files: Vec<DriveFile>,
}

#[derive(Debug, Deserialize)]
pub struct DriveFile {
    name: String,
    #[serde(rename(deserialize = "webContentLink"))]
    download_link: String,
}

pub fn fetch_map_deps_parallel(maps: &[Map], handles: &mut Vec<JoinHandle<()>>) {
    let mut unique_maps: Vec<_> = maps.to_vec();
    unique_maps.sort();
    unique_maps.dedup();
    for map in unique_maps {
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
                                    eprintln!("Error: We found a map inside the Factorio save directory, but failed to copy it to the cache folder.");
                                    eprintln!("Error details: {}", e);
                                    exit(1);
                                },
                            }
                        }
                        if !filepath.is_file() {
                            println!("Could not find map in cache or Factorio save directory, doing download.");
                            download_save(&map.name, map.download_link);
                        } else {
                            println!("Found an existing map, checking sha256sum...", );
                            sha256 = sha256sum(&fbh_save_dl_dir().join(&filepath));
                            if sha256 == map.sha256 && map.sha256 != "" {
                                println!("Found correct sha256sum, skipping download.");
                            } else {
                                println!("Found mismatched or empty sha256sum, performing download.");
                                download_save(&map.name, map.download_link);
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
                    sha256 = sha256sum(&fbh_save_dl_dir().join(&filepath));
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

fn download_shared_folder_file_listing_and_parse(
    drive_folder_url: &str,
) -> Option<DriveFolderListing> {
    if !drive_folder_url.contains("drive.google.com") || drive_folder_url.is_empty() {
        eprintln!("You provided a link that isn't part of the drive.google.com domain");
        return None;
    }
    let client = reqwest::Client::new();
    let folder_id = drive_folder_url
        .replace("https://drive.google.com/drive", "")
        .replace("https://drive.google.com/open?id=", "")
        .replace("/view", "")
        .replace("?usp=sharing", "")
        .replace("/u/0", "")
        .replace("/folders/", "");
    println!("folder_id: {}", folder_id);
    if let Some(api_key) = fbh_read_configuration_setting("google-drive-api-key") {
        let req_url = format!(
            "{}{}{}{}{}{}{}{}{}",
            "https://www.googleapis.com/drive/v3/files?",
            "fields=files/name,files/webContentLink",
            "&q=%27",
            folder_id,
            "%27",
            "%20in%20parents",
            "%20and%20mimeType=%22application/zip%22", //Only .zip files
            "&key=",
            api_key,
        );
        if let Ok(mut resp) = client.get(&req_url).send() {
            if resp.status() == 200 {
                if let Ok(parsed_file_list) = resp.json::<DriveFolderListing>() {
                    return Some(parsed_file_list);
                }
            } else if resp.status() == 404 {
                eprintln!("Failed to fetch google drive folder due to 404 error (maybe folder doesn't exist?)");
            } else if resp.status() == 403 {
                eprintln!("Failed to fetch google drive folder due to 403 forbidden! (Check your api key and that the folder is shared)");
            } else {
                eprintln!(
                    "Failed to fetch google drive folder due to an unknown response: {}",
                    resp.status()
                );
            }
        }
    } else {
        eprintln!("Couldn't get a google drive api key from your config.ini file.");
        eprintln!("Follow instructions at LINK to add this api key.");
    }
    None
}

pub fn get_download_links_from_google_drive_by_filelist(
    filenames_to_find: Vec<String>,
    drive_folder_url: &str,
) -> Option<Vec<(String, String)>> {
    if let Some(file_listing) = download_shared_folder_file_listing_and_parse(drive_folder_url) {
        let mut links_to_dl = Vec::new();
        for drive_file in file_listing.files {
            for searched_name in &filenames_to_find {
                if &drive_file.name == searched_name {
                    links_to_dl.push((drive_file.name.clone(), drive_file.download_link.clone()));
                }
            }
        }
        if !links_to_dl.is_empty() {
            return Some(links_to_dl);
        }
    }
    None
}

fn download_save(save_name: &str, url: String) {
    if url.is_empty() {
        eprintln!(
            "Could not download map {} because a download link was not defined!",
            save_name
        );
        exit(1);
    }
    let mut whitelisted_url = false;
    for domain in WHITELISTED_DOMAINS.clone() {
        if url.contains(&domain) {
            whitelisted_url = true;
        }
    }
    if !whitelisted_url {
        println!(
            "Warning, downloads from this domain have not been verified to work.\n{}",
            url
        );
    }
    let mut resp = match reqwest::get(&url) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to download map: {}", save_name);
            eprintln!("Error details: {}", e);
            exit(1);
        }
    };
    if resp.status().as_u16() == 200 {
        let save_path = fbh_save_dl_dir().join(save_name);
        if save_path.exists() {
            match std::fs::remove_file(&save_path) {
                Ok(()) => (),
                Err(e) => {
                    eprintln!("A failure occured when trying to remove already existing map with mismatched hash");
                    eprintln!("{}", e);
                    exit(1);
                }
            }
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(save_path)
            .unwrap();
        match resp.copy_to(&mut file) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Failed to write file to {:?}!", file);
                eprintln!("Error details: {}", e);
                exit(1);
            }
        }
    } else {
        eprintln!(
            "Error: We recieved a bad response. Status code: {}",
            resp.status().as_u16()
        );
        exit(1);
    }
}
