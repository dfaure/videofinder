//use std::io::Write;
//use std::fs::File;
use std::path::PathBuf;
use std::env;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DownloadError {
    //#[error("Local error: {0}")]
    //LocalError(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] reqwest::Error),
    // Add other specific errors as needed
}

// In your functions, you can then return `Result<T, DownloadError>`
// async fn download_db() -> Result<()> {
    // ... your download logic
    // If a `reqwest::Error` or `std::io::Error` occurs,
    // the `#[from]` attribute will automatically convert it to DownloadError
    // Ok(())
//}

fn db_dir() -> PathBuf {
    if cfg!(target_os = "android") {
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

/*
pub async fn download_db() -> Result<(), DownloadError> {
    let target_dir = db_dir();
    if !target_dir.exists() {
        let error_msg = format!("Local dir does not exist: {}", target_dir.display());
        return Err(DownloadError::LocalError(error_msg));
    }
    let url = "http://www.davidfaure.fr/kvideomanager/kvideomanager.sqlite";
    let response = reqwest::get(url).await?;

    let mut dest = {
        let file_path = db_full_path();
        log::debug!("will be located under: '{:?}'", file_path);
        File::create(file_path)?
    };
    let content = response.bytes().await?;
    dest.write_all(&content)?;
    Ok(())
}
*/
