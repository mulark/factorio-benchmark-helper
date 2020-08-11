//! Downloading Factorio installs based on available headless versions.
//! Not all previously released versions may be available in the future.
//! Inquires Factorio.com for the lastest versions.

use std::path::PathBuf;
use crate::util::fbh_unpacked_headless_storage;
use crate::util::fbh_regression_headless_storage;
use megabase_index_incrementer::FactorioVersion;
use std::convert::TryFrom;
use std::io;
use std::io::Read;
use std::time::Duration;
use ureq::Agent;
use std::convert::TryInto;

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
/// regression testing. Returns a tuple of the FactorioVersion and the path of
/// the tar file of the headless version
pub fn get_local_headless_versions() -> Result<Vec<(FactorioVersion, PathBuf)>, std::io::Error> {
    let mut versions = vec![];

    let rd_dir = std::fs::read_dir(&fbh_regression_headless_storage())?;
    for entry in rd_dir {
        let entry = entry?;
        let fname = entry.file_name();
        let fname = fname.to_string_lossy();
        let prefix_removed = fname.replace("factorio_headless_x64_", "");
        let splits = prefix_removed.split('.').collect::<Vec<_>>();
        let major = splits[0].parse().unwrap_or_default();
        let minor = splits[1].parse().unwrap_or_default();
        let patch = splits[2].parse().unwrap_or_default();
        let parsed_fv = FactorioVersion {
            major,
            minor,
            patch
        };
        versions.push((parsed_fv, entry.path()));
    }

    Ok(versions)
}

/// Download a single Factorio version from the Factorio website.
/// Does not require any authentication for headless version of Factorio.
fn download_single_version(client: &Agent, url_segment: &str) {
    for i in 1..=3 {
        let combined_url = format!("{}{}", FACTORIO_BASE_URL, url_segment);
        eprintln!("Attempting download of {}, attempt {}", combined_url, i);
        let resp = client.get(&combined_url).timeout(Duration::from_secs(60)).call();
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

/// Unpacks a given FactorioVersion if it's present.
/// Returns Ok(true) if the version was present and unpacked successfully.
/// Returns Ok(false) if the version was not present.
/// Returns Err(io::Error) if some io error occurred.
pub fn unpack_headless_version(unpack_version: FactorioVersion) -> Result<bool, io::Error> {
    let mut version_found = false;
    let vers = get_local_headless_versions()?;
    let already_unpacked_vers = get_unpacked_executables()?.iter().map(|(fv, _path)| *fv).collect::<Vec<_>>();
    if already_unpacked_vers.contains(&unpack_version) {
        return Ok(true);
    }
    for (local_version, path) in vers {
        if local_version == unpack_version {
            let bytes = std::fs::read(&path)?;
            if let Ok(decompressed_bytes) = lzma::decompress(&bytes) {
                let mut ar = tar::Archive::new(&*decompressed_bytes);
                ar.unpack(fbh_unpacked_headless_storage().join(local_version.to_string()))?;
                version_found = true;
                break;
            }
        }
    }

    Ok(version_found)
}

/// Gets a listing of the currently unpacked FactorioVersions with executables
pub fn get_unpacked_executables() -> Result<Vec<(FactorioVersion, PathBuf)>, io::Error> {
    let mut found_version_path_tuple = Vec::new();
    for entry in std::fs::read_dir(fbh_unpacked_headless_storage())? {
        let entry = entry?;
        let fname = entry.file_name();
        if let Ok(fv) = fname.to_str().unwrap().try_into() {
            // Regression tests not supported on Windows.
            let path = entry.path().join("factorio").join("bin").join("x64").join("factorio");
            if path.is_file() {
                found_version_path_tuple.push((fv, path));
            }
        }
    }
    Ok(found_version_path_tuple)
}

/// Download the Factorio versions that are available remotely but not present
/// locally.
pub fn download_nonlocal_versions(client: &Agent) {
    let remote_versions_available = get_downloadable_headless_versions(&client);
    let local_versions = get_local_headless_versions();
    if let Ok(remote_versions) = remote_versions_available {
        if let Ok(local_versions) = local_versions {
            let needed_remote_versions = remote_versions
                .iter()
                .filter(|(vers, _urls)| {
                    !local_versions
                        .iter()
                        .map(|x| x.0)
                        .any(|x| x == *vers)
                    });
            for (_ver, url_segment) in needed_remote_versions {
                download_single_version(client, url_segment);
            }
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
        let local_vers = get_local_headless_versions().unwrap();
        assert!(!local_vers.iter().map(|x| x.0)
            .any(|x| x == FactorioVersion::new(0, 0, 0)));
    }

    #[ignore]
    #[test]
    fn test_download_nonlocal_versions() {
        let client = Agent::new();
        download_nonlocal_versions(&client);
    }

    #[test]
    fn test_unpack_headless_fv() {
        test_download_nonlocal_versions();
        let fv = FactorioVersion::new(0, 17, 79);
        unpack_headless_version(fv).unwrap();
    }
}
