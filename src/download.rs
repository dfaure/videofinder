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

use hyper::{Request, Response, Uri};
use hyper::client::conn;
use hyper::header::HOST;
use hyper::body::Bytes;
use hyper::body::Incoming;
use tokio::net::TcpStream;
use http_body_util::BodyExt;
use hyper_util::rt::TokioIo;
use futures::{FutureExt, select, pin_mut};
use std::future::Future;

type ProgressFunc = dyn FnMut(f32) + 'static;

async fn http_connect(
    url_str: &str,
) -> Result<
    (
        Uri,
        conn::http1::SendRequest<http_body_util::Empty<Bytes>>,
        impl Future<Output = Result<(), hyper::Error>>,
    ),
    Box<dyn Error>,
> {
    let url = url_str.parse::<Uri>()?;
    let host = url.host().ok_or("missing host")?;
    let port = url.port_u16().unwrap_or(80);
    let address = format!("{}:{}", host, port);

    let stream = TcpStream::connect(address).await?;
    let io = TokioIo::new(stream);
    let (sender, conn) = conn::http1::handshake(io).await?;

    Ok((url, sender, conn))
}

fn build_request(url: &Uri) -> Result<Request<http_body_util::Empty<Bytes>>, Box<dyn Error>> {
    let authority = url
        .authority()
        .ok_or("missing authority")?
        .clone();

    let req = Request::builder()
        .uri(url)
        .header(HOST, authority.as_str())
        .body(http_body_util::Empty::new())?;

    Ok(req)
}

async fn http_send_request(
    url: &Uri,
    sender: &mut conn::http1::SendRequest<http_body_util::Empty<Bytes>>,
) -> Result<Response<Incoming>, Box<dyn Error>> {
    let req = build_request(url)?;
    let res = sender.send_request(req).await?;
    Ok(res)
}

// Based on https://hyper.rs/guides/1/client/basic/
pub async fn download_to_file(
    url_str: &str,
    file_path: PathBuf,
    mut progress_func: Box<ProgressFunc>,
    ) -> Result<(), Box<dyn Error>>
{
    let (url, mut sender, conn) = http_connect(url_str).await?;
    log::debug!("http_connect done");
    let mut res = http_send_request(&url, &mut sender).await?;
    log::debug!("http_send_request done");

    let total_size = res
        .headers()
        .get("content-length")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    log::info!("Content length: {:?}", total_size);

    let mut dest = File::create(file_path)?;
    let mut downloaded = 0u64;

    let conn_fut = conn.fuse(); // make it FusedFuture for select!
    let body_fut = async {
        while let Some(next) = res.frame().await {
            let frame = next?;
            if let Some(chunk) = frame.data_ref() {
                dest.write_all(chunk)?;
                downloaded += chunk.len() as u64;

                if let Some(total) = total_size {
                    let progress = downloaded as f32 / total as f32;
                    progress_func(progress);
                }
            }
        }
        // TODO log::info!("Done downloading {}, downloaded {}", file_path.display(), downloaded);
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .fuse(); // also needs to be fused

    pin_mut!(conn_fut, body_fut);

    select! {
        res = body_fut => res,
        _ = conn_fut => Err("connection closed before download finished".into()),
    }
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

use futures::join;

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
