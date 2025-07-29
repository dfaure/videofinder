use std::io::Write;
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

use hyper::{Request, Uri};
use hyper::client::conn;
use hyper::header::HOST;
use hyper::body::Bytes;
use tokio::net::TcpStream;
use http_body_util::BodyExt;
use hyper_util::rt::TokioIo;

type ProgressFunc = dyn FnMut(f32) + 'static;

// Based on https://hyper.rs/guides/1/client/basic/
pub async fn download_to_file(
    url_str: &'static str,
    file_path: PathBuf,
    mut progress_func: Box<ProgressFunc>,
    ) -> Result<(), Box<dyn Error>>
{
    let url = url_str.parse::<Uri>()?;
    let host = url.host().expect("uri has no host");
    let port = url.port_u16().unwrap_or(80);
    let address = format!("{}:{}", host, port);
    log::info!("will get {} from {}", url, address);

    let stream = TcpStream::connect(address).await?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = conn::http1::handshake(io).await?;

    // Drive connection in background
    tokio::spawn(async move {
        if let Err(err) = conn.await {
            log::info!("Connection failed: {:?}", err);
        }
    });

    let authority = url.authority().unwrap().clone();
    let req = Request::builder()
        .uri(url)
        .header(HOST, authority.as_str())
        .body(http_body_util::Empty::<Bytes>::new())
        .unwrap();

    let mut res = sender.send_request(req).await?;
    log::info!("Response status: {}", res.status());

    let file_path_str = file_path.clone();
    let mut dest = {
        log::info!("will be located under: '{:?}'", file_path);
        File::create(file_path)?
    };

    let mut downloaded = 0u64;

    let total_size = res
        .headers()
        .get("content-length")                 // Get the header value
        .and_then(|h| h.to_str().ok())         // Convert from HeaderValue to &str
        .and_then(|s| s.parse::<u64>().ok());  // Parse it to a u64
    log::info!("Content length: {:?}", total_size);

    // Stream the body, writing each frame to stdout as it arrives
    while let Some(next) = res.frame().await {
        let frame = next?;
        if let Some(chunk) = frame.data_ref() {
            dest.write_all(chunk)?;
            downloaded += chunk.len() as u64;
             if let Some(total) = total_size {
                let progress = downloaded as f32 / total as f32;
                progress_func(progress); // UI callback
            }
        }
    }

    log::info!("Done downloading {}", file_path_str.display());
    Ok(())
}

use std::io::BufReader;
use std::io::BufRead;

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
