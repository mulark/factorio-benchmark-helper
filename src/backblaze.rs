// Keep returned values from API in same camelCase format, or snake_case as applicable
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
use std::time::Duration;
use reqwest::Client;
use crate::util::sha1sum;
use std::path::PathBuf;
use crate::util::fbh_read_configuration_setting;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

const B2_AUTHORIZE_ACCOUNT_URL: &str = "https://api.backblazeb2.com/b2api/v2/b2_authorize_account";

// Holds the things necessary for uploading this file
#[derive(Clone, Default, Debug)]
struct FileUploadInstance {
    filepath: PathBuf,
    sha256: String,
    sha1: String,
    relative_filename: String, // The relative filename with included position within subfolders, if any
    final_url: String,
    already_uploaded: bool,
    upload_response: Option<BackblazeUploadFileResponse>,
}

#[derive(Clone, Debug, Default)]
struct BackbazeDataContainer {
    auth: Option<BackblazeAuth>,
    get_url_response: Option<BackblazeGetUploadUrlResponse>,
    already_uploaded_files: Option<BackblazeListFileNamesResponse>,
    files: Vec<FileUploadInstance>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct BackblazeAuth {
    accountId: String,
    authorizationToken: String,
    downloadUrl: String,
    apiUrl: String,
    allowed: BackblazeAuthAllowed,
    #[serde(skip)]
    dirty: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct BackblazeAuthAllowed {
    bucketId: String,
    bucketName: String,
    capabilities: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct BackblazeGetUploadUrlResponse {
    bucketId: String,
    uploadUrl: String,
    authorizationToken: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct BackblazeListFileNamesPostBody {
    bucketId: String,
    maxFileCount: u16,
    prefix: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct BackblazeListFileNamesResponse {
    files: Vec<BackblazeUploadFileResponse>,
    //nextFileName: String, // If more than 1000 files are in this bucket folder we'd have to do this
}

/// The data returned by a successful upload to backblaze
#[derive(Serialize, Deserialize, Clone, Debug)]
struct BackblazeUploadFileResponse {
    accountId: String,
    action: String,
    bucketId: String,
    contentLength: u64, // Length in bytes
    contentSha1: String,
    fileId: String,
    fileName: String,
}

#[derive(Deserialize, Debug, Clone)]
struct BackblazeErrorResponse {
    status: u16,
    code: BackblazeErrorKind,
    message: String,
}

#[derive(Deserialize, Debug, Clone)]
enum BackblazeErrorKind {
    SendError,              // 0 - Failed for some reason attempting to send data
    KeysNotPresent,         // 0 - Failed to read needed keys from config file
    bad_request,            // 400 or 503
    invalid_bucket_id,      // 400
    out_of_range,           // 400
    unauthorized,           // 401
    unsupported,            // 401
    bad_auth_token,         // 401
    expired_auth_token,     // 401
    cap_exceeded,           // 403
    method_not_allowed,     // 405
    request_timeout,        // 408
    service_unavailable,    // 503
}


//TODO carry all paths belonging to (subdir)
#[derive(Debug)]
enum BackblazeUploadState {
    GetAuth, // once
    ListFileNames, //Once? maybe or partitioned // poolable
    TestIfAlreadyUploaded, // O(N)
    GetUploadUrl, // O(1) to O(N)
    Upload, // O(N)
}

/// Authorize with the Backblaze API using test credentials.
/// These credentials can be found in the envs TRAVIS_CI_B2_KEYID & TRAVIS_CI_B2_APPLICATIONKEY
/// or within the config.ini file under the same name.
/// These fields are not written to the config.ini file normally.
fn authorize_test(client: &Client) -> Result<BackblazeAuth,BackblazeErrorResponse> {
    let vars = std::env::vars().collect::<HashMap<String,String>>();
    let key_id = if vars.get("TRAVIS_CI_B2_KEYID").is_some() {
        vars.get("TRAVIS_CI_B2_KEYID").unwrap().to_string()
    } else {
        fbh_read_configuration_setting("TRAVIS_CI_B2_KEYID").unwrap()
    };
    let application_key = if vars.get("TRAVIS_CI_B2_APPLICATIONKEY").is_some() {
        vars.get("TRAVIS_CI_B2_APPLICATIONKEY").unwrap().to_string()
    } else {
        fbh_read_configuration_setting("TRAVIS_CI_B2_APPLICATIONKEY").unwrap()
    };
    authorize(&client, &key_id, &application_key)
}

/// Authorize with Backblaze API using keys found within config.ini
fn authorize_cfg(client: &Client) -> Result<BackblazeAuth, BackblazeErrorResponse> {
    if cfg!(test) {
        return authorize_test(&client)
    } else if let Some(key_id) = fbh_read_configuration_setting("b2-backblaze-keyID") {
        if let Some(application_key) = fbh_read_configuration_setting("b2-backblaze-applicationKey") {
            if !key_id.is_empty() && !application_key.is_empty() {
                return authorize(client, &key_id, &application_key)
            }
        }
    }
    Err(BackblazeErrorResponse {
        status: 0,
        code: BackblazeErrorKind::KeysNotPresent,
        message: "Could not get keys from config.ini".to_string()
    })
}

/// Authorize with Backblaze using an application key and key ID.
fn authorize(client: &Client, key_id: &str, application_key: &str) -> Result<BackblazeAuth,BackblazeErrorResponse> {
    assert!(!key_id.is_empty());
    assert!(!application_key.is_empty());

    match client.get(B2_AUTHORIZE_ACCOUNT_URL)
        .basic_auth(key_id, Some(application_key))
        .send()
        {
            Ok(mut resp) => {
                let unparsed_json_response = resp.text().unwrap();
                match u16::from(resp.status()) {
                    200 => {
                        return Ok(serde_json::from_str(&unparsed_json_response).unwrap())
                    },
                    _ => {
                        return Err(serde_json::from_str(&unparsed_json_response).unwrap())
                    }
                }

            },
            Err(e) => {
                return Err(BackblazeErrorResponse{
                        code: BackblazeErrorKind::SendError,
                        status: 0,
                        message: format!("Could not send request {}", e)
                })
            },
    }
}

fn b2_list_file_names(client: &Client, auth: &BackblazeAuth, file_subdirectory: &str) -> Result<BackblazeListFileNamesResponse,BackblazeErrorResponse> {
    let mut body = BackblazeListFileNamesPostBody::default();
    body.bucketId = auth.allowed.bucketId.to_owned();
    body.maxFileCount = 1000;
    body.prefix = file_subdirectory.to_string();
    let body = serde_json::to_string(&body).unwrap();
    let api_url_cmd = format!("{}{}",&auth.apiUrl, "/b2api/v2/b2_list_file_names");

    match client.post(&api_url_cmd)
        .header("Authorization", &auth.authorizationToken)
        .body(body)
        .send()
    {
        Ok(mut resp) => {
            let unparsed_json_response = resp.text().unwrap();
            match u16::from(resp.status()) {
                200 => {
                    return Ok(serde_json::from_str(&unparsed_json_response).unwrap())
                },
                _ => {
                    return Err(serde_json::from_str(&unparsed_json_response).unwrap())
                }
            }
        }
        Err(e) => {
            Err(BackblazeErrorResponse {status: 0, code: BackblazeErrorKind::SendError, message: e.to_string()})
        }
    }
}

/// Tests if the file is publicly downloadable
/// Doesn't check the sha1 of the already uploaded file
fn b2_test_files_already_uploaded(client: &Client, files: &mut Vec<FileUploadInstance>, filenames_found_already: &BackblazeListFileNamesResponse) -> bool {
    let mut known_hashmap = HashMap::new();
    for already_uploaded_file in &filenames_found_already.files {
        known_hashmap.insert(already_uploaded_file.fileName.clone(), already_uploaded_file.contentSha1.clone());
    }
    for mut file in files {
        if let Ok(r) = client.get(&file.final_url).send() {
            if r.status().is_success() {
                if let Some(s) = known_hashmap.get(&file.relative_filename) {
                    if s == &file.sha1 {
                        file.already_uploaded = true;
                    }
                }
            }
        }
    }
    false
}

/// Get a Url to use for uploading
fn b2_get_upload_url(client: &Client, auth: &BackblazeAuth) -> Result<BackblazeGetUploadUrlResponse, BackblazeErrorResponse> {
    let api_url_cmd = format!("{}{}",&auth.apiUrl, "/b2api/v2/b2_get_upload_url");
    let body = format!("{{\"bucketId\":\"{}\"}}", auth.allowed.bucketId);

    match client.post(&api_url_cmd)
        .header("Authorization", &auth.authorizationToken)
        .body(body)
        .send()
    {
        Ok(mut resp) => {
            let unparsed_json_response = resp.text().unwrap();
            match u16::from(resp.status()) {
                200 => {
                    return Ok(serde_json::from_str(&unparsed_json_response).unwrap())
                },
                _ => {
                    return Err(serde_json::from_str(&unparsed_json_response).unwrap())
                }
            }
        }
        Err(e) => {
            Err(BackblazeErrorResponse{status: 0, code: BackblazeErrorKind::SendError, message: e.to_string()})
        }
    }
}

/// Naive implementation to get the mime type of a file based on file extension
/// Add additional extensions if you ever panic here
fn get_file_mimetype(filepath: &PathBuf) -> String {
    let extension = filepath.extension().unwrap_or_default().to_str().unwrap();
    match extension {
        "zip" => return "application/zip".to_string(),
        "txt" => return "text/plain".to_string(),
        "" => return "text/plain".to_string(),
        _ => {
            eprintln!("Unknown file extension ({}) needs a mime type defined, cannot upload", extension);
            panic!();
        },
    };
}

/// Uploads a file to b2 Backblaze, after getting an upload Url
fn b2_upload_file(client: &Client, url_response: &BackblazeGetUploadUrlResponse, file: &FileUploadInstance) -> Result<BackblazeUploadFileResponse,BackblazeErrorResponse> {
    let mime_type = get_file_mimetype(&file.filepath);

    match client.post(&url_response.uploadUrl)
        .header("Authorization", &url_response.authorizationToken)
        .header("X-Bz-File-Name", &file.relative_filename)
        .header("Content-Type", mime_type)
        .header("Content-Length", file.filepath.metadata().unwrap().len())
        .header("X-Bz-Content-Sha1", &file.sha1)
        .body(std::fs::File::open(&file.filepath).unwrap())
        .send() {
        Ok(mut resp) => {
            let unparsed_json_response = &resp.text().unwrap();
            match u16::from(resp.status()) {
                200 => {
                    return Ok(serde_json::from_str(&unparsed_json_response).unwrap())
                },
                _ => {
                    return Err(serde_json::from_str(&unparsed_json_response).unwrap())
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to post file to Backblaze during initial submission");
            panic!(e.to_string());
        },
    }
}

fn populate_final_urls(auth: &BackblazeAuth, files: &mut Vec<FileUploadInstance>) {
    for mut file in files {
        file.final_url = b2_download_url(auth, &file.relative_filename);
    }
}

/// Returns a properly formatted url of the file requested.
/// There is no guarentee the file will be present.
///
/// relative_file_name should include the subdirectories where this file will be found
fn b2_download_url(auth: &BackblazeAuth, relative_file_name: &str) -> String {
    format!("{}/file/{}/{}", auth.downloadUrl, auth.allowed.bucketName, relative_file_name)
}

fn get_relative_filename(subdir: &str, filename: &str) -> String {
    if subdir.is_empty() {
        return filename.to_string();
    }
    if cfg!(target_os = "linux") {
        if !subdir.ends_with('/') {
            format!("{}/{}", subdir, filename)
        } else {
            format!("{}{}", subdir, filename)
        }
    } else if !subdir.ends_with('\\') {
        format!("{}\\{}", subdir, filename)
    } else {
        format!("{}{}", subdir, filename)
    }
}

fn collect_file_upload_instances(subdir: &str, filepaths: &[PathBuf]) -> Vec<FileUploadInstance> {
    let mut instances = Vec::new();
    for file in filepaths {
        let mut instance = FileUploadInstance::default();
        let filename = file.file_name().unwrap().to_str().unwrap();
        instance.filepath = file.to_path_buf();
        instance.sha1 = sha1sum(file);
        instance.relative_filename = get_relative_filename(subdir, filename);
        instances.push(instance);
    }
    instances
}

/// Performs upload attempt with the b2-backblaze-keyID and b2-backblaze-
/// applicationKey found within your config.ini file.
///
/// Upon a successful upload it returns Ok(String) containing the url to download
/// the file from.
///
/// The url returned does not use the file id, thus only the most recently uploaded
/// file with the same name will be the one downloadable.
pub fn upload_files_to_backblaze(file_subdirectory: & str, filepaths: &[PathBuf]) -> Result<Vec<(PathBuf,String)>, String> {
    for filepath in filepaths {
        if !filepath.exists() {
            return Err("File does not exist!".to_string())
        }
        if !filepath.is_file() {
            return Err("Cannot upload a folder".to_string())
        }
    }

    let client = reqwest::Client::new();
    let mut state = BackblazeUploadState::GetAuth;
    let mut container = BackbazeDataContainer::default();
    container.files = collect_file_upload_instances(file_subdirectory, filepaths);

    let mut path_Url_pairs = Vec::new();

    let mut attempts = 0;

    loop {
        if path_Url_pairs.len() == container.files.len() {
            return Ok(path_Url_pairs);
        }
        if attempts >= 3 {
            return Err("Exhausted backblaze upload attempts!".to_string())
        }
        match state {
            BackblazeUploadState::GetAuth => {
                match authorize_cfg(&client) {
                    Ok(auth) => {
                        container.auth = Some(auth);
                        state = BackblazeUploadState::ListFileNames;
                    },
                    Err(e) => {
                        return Err(format!("Failed to authenticate with backblaze {:?}", e));
                    },
                }
            },
            BackblazeUploadState::ListFileNames => {
                match b2_list_file_names(&client, &container.auth.as_ref().unwrap(), file_subdirectory) {
                    Ok(resp) => {
                        container.already_uploaded_files = Some(resp);
                        state = BackblazeUploadState::TestIfAlreadyUploaded;
                    },
                    Err(err_resp) => {
                        attempts += 1;
                        println!("{:?}", state);
                        println!("{:?}", err_resp);
                        match err_resp.status {
                            400 => return Err(format!("Unrecoverable uploading error {:?}", err_resp)),
                            401 => {
                                state = BackblazeUploadState::GetAuth;
                            },
                            503 => {
                                std::thread::sleep(Duration::from_millis(1000));
                            }
                            _ => unreachable!("Recieved an impossible status {}", err_resp.status)
                        }
                    }
                }
            },
            BackblazeUploadState::TestIfAlreadyUploaded => {
                populate_final_urls(&container.auth.as_ref().unwrap(), &mut container.files);
                b2_test_files_already_uploaded(&client, &mut container.files, &container.already_uploaded_files.as_ref().unwrap());
                state = BackblazeUploadState::GetUploadUrl;
            },
            BackblazeUploadState::GetUploadUrl => {
                match b2_get_upload_url(&client, &container.auth.as_ref().unwrap()) {
                    Ok(resp) => {
                        container.get_url_response = Some(resp);
                        state = BackblazeUploadState::Upload;
                    },
                    Err(err_resp) => {
                        attempts += 1;
                        println!("{:?}", state);
                        println!("{:?}", err_resp);
                        match err_resp.status {
                            400 => return Err(format!("Unrecoverable uploading error {:?}", err_resp)),
                            401 => {
                                state = BackblazeUploadState::GetAuth;
                            },
                            503 => {
                                std::thread::sleep(Duration::from_millis(1000));
                                state = BackblazeUploadState::GetAuth;
                            }
                            _ => unreachable!("Recieved an impossible status {}", err_resp.status)
                        }
                    },
                }
            },
            BackblazeUploadState::Upload => {
                for file in &container.files {
                    if file.already_uploaded {
                        path_Url_pairs.push((file.filepath.clone(), file.final_url.clone()));
                    } else {
                        match b2_upload_file(&client, &container.get_url_response.as_ref().unwrap(), &file) {
                            Ok(_upload_resp) => {
                                path_Url_pairs.push((file.filepath.clone(), file.final_url.clone()));
                            },
                            Err(err_resp) => {
                                attempts += 1;
                                println!("{:?}", state);
                                println!("{:?}", err_resp);
                                match err_resp.status {
                                    400 => return Err(format!("Unrecoverable uploading error {:?}", err_resp)),
                                    401 => {
                                        match err_resp.code {
                                            BackblazeErrorKind::unauthorized => {
                                                return Err("API key does not allow uploading files".to_string());
                                            },
                                            BackblazeErrorKind::bad_auth_token => {
                                                state = BackblazeUploadState::GetUploadUrl;
                                            },
                                            BackblazeErrorKind::expired_auth_token => {
                                                state = BackblazeUploadState::GetUploadUrl;
                                            }
                                            _ => unreachable!("Reached an impossible error code {:?}", err_resp.code)
                                        }
                                    },
                                    403 => {
                                        return Err("Backblaze usage cap exceeded, cannot upload".to_string());
                                    },
                                    405 => {
                                        unreachable!("Did you get() when you should have post()?");
                                    }
                                    408 => {
                                        std::thread::sleep(Duration::from_millis(1000));
                                        // It's already in this state but this is explicit.
                                        state = BackblazeUploadState::Upload;
                                    }
                                    503 => {
                                        state = BackblazeUploadState::GetUploadUrl;
                                    }
                                    _ => {
                                        unreachable!("Recieved an impossible response status {}", err_resp.status)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::backblaze::authorize_test;
    use crate::util::fbh_save_dl_dir;
    use crate::backblaze::upload_files_to_backblaze;
    use std::fs::OpenOptions;
    use crate::backblaze::b2_list_file_names;
    use reqwest::Client;
    #[test]
    fn list_files() {
        let client = Client::new();
        let auth = authorize_test(&client).unwrap();
        b2_list_file_names(&client, &auth, "").unwrap();
    }
    #[test]
    fn upload_file() {
        match reqwest::get("https://f000.backblazeb2.com/file/cargo-test/this-is-a-test-generated-name-ignore-it.zip") {
            Ok(mut resp) => {
                let to_save_to_path = fbh_save_dl_dir().join("this-is-a-test-generated-name-ignore-it.zip");

                let mut file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&to_save_to_path)
                    .unwrap();
                resp.copy_to(&mut file).unwrap();
                let uploaded = upload_files_to_backblaze("", &[to_save_to_path.clone()]).unwrap();
                assert!(uploaded.len() == 1);
                let (k,v) = uploaded.iter().next().unwrap();
                assert_eq!(k, &to_save_to_path);
                assert_eq!(v, "https://f000.backblazeb2.com/file/cargo-test/this-is-a-test-generated-name-ignore-it.zip");
                std::fs::remove_file(&to_save_to_path).unwrap();
            },
            Err(e) => panic!(e),
        }
    }
}
