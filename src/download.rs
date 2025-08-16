use std::env;
use std::error::Error;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::PathBuf;

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

use futures_util::stream::StreamExt;
use reqwest::Client;

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
pub fn parse_file_list() -> Result<ImageForDirHash, Box<dyn Error>> {
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
        }
        Err(e) => {
            log::warn!("{:?}", e);
            Err(Box::new(e))
        }
    }
}

pub async fn download_db(progress_func: Box<ProgressFunc>) -> Result<(), Box<dyn Error>> {
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

use slint::Image;
use std::path::Path;
use tempfile::Builder;

/// Downloads the image at `url_str`, writes it to a temp file, and loads it via `slint::Image::load_from_path`.

pub async fn download_image_data(url_str: &str) -> Result<Image, Box<dyn Error>> {
    log::info!("Downloading image from {}", url_str);

    let client = Client::new();
    let response = client.get(url_str).send().await?;

    if !response.status().is_success() {
        return Err(format!("Request failed with status {}", response.status()).into());
    }

    let bytes = response.bytes().await?;
    log::info!("Image downloaded, {} bytes", bytes.len());

    // Extract extension from URL
    let extension = Path::new(url_str).extension().and_then(|ext| ext.to_str()).unwrap_or("img");

    // Create a temporary file with the appropriate extension
    let temp_file = Builder::new().suffix(&format!(".{}", extension)).tempfile()?;

    let temp_path = temp_file.path().to_owned();
    {
        let mut file = File::create(&temp_path)?;
        file.write_all(&bytes)?;
    }

    // Load using Slint
    let image = Image::load_from_path(&temp_path)?;
    log::info!("Image loaded from path: {:?}", temp_path);

    Ok(image)
}
