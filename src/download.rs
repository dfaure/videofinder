use std::io::Write;
use std::io::BufRead;
use std::io::BufReader;
use std::fs::File;
use std::path::PathBuf;
use std::env;
use std::error::Error;

fn db_dir() -> PathBuf {
    if cfg!(target_os = "android") {
        //PathBuf::from("/storage/emulated/0/Download")
        PathBuf::from("/storage/emulated/0/Android/data/fr.davidfaure.videofinder/files/")
    } else {
        PathBuf::from(env::home_dir().unwrap_or("No Home Dir!".into()))
    }
}

fn db_fname() -> &'static str {
    "kvideomanager.sqlite"
}
pub fn db_full_path() -> PathBuf {
    db_dir().join(db_fname())
}

pub fn filelist_full_path() -> PathBuf {
    db_dir().join("filelist.txt")
}

use reqwest::Client;
use futures_util::stream::StreamExt;

/// Type alias for the progress reporting function.
/// It receives a `f32` in the range 0.0 to 1.0.
pub type ProgressFunc = dyn FnMut(f32) + 'static;

/// Downloads the content of the given URL into the specified file, reporting progress.
pub async fn download_to_file(
    url_str: &str,
    file_path: PathBuf,
    mut progress_func: Box<ProgressFunc>,
) -> Result<(), Box<dyn Error>> {
    log::info!("Starting download from {}", url_str);

    let client = Client::new();
    let response = client.get(url_str).send().await?;

    if !response.status().is_success() {
        return Err(format!("Request failed with status {}", response.status()).into());
    }

    let total = response.content_length().unwrap_or(0);
    let mut downloaded = 0u64;
    let mut stream = response.bytes_stream();
    let mut file = File::create(&file_path)?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;

        if total > 0 {
            let progress = downloaded as f32 / total as f32;
            progress_func(progress);
        }
    }

    log::info!("Download finished: {} bytes written to {}", downloaded, file_path.display());
    Ok(())
}

pub type ImageForDirHash = std::collections::HashMap<PathBuf, PathBuf>;
pub fn parse_file_list() -> Result<ImageForDirHash, Box<dyn Error>>
{
    log::debug!("parse_file_list");
    match File::open(filelist_full_path()) {
        Ok(file) => {
            let mut hash = ImageForDirHash::new();
            let lines = BufReader::new(file).lines();
            for line in lines {
                let line = line?;
                if let Some(pos) = line.rfind('/') {
                    let (dir, file_name) = line.split_at(pos);
                    let file_name = &file_name[1..]; // skip leading '/'
                    let dir_path = PathBuf::from(dir);
                    hash.entry(dir_path).or_insert(PathBuf::from(file_name));
                }
            }
            //log::debug!("{:?}", hash);
            Ok(hash)
        },
        Err(e) => {
            log::warn!("{:?}", e);
            Err(Box::new(e))
        }
    }
}

pub async fn download_db(
    progress_func: Box<ProgressFunc>,
) -> Result<(), Box<dyn Error>> {
    log::info!("download_db begin");
    let target_dir = db_dir();
    if !target_dir.exists() {
        let error_msg = format!("Local dir does not exist: {}", target_dir.display());
        log::warn!("Local dir does not exist: {}", target_dir.display());
        //return Err(DownloadError::LocalError(error_msg));
        return Err(error_msg.into());
    }
    let url = "http://www.davidfaure.fr/kvideomanager/kvideomanager.sqlite";
    let file_path = db_full_path();
    download_to_file(url, file_path, progress_func).await?;

    let filelist_url = "http://www.davidfaure.fr/kvideomanager/kvideomanager.filelist.txt";
    let dummy_fn = Box::new(|_| {});
    let file_list_path = filelist_full_path();
    download_to_file(filelist_url, file_list_path, dummy_fn).await
}

async fn download_body_bytes(
    url: &str
) -> Result<bytes::Bytes, Box<dyn Error>> {
    Err("not impl".into())
    /*
    let (mut res, conn) = http_get(url).await?;

    let collect_fut = res.body_mut().collect();
    let (body_result, conn_result) = join!(collect_fut, conn);
    let body = body_result?.to_bytes();
    conn_result?; // optional: check if conn closed cleanly
    Ok(body)
    */
}

pub async fn download_image_data(url_str: &str) -> Result<slint::Image, Box<dyn Error>> {
    log::info!("download_image_data {}", url_str);
    let data = download_body_bytes(url_str).await?;
    //let image = slint::Image::load_from_encoded(data.to_vec())?;
    //Ok(image)
    Err("not impl".into())
}
