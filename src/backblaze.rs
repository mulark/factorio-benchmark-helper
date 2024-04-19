// Keep returned values from API in same camelCase format, or snake_case as applicable
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
use crate::util::config_file::CONFIG_FILE_SETTINGS;
use crate::util::sha1sum;
use percent_encoding::percent_encode;
use percent_encoding::{AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;
use ureq::Agent;

const PERCENT_ENCODE_SET: &AsciiSet =
    &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');
const B2_AUTHORIZE_ACCOUNT_URL: &str =
    "https://api.backblazeb2.com/b2api/v2/b2_authorize_account";

/// A container holding the pieces unique to each file necessary to upload it to Backblaze.
#[derive(Clone, Default, Debug)]
struct FileUploadInstance {
    /// The filepath of the file we intend to upload.
    filepath: PathBuf,

    /// The sha1 sum of the file we intend to upload.
    sha1: String,

    /// The relative filename with included position within subfolders.
    /// This uses forward slashes even on Windows because that's what
    /// Backblaze B2 expects.
    relative_filename: String,

    /// The final component url where the uploaded file will be downloadable.
    final_url: String,

    /// Whether this file is already uploaded to b2 backblaze,
    /// based on the final url and the sha1 sum of the file.
    already_uploaded: bool,

    /// The base part of the url where we will upload this file.
    get_upload_url_response: BackblazeGetUploadUrlResponse,

    /// The upload's parsed JSON response if available.
    #[allow(dead_code)]
    upload_response: Option<BackblazeUploadFileResponse>,
}

#[derive(Clone, Debug, Default)]
struct BackbazeDataContainer {
    auth: Option<BackblazeAuth>,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    dirty: bool,
}

/// The bucket this keyID and applicationKey are allowed to access.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct BackblazeAuthAllowed {
    bucketId: String,
    bucketName: String,
    capabilities: Vec<String>,
}

/// The response to calling b2_get_upload_url containing the bucket,
/// the Url to use to upload with, and the token authorize with.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct BackblazeGetUploadUrlResponse {
    bucketId: String,
    uploadUrl: String,
    authorizationToken: String,
}

