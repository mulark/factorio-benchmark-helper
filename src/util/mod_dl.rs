use crate::util::sha1sum;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::process::exit;
use std::thread::JoinHandle;

use crate::util::{fbh_mod_dl_dir, get_factorio_rw_directory};

const MOD_PORTAL_URL: &str = "https://mods.factorio.com";
const MOD_PORTAL_API_URL: &str = "https://mods.factorio.com/api/mods/";

#[derive(Debug, Serialize, Deserialize, Clone, PartialOrd, Ord, Eq)]
pub struct Mod {
    pub name: String,
    #[serde(default)]
    pub file_name: String,
    pub version: String,
    pub sha1: String,
}

impl Mod {
    pub fn new(name: &str, file_name: &str, version: &str, hash: &str) -> Mod {
        Mod {
            name: name.to_string(),
            file_name: file_name.to_string(),
            version: version.to_string(),
            sha1: hash.to_string(),
        }
    }
}

impl PartialEq for Mod {
    fn eq(&self, cmp: &Self) -> bool {
        if self.sha1 == cmp.sha1 && !self.sha1.is_empty() {
            return true;
        }
        false
    }
}

#[derive(Debug, Deserialize, Clone)]
struct ModMetaInfoHolder {
    releases: Vec<ModPortalReleaseHolder>,
}

#[derive(Debug, Deserialize, Clone)]
struct ModPortalReleaseHolder {
    #[serde(skip_serializing)]
    download_url: String,
    file_name: String,
    version: String,
    sha1: String,
}

#[derive(Debug, Deserialize)]
struct User {
    #[serde(rename(deserialize = "service-username"))]
    username: String,
    #[serde(rename(deserialize = "service-token"))]
    token: String,
}

impl User {
    fn default() -> User {
        User {
            username: "".to_string(),
            token: "".to_string(),
        }
    }
}

//TODO make work with same mod but 2 diff versions
pub fn fetch_mod_deps_parallel(mods: &[Mod], mut handles: &mut Vec<JoinHandle<()>>) {
    let mut user_data: User = User::default();
    let maybe_playerdata_json_file = get_factorio_rw_directory().join("player-data.json");
    if maybe_playerdata_json_file.is_file() {
        if let Ok(file) = File::open(maybe_playerdata_json_file) {
            user_data = serde_json::from_reader(file).unwrap();
        }
    }
    let mut unique_mods: Vec<Mod> = mods.to_owned();
    //Only attempt to download unique mods from the sets. Skip base mod as it's special for vanilla.
    unique_mods.sort();
    unique_mods.dedup();
    
    let mut filename;
    for m in unique_mods {
        filename = if m.file_name.is_empty() {
            format!(
                "{}_{}.zip",
                m.name,
                if m.version.is_empty() {
                    r"{latest}"
                } else {
                    &m.version
                }
            )
        } else {
            m.file_name.clone()
        };
        let maybe_already_dl_mod = fbh_mod_dl_dir().join(&filename);
        let computed_sha1 = if maybe_already_dl_mod.is_file() {
            sha1sum(&maybe_already_dl_mod)
        } else {
            "".to_string()
        };
        if computed_sha1 != m.sha1 || computed_sha1 == "" {
            if !user_data.token.is_empty() && !user_data.username.is_empty() {
                // if the mod isn't found or its hash doesn't match the one we have on file, download it.
                fetch_single_mod(&user_data, filename, m, &mut handles);
            } else {
                eprintln!("Couldn't read playerdata.json for service-username or service-token, downloading mods from the mod portal is not possible.");
                eprintln!(
                    "If using the steam version try launching the game, and exiting normally once."
                );
                eprintln!("Presently running a benchmark on the Steam version causes the playerdata.json file to get overwritten until you run non-headlessly again.");
                exit(1);
            }
        } else {
            println!("Mod already up to date: {}", m.name);
        }
    }
}

