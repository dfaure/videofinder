use std::io::Write;
use std::fs::File;
use std::path::Path;
use thiserror::Error;

use crate::log;

#[derive(Error, Debug)]
pub enum DownloadError {
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

pub async fn download_db() -> Result<(), DownloadError> {
    let tmp_dir = Path::new("/storage/emulated/0/Documents/");
    log!("tmp_dir: '{}'", tmp_dir.display());
    let target = "http://www.davidfaure.fr/kvideomanager/kvideomanager.sqlite";
    let response = reqwest::get(target).await?;

    let mut dest = {
        let fname = "kvideomanager.sqlite";
        let filePath = tmp_dir.join(fname);
        log!("will be located under: '{:?}'", filePath);
        File::create(filePath)?
    };
    let content = response.bytes().await?;
    dest.write_all(&content)?;
    Ok(())
}