/// A struct holding the parameters we need to POST after serializing into JSON
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct BackblazeListFileNamesPostBody {
    bucketId: String,
    maxFileCount: u16,
    prefix: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct BackblazeListFileNamesResponse {
    files: Vec<BackblazeUploadFileResponse>,
    // TODO support more than 1k files in a subdirectory.
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

/// An error response when attempting to perform an action on the Backblaze API,
/// or failure to perform an action.
#[derive(Deserialize, Debug, Clone)]
struct BackblazeErrorResponse {
    status: u16,
    code: BackblazeErrorKind,
    #[allow(dead_code)]
    message: String,
}

#[derive(Deserialize, Debug, Clone)]
enum BackblazeErrorKind {
    SendError,         // 0 - Failed for some reason attempting to send data
    KeysNotPresent,    // 0 - Failed to read needed keys from config file
    bad_request,       // 400 or 503
    invalid_bucket_id, // 400
    out_of_range,      // 400
    unauthorized,      // 401
    unsupported,       // 401
    bad_auth_token,    // 401
    expired_auth_token, // 401
    cap_exceeded,      // 403
    method_not_allowed, // 405
    request_timeout,   // 408
    service_unavailable, // 503
}

/// The states possible for uploading files to Backblaze.
///
/// We can return to earlier states when Backblaze responses indicate it.
/// For example: if our auth token expires, we can return to GetAuth.
#[derive(Debug)]
enum BackblazeUploadState {
    GetAuth,
    ListFileNames,
    TestIfAlreadyUploaded,
    GetUploadUrl,
    Upload,
}

/// Authorize with the Backblaze API using test credentials.
///
/// These credentials can be found in the envs TRAVIS_CI_B2_KEYID & TRAVIS_CI_B2_APPLICATIONKEY
/// or within the config.ini file under the same name.
/// These fields are not written to the config.ini file normally,
/// but will be carried over to future config file versions.
fn authorize_test(
    mut client: &mut Agent,
) -> Result<BackblazeAuth, BackblazeErrorResponse> {
    let vars = std::env::vars().collect::<HashMap<String, String>>();
    let key_id = if vars.get("TRAVIS_CI_B2_KEYID").is_some() {
        vars.get("TRAVIS_CI_B2_KEYID").unwrap()
    } else {
        &CONFIG_FILE_SETTINGS.travis_ci_b2_key_id
    };
    let application_key = if vars.get("TRAVIS_CI_B2_APPLICATIONKEY").is_some() {
        vars.get("TRAVIS_CI_B2_APPLICATIONKEY").unwrap()
    } else {
        &CONFIG_FILE_SETTINGS.travis_ci_b2_applicationkey
    };
    authorize(&mut client, key_id, application_key)
}

/// Authorize with Backblaze API using keys found within config.ini
fn authorize_cfg(
    mut client: &mut Agent,
) -> Result<BackblazeAuth, BackblazeErrorResponse> {
    if cfg!(test) {
        return authorize_test(&mut client);
    }
    let key_id = &CONFIG_FILE_SETTINGS.b2_backblaze_key_id;
    let application_key = &CONFIG_FILE_SETTINGS.b2_backblaze_application_key;
    if !key_id.is_empty() && !application_key.is_empty() {
        return authorize(client, key_id, application_key);
    }
    Err(BackblazeErrorResponse {
        status: 0,
        code: BackblazeErrorKind::KeysNotPresent,
        message: "Could not get keys from config.ini".to_string(),
    })
}

/// Authorize with Backblaze using an application key and key ID.
fn authorize(
    client: &mut Agent,
    key_id: &str,
    application_key: &str,
) -> Result<BackblazeAuth, BackblazeErrorResponse> {
    assert!(!key_id.is_empty());
    assert!(!application_key.is_empty());

    let resp = client
        .auth(key_id, application_key)
        .get(B2_AUTHORIZE_ACCOUNT_URL)
        .call();
    match resp.status() {
        200 => Ok(resp.into_json_deserialize().unwrap()),
        _ => Err(resp.into_json_deserialize().unwrap()),
    }
}

fn b2_list_file_names(
    client: &Agent,
    auth: &BackblazeAuth,
    file_subdirectory: &str,
) -> Result<BackblazeListFileNamesResponse, BackblazeErrorResponse> {
    let mut body = BackblazeListFileNamesPostBody::default();
    body.bucketId = auth.allowed.bucketId.to_owned();
    body.maxFileCount = 1000;
    body.prefix = file_subdirectory.to_string();
    let body = serde_json::to_string(&body).unwrap();
    let api_url_cmd =
        format!("{}{}", &auth.apiUrl, "/b2api/v2/b2_list_file_names");

    let resp = client
        .post(&api_url_cmd)
        .set("Authorization", &auth.authorizationToken)
        .send_string(&body);

    match resp.status() {
        200 => Ok(resp.into_json_deserialize().unwrap()),
        _ => Err(resp.into_json_deserialize().unwrap()),
    }
}

/// Tests if the file is publicly downloadable
/// If it is then it checks that the sha1 sum matches the file we intend to upload.
fn b2_test_files_already_uploaded(
    client: &Agent,
    files: &mut Vec<FileUploadInstance>,
    filenames_found_already: &BackblazeListFileNamesResponse,
) -> bool {
    let mut known_hashmap = HashMap::new();
    for already_uploaded_file in &filenames_found_already.files {
        known_hashmap.insert(
            already_uploaded_file.fileName.clone(),
            already_uploaded_file.contentSha1.clone(),
        );
    }
    for file in files {
        let resp = client.get(&file.final_url).call();
        if resp.status() == 200 {
            if let Some(s) = known_hashmap.get(&file.relative_filename) {
                if s == &file.sha1 {
                    file.already_uploaded = true;
                }
            }
        }
    }
    false
}

fn get_b2_upload_urls_per_file(
    client: &Agent,
    container: &mut BackbazeDataContainer,
) -> Result<(), BackblazeErrorResponse> {
    for file in &mut container.files {
        file.get_upload_url_response =
            b2_get_upload_url(&client, &container.auth.as_ref().unwrap())?;
    }
    Ok(())
}

/// Get a Url to use for uploading
fn b2_get_upload_url(
    client: &Agent,
    auth: &BackblazeAuth,
) -> Result<BackblazeGetUploadUrlResponse, BackblazeErrorResponse> {
    let api_url_cmd =
        format!("{}{}", &auth.apiUrl, "/b2api/v2/b2_get_upload_url");
    let body = format!("{{\"bucketId\":\"{}\"}}", auth.allowed.bucketId);

    let resp = client
        .post(&api_url_cmd)
        .set("Authorization", &auth.authorizationToken)
        .send_string(&body);
    match resp.status() {
        200 => Ok(resp.into_json_deserialize().unwrap()),
        _ => Err(resp.into_json_deserialize().unwrap()),
    }
}

/// Naive implementation to get the mime type of a file based on file extension
/// Add additional extensions if you ever panic here
fn get_file_mimetype(filepath: &PathBuf) -> String {
    let extension = filepath.extension().unwrap_or_default().to_str().unwrap();
    match extension {
        "zip" => "application/zip".to_string(),
        "txt" => "text/plain".to_string(),
        "" => "text/plain".to_string(),
        _ => {
            eprintln!(
                "Unknown file extension ({}) needs a mime type defined, cannot upload",
                extension
            );
            panic!();
        }
    }
}

/// Uploads a file to b2 Backblaze, after getting an upload Url
fn b2_upload_file(
    client: &Agent,
    url_response: &BackblazeGetUploadUrlResponse,
    file: &FileUploadInstance,
) -> Result<BackblazeUploadFileResponse, BackblazeErrorResponse> {
    let mime_type = get_file_mimetype(&file.filepath);
    let percent_encoded_filename =
        percent_encode(file.relative_filename.as_bytes(), PERCENT_ENCODE_SET)
            .to_string();

    let mut file_buf = vec![];
    std::fs::File::open(&file.filepath)
        .unwrap()
        .read_to_end(&mut file_buf)
        .unwrap();
    let resp = client
        .post(&url_response.uploadUrl)
        .set("Authorization", &url_response.authorizationToken)
        .set("X-Bz-File-Name", &percent_encoded_filename)
        .set("Content-Type", &mime_type)
        .set(
            "Content-Length",
            &file.filepath.metadata().unwrap().len().to_string(),
        )
        .set("X-Bz-Content-Sha1", &file.sha1)
        .send_bytes(&file_buf);

    match resp.status() {
        200 => Ok(resp.into_json_deserialize().unwrap()),
        _ => panic!("{:?}", resp),
    }
}

fn populate_final_urls(
    auth: &BackblazeAuth,
    files: &mut Vec<FileUploadInstance>,
) {
    for file in files {
        file.final_url = b2_download_url(auth, &file.relative_filename);
    }
}

/// Returns a properly formatted url of the file requested.
/// There is no guarentee the file will be present.
///
/// relative_file_name should include the subdirectories where this file will be found
/// These relative subdirectories use forward slashes, even on Windows
fn b2_download_url(auth: &BackblazeAuth, relative_file_name: &str) -> String {
    format!(
        "{}/file/{}/{}",
        auth.downloadUrl, auth.allowed.bucketName, relative_file_name
    )
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
    } else {
        format!("{}/{}", subdir.replace("\\", ""), filename)
    }
}

fn collect_file_upload_instances(
    subdir: &str,
    filepaths: &[PathBuf],
) -> Vec<FileUploadInstance> {
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
/// Upon a successful upload it returns Ok((PathBuf, String)) containing the filepath
/// of the uploaded file and the url to download the file from.
///
/// The url returned does not use the file id, thus only the most recently uploaded
/// file with the same name will be the one downloadable.
pub fn upload_files_to_backblaze(
    file_subdirectory: &str,
    filepaths: &[PathBuf],
) -> Result<Vec<(PathBuf, String)>, String> {
    for filepath in filepaths {
        if !filepath.exists() {
            return Err("File does not exist!".to_string());
        }
        if !filepath.is_file() {
            return Err("Cannot upload a folder".to_string());
        }
    }

    let mut client = Agent::new();
    let mut state = BackblazeUploadState::GetAuth;
    let mut container = BackbazeDataContainer::default();
    container.files =
        collect_file_upload_instances(file_subdirectory, filepaths);

    let mut path_Url_pairs = Vec::new();

    let mut attempts = 0;

    loop {
        if path_Url_pairs.len() == container.files.len() {
            return Ok(path_Url_pairs);
        }
        if attempts >= 3 {
            return Err("Exhausted backblaze upload attempts!".to_string());
        }
        match state {
            BackblazeUploadState::GetAuth => match authorize_cfg(&mut client) {
                Ok(auth) => {
                    container.auth = Some(auth);
                    state = BackblazeUploadState::ListFileNames;
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to authenticate with backblaze {:?}",
                        e
                    ));
                }
            },
            BackblazeUploadState::ListFileNames => {
                match b2_list_file_names(
                    &client,
                    &container.auth.as_ref().unwrap(),
                    file_subdirectory,
                ) {
                    Ok(resp) => {
                        container.already_uploaded_files = Some(resp);
                        state = BackblazeUploadState::TestIfAlreadyUploaded;
                    }
                    Err(err_resp) => {
                        attempts += 1;
                        println!("{:?}", state);
                        println!("{:?}", err_resp);
                        match err_resp.status {
                            400 => {
                                return Err(format!(
                                    "Unrecoverable uploading error {:?}",
                                    err_resp
                                ))
                            }
                            401 => {
                                state = BackblazeUploadState::GetAuth;
                            }
                            503 => {
                                std::thread::sleep(Duration::from_millis(1000));
                            }
                            _ => unreachable!(
                                "Recieved an impossible status {}",
                                err_resp.status
                            ),
                        }
                    }
                }
            }
            BackblazeUploadState::TestIfAlreadyUploaded => {
                populate_final_urls(
                    &container.auth.as_ref().unwrap(),
                    &mut container.files,
                );
                b2_test_files_already_uploaded(
                    &client,
                    &mut container.files,
                    &container.already_uploaded_files.as_ref().unwrap(),
                );
                state = BackblazeUploadState::GetUploadUrl;
            }
            BackblazeUploadState::GetUploadUrl => {
                match get_b2_upload_urls_per_file(&client, &mut container) {
                    Ok(()) => {
                        state = BackblazeUploadState::Upload;
                    }
                    Err(err_resp) => {
                        attempts += 1;
                        println!("{:?}", state);
                        println!("{:?}", err_resp);
                        match err_resp.status {
                            400 => {
                                return Err(format!(
                                    "Unrecoverable uploading error {:?}",
                                    err_resp
                                ))
                            }
                            401 => {
                                state = BackblazeUploadState::GetAuth;
                            }
                            503 => {
                                std::thread::sleep(Duration::from_millis(1000));
                                state = BackblazeUploadState::GetAuth;
                            }
                            _ => unreachable!(
                                "Recieved an impossible status {}",
                                err_resp.status
                            ),
                        }
                    }
                }
            }
            BackblazeUploadState::Upload => {
                for file in &container.files {
                    if file.already_uploaded {
                        path_Url_pairs.push((
                            file.filepath.clone(),
                            file.final_url.clone(),
                        ));
                    } else {
                        match b2_upload_file(
                            &client,
                            &file.get_upload_url_response,
                            &file,
                        ) {
                            Ok(_upload_resp) => {
                                path_Url_pairs.push((
                                    file.filepath.clone(),
                                    file.final_url.clone(),
                                ));
                            }
                            Err(err_resp) => {
                                attempts += 1;
                                println!("{:?}", state);
                                println!("{:?}", err_resp);
                                match err_resp.status {
                                    0 => {
                                        //Sending error, probably try to get a new upload URL.
                                        eprintln!("filename {:?}", file.relative_filename);
                                        state = BackblazeUploadState::TestIfAlreadyUploaded;
                                    }
                                    400 => {
                                        return Err(format!(
                                            "Unrecoverable uploading error {:?}",
                                            err_resp
                                        ))
                                    }
                                    401 => match err_resp.code {
                                        BackblazeErrorKind::unauthorized => {
                                            return Err("API key does not allow uploading files"
                                                .to_string());
                                        }
                                        BackblazeErrorKind::bad_auth_token => {
                                            state = BackblazeUploadState::GetUploadUrl;
                                        }
                                        BackblazeErrorKind::expired_auth_token => {
                                            state = BackblazeUploadState::GetUploadUrl;
                                        }
                                        _ => unreachable!(
                                            "Reached an impossible error code {:?}",
                                            err_resp.code
                                        ),
                                    },
                                    403 => {
                                        return Err("Backblaze usage cap exceeded, cannot upload"
                                            .to_string());
                                    }
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
                                    _ => unreachable!(
                                        "Recieved an impossible response status {}",
                                        err_resp.status
                                    ),
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
    use crate::backblaze::b2_list_file_names;
    use crate::backblaze::upload_files_to_backblaze;
    use crate::util::fbh_save_dl_dir;
    use std::fs::OpenOptions;
    use std::io::Read;
    use std::io::Write;
    use ureq::Agent;

    #[test]
    fn list_files() {
        let mut client = Agent::new();
        let auth = authorize_test(&mut client).unwrap();
        b2_list_file_names(&client, &auth, "").unwrap();
    }

    #[test]
    fn upload_file() {
        let resp = ureq::get("https://f000.backblazeb2.com/file/cargo-test/this-is-a-test-generated-name-ignore-it.zip").call();

        std::fs::create_dir_all(fbh_save_dl_dir()).unwrap();
        let to_save_to_path = fbh_save_dl_dir()
            .join("this-is-a-test-generated-name-ignore-it.zip");

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&to_save_to_path)
            .unwrap();
        let mut buf = Vec::new();
        resp.into_reader().read_to_end(&mut buf).unwrap();
        file.write_all(&buf).unwrap();
        let uploaded =
            upload_files_to_backblaze("", &[to_save_to_path.clone()]).unwrap();
        assert!(uploaded.len() == 1);
        let (k, v) = uploaded.get(0).unwrap();
        assert_eq!(k, &to_save_to_path);
        assert_eq!(v, "https://f000.backblazeb2.com/file/cargo-test/this-is-a-test-generated-name-ignore-it.zip");
        std::fs::remove_file(&to_save_to_path).unwrap();
    }

    #[test]
    fn test_percent_encodedness() {
        let resp = ureq::get(
            "https://f000.backblazeb2.com/file/cargo-test/Spa+ce/new+file.txt",
        )
        .call();
        let newfilepath = std::env::current_dir().unwrap().join("new file.txt");
        let mut newfile = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&newfilepath)
            .unwrap();
        let mut buf = Vec::new();
        resp.into_reader().read_to_end(&mut buf).unwrap();
        newfile.write_all(&buf).unwrap();
        let uploaded =
            upload_files_to_backblaze("Spa ce", &[newfilepath.clone()])
                .unwrap();
        assert!(uploaded.len() == 1);
        let (k, v) = uploaded.get(0).unwrap();
        assert_eq!(k, &newfilepath);
        assert_eq!(
            v,
            "https://f000.backblazeb2.com/file/cargo-test/Spa ce/new file.txt"
        );
        std::fs::remove_file(&newfilepath).unwrap();
    }
}
