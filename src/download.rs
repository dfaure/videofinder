use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::PathBuf;

fn db_dir() -> PathBuf {
    if cfg!(target_os = "android") {
        //PathBuf::from("/storage/emulated/0/Download")
        PathBuf::from("/storage/emulated/0/Android/data/fr.davidfaure.videofinder.slint/files/")
    } else {
        dirs::home_dir().expect("No home directory found")
    }
}

/// HDDs whose JSONL slices should be merged into the queryable DB.
/// Each name must match the LOCATION label produced by scripts/scan_hdd.py
/// (derived from the HDD's `id` file). The corresponding files on the FTP
/// server are `<NAME>.jsonl`, served alongside kvideomanager.sqlite.
pub const HDD_NAMES: &[&str] = &["ELORA_1", "ELORA_2", "ELORA_3"];

/// Path to the Qt-curated source DB (downloaded as-is from FTP).
pub fn qt_db_full_path() -> PathBuf {
    db_dir().join("kvideomanager.sqlite")
}

/// Path to the merged DB that videofinder actually queries. Produced by
/// merging the Qt source DB with the per-HDD JSONL slices.
pub fn db_full_path() -> PathBuf {
    db_dir().join("merged.sqlite")
}

pub fn jsonl_full_path(hdd_name: &str) -> PathBuf {
    db_dir().join(format!("{}.jsonl", hdd_name))
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
) -> Result<(), anyhow::Error> {
    log::info!("Starting download from {}", url_str);

    let client = Client::new();
    let response = client.get(url_str).send().await?.error_for_status()?;

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
pub fn parse_file_list() -> Result<ImageForDirHash, anyhow::Error> {
    log::debug!("parse_file_list");
    let file = File::open(filelist_full_path())?;
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

const BASE_URL: &str = "http://www.davidfaure.fr/kvideomanager";

pub async fn download_db(progress_func: Box<ProgressFunc>) -> Result<(), anyhow::Error> {
    log::info!("download_db begin");
    let target_dir = db_dir();
    if !target_dir.exists() {
        let error_msg = format!("Local dir does not exist: {}", target_dir.display());
        log::warn!("Local dir does not exist: {}", target_dir.display());
        anyhow::bail!(error_msg);
    }

    // Progress is only wired to the (much larger) Qt DB download; the JSONL
    // files are kilobytes each, so we don't bother reporting their progress.
    let qt_url = format!("{}/kvideomanager.sqlite", BASE_URL);
    let qt_path = qt_db_full_path();
    download_to_file(&qt_url, qt_path.clone(), progress_func).await?;

    let filelist_url = format!("{}/kvideomanager.filelist.txt", BASE_URL);
    let dummy_fn = Box::new(|_| {});
    download_to_file(&filelist_url, filelist_full_path(), dummy_fn).await?;

    // HDD slices: download each one, but a missing/unreachable file just
    // produces a warning — the merge step will skip whatever isn't present.
    let mut jsonl_paths: Vec<PathBuf> = Vec::new();
    for hdd in HDD_NAMES {
        let url = format!("{}/{}.jsonl", BASE_URL, hdd);
        let path = jsonl_full_path(hdd);
        match download_to_file(&url, path.clone(), Box::new(|_| {})).await {
            Ok(()) => jsonl_paths.push(path),
            Err(e) => log::warn!("Failed to download {}: {}", url, e),
        }
    }

    crate::merge::merge(&qt_path, &jsonl_paths, &db_full_path())?;
    Ok(())
}

use slint::Image;
use std::path::Path;
use tempfile::Builder;

/// Downloads the image at `url_str`, writes it to a temp file, and loads it via `slint::Image::load_from_path`.
pub async fn download_image_data(url_str: &str) -> Result<Image, anyhow::Error> {
    log::info!("Downloading image from {}", url_str);

    let client = Client::new();
    let response = client.get(url_str).send().await?.error_for_status()?;

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