fn fetch_single_mod(
    user_data: &User,
    filename: String,
    mut m: Mod,
    handles: &mut Vec<JoinHandle<()>>,
) {
    let token = user_data.token.clone();
    let username = user_data.username.clone();
    handles.push(std::thread::spawn(move || {
        println!("Downloading Mod: {}", filename);
        let mod_url = format!("{}{}", MOD_PORTAL_API_URL, m.name);
        let resp_raw = reqwest::get(&mod_url);
        let mut meta_info_response = ModMetaInfoHolder {
            releases: Vec::new(),
        };
        if let Ok(mut resp) = resp_raw {
            meta_info_response = resp.json().unwrap();
        }
        if m.version.is_empty() {
            for release in &meta_info_response.releases {
                m.version = compare_version_str(&release.version, &m.version);
            }
        }
        for release in meta_info_response.releases {
            if release.version == m.version {
                assert!(!username.is_empty());
                assert!(!token.is_empty());
                let dl_req = format!(
                    "{}{}?username={}&token={}",
                    MOD_PORTAL_URL, release.download_url, username, token
                );

                let mut resp = match reqwest::get(&dl_req) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Failed to download mod: {}", release.file_name);
                        panic!(e);
                    }
                };
                if resp.status().as_u16() == 200 {
                    let fpath = fbh_mod_dl_dir().join(&release.file_name);
                    if fpath.exists() {
                        match std::fs::remove_file(&fpath) {
                            Ok(_) => (),
                            Err(e) => {
                                eprintln!(
                                    "Mod exists in local directory but we couldn't remove it!"
                                );
                                eprintln!("Reason: {}", e);
                                exit(1);
                            }
                        }
                    }
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(fpath)
                        .unwrap();
                    match resp.copy_to(&mut file) {
                        Ok(_) => (),
                        Err(e) => {
                            println!("Failed to write file to {:?}!", file);
                            panic!(e);
                        }
                    }
                } else {
                    panic!(
                        "Error: We recieved a bad response from the mod portal. Status code: {}",
                        resp.status().as_u16()
                    );
                }
                let newly_dl_mod_sha1 = sha1sum(&fbh_mod_dl_dir().join(&release.file_name));
                if m.sha1 == "" {
                    m.sha1 = newly_dl_mod_sha1.clone();
                }
                if newly_dl_mod_sha1 != m.sha1 {
                    eprintln!("Recently downloaded mod {} hash mismatch!", m.name);
                    eprintln!("sha1 in config: {}", m.sha1);
                    eprintln!("sha1 of downloaded mod: {}", newly_dl_mod_sha1);
                }
                println!("Finished Downloading Mod: {}", &release.file_name);
                break;
            }
        }
    }));
}

pub fn compare_version_str(vers1: &str, vers2: &str) -> String {
    //Compare versions as vectors of u32 because 0.0.9 > 0.0.35 in String compare.
    let vers_cmp1 = convert_version_str_to_vec(&vers1);
    let vers_cmp2 = convert_version_str_to_vec(&vers2);
    if vers_cmp1 > vers_cmp2 {
        format!("{}.{}.{}", vers_cmp1[0], vers_cmp1[1], vers_cmp1[2])
    } else {
        format!("{}.{}.{}", vers_cmp2[0], vers_cmp2[1], vers_cmp2[2])
    }
}

fn convert_version_str_to_vec(version: &str) -> Vec<u32> {
    let mut vers = Vec::new();
    if !version.is_empty() {
        for u in version.split('.') {
            if let Ok(u) = u.parse::<u32>() {
                vers.push(u);
            } else {
                eprintln!(
                    "Error: Could not parse version string {} as a valid version!",
                    version
                );
                exit(1);
            }
        }
    }
    if vers.len() > 3 {
        eprintln!("Error: Mod versions can have at most 3 sections!",);
        exit(1);
    }
    if vers.is_empty() {
        vers = vec![0, 0, 0];
    }
    vers
}

fn get_latest_mod_version(meta_info: ModMetaInfoHolder) -> String {
    let mut latest = "0.0.0".to_string();
    for release in &meta_info.releases {
        latest = compare_version_str(&release.version, &latest);
    }
    latest
}

pub fn get_mod_info(mod_name: &str, mod_version: &str) -> Option<Mod> {
    let mut mod_v = mod_version.to_string();
    let mod_url = format!("{}{}", MOD_PORTAL_API_URL, mod_name);
    if let Ok(mut resp) = reqwest::get(&mod_url) {
        if resp.status() == 200 {
            println!("Found mod: {}", mod_name);
            if let Ok(meta_info_response) = resp.json::<ModMetaInfoHolder>() {
                if mod_version.is_empty() {
                    println!("Getting latest version...");
                    mod_v = get_latest_mod_version(meta_info_response.clone());
                }
                for release in meta_info_response.releases {
                    if release.version == mod_v {
                        println!("Succesfully found mod {}", release.file_name);
                        let mod_name = release.file_name.split('_').collect::<Vec<_>>()[0];
                        return Some(Mod {
                            name: mod_name.to_string(),
                            file_name: release.file_name,
                            sha1: release.sha1,
                            version: release.version,
                        });
                    }
                }
            }
        } else if resp.status() == 404 {
            println!("The mod {} was not found", mod_name);
            return None;
        } else {
            println!(
                "An unexpected response was recieved. Http code: {}",
                resp.status()
            );
            return None;
        }
    }
    None
}
