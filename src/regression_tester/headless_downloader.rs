//! Downloading Factorio installs based on available headless versions.
//! Not all previously released versions may be available in the future.
//! Inquires Factorio.com for the lastest versions.

use crate::util::fbh_regression_headless_storage;
use crate::util::FactorioVersion;
use std::convert::TryFrom;
use std::io::Read;
use std::time::Duration;
use ureq::Agent;

const FACTORIO_BASE_URL: &str = "https://factorio.com";
const FACTORIO_ARCHIVE_URL: &str = "https://factorio.com/download/archive";

fn get_downloadable_headless_versions(
    client: &Agent,
) -> Result<Vec<(FactorioVersion, String)>, Box<dyn std::error::Error>> {
    let resp = client.get(FACTORIO_ARCHIVE_URL).call();
    // TODO check for ok()
    let s = resp.into_string()?;
    let mut s2 = Vec::new();
    for line in s.lines() {
        if line.contains("get-download") && line.contains("headless") {
            let splits = line.split('"').collect::<Vec<_>>();
            let url_segment = splits[1];
            let version_str = url_segment.split('/').collect::<Vec<_>>()[2];
            let version = FactorioVersion::try_from(version_str).unwrap();
            s2.push((version, splits[1].to_owned()));
        }
    }

    Ok(s2)
}

/// Gets the locally downloaded versions of the headless version of Factorio for
/// regression testing.
fn get_local_headless_versions() -> Vec<FactorioVersion> {
    let mut versions = vec![];
    if let Ok(entries) = std::fs::read_dir(fbh_regression_headless_storage()) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    let filename = path.file_name().unwrap().to_string_lossy();
                    if filename.starts_with("factorio_headless_x64_")
                        && (filename.ends_with(".tar.gz")
                            || filename.ends_with(".tar.xz"))
                    {
                        // These must be the files, probably
                        // remove ext
                        let strip = filename.split(".tar").next().unwrap();
                        // remove prefix
                        let strip = strip.replace("factorio_headless_x64_", "");

                        let version = FactorioVersion::try_from(strip.as_ref());
                        if let Ok(version) = version {
                            versions.push(version);
                        }
                    }
                }
            }
        }
    }

    versions
}

/// Download a single Factorio version from the Factorio website.
/// Does not require any authentication for headless version of Factorio.
fn download_single_version(client: &Agent, url_segment: &str) {
    for i in 1..=3 {
        let combined_url = format!("{}{}", FACTORIO_BASE_URL, url_segment);
        eprintln!("Attempting download of {}, attempt {}", combined_url, i);
        let resp = client.get(&combined_url).call();
        let url = resp.get_url();
        if resp.ok() {
            let parsed_filename = url.split('?').next().unwrap();
            let parsed_filename =
                parsed_filename.split('/').last().unwrap().to_owned();
            assert!(
                parsed_filename.starts_with("factorio_headless_x64_"),
                "Parsing filename had unexpected format!? {}\nParsed {}",
                url,
                parsed_filename
            );
            let mut reader = resp.into_reader();
            let mut bytes = vec![];
            if reader.read_to_end(&mut bytes).is_ok()
                && std::fs::write(
                    &fbh_regression_headless_storage().join(parsed_filename),
                    bytes,
                )
                .is_ok()
            {
                break;
            }
        } else if resp.status() == 503 {
            eprintln!("Recieved 503 error, sleeping 1 second");
            std::thread::sleep(Duration::from_secs(1));
        } else {
            eprintln!("Some other unknown error error happened, giving up on this version");
            break;
        }
    }
}

/// Download the Factorio versions that are available remotely but not present
/// locally.
fn download_nonlocal_versions(client: &Agent) {
    let remote_versions_available = get_downloadable_headless_versions(&client);
    let local_versions = get_local_headless_versions();
    if let Ok(remote_versions) = remote_versions_available {
        let needed_remote_versions = remote_versions
            .iter()
            .filter(|(vers, _urls)| !local_versions.contains(vers));
        for (_ver, url_segment) in needed_remote_versions {
            download_single_version(client, url_segment);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ureq::Agent;
    #[test]
    fn test_get_headless_versions() {
        let client = Agent::new();
        let versions_tuple =
            get_downloadable_headless_versions(&client).unwrap();
        let versions = versions_tuple
            .into_iter()
            .map(|(vers, _urls)| vers)
            .collect::<Vec<_>>();
        assert!(versions.contains(&FactorioVersion::new(0, 17, 79)));
    }

    #[test]
    fn test_read_avail_headless_versions() {
        let local_vers = get_local_headless_versions();
        assert!(!local_vers.contains(&FactorioVersion::new(0, 0, 0)));
    }

    #[test]
    fn test_download_nonlocal_versions() {
        let client = Agent::new();
        download_nonlocal_versions(&client);
    }
}
