use crate::AppWindow;
use crate::download::download_image_data;
use anyhow::anyhow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

fn relative_path(path: &str) -> anyhow::Result<&str> {
    let prefixes = [
        "/d/home/sabine/Films/",
        "/home/sabine/Films/",
        "/mnt/big/video/Films/",
        "/d/more/src/perso/Films/",
    ];
    for prefix in prefixes {
        if let Some(stripped) = path.strip_prefix(prefix) {
            log::debug!(r#"Stripped "{}" to "{}""#, path, stripped);
            return Ok(stripped);
        }
    }
    log::warn!("Unknown prefix in image path {}", path);
    Err(anyhow!("unknown prefix"))
}

pub fn image_url(
    maybe_image_path: Option<String>,
    hash: &crate::download::ImageForDirHash,
) -> String {
    log::debug!("image_url({:?})", maybe_image_path);
    maybe_image_path
        .as_deref() // Convert Option<String> to Option<&str> without moving
        .and_then(|path| relative_path(path).ok()) // Try to get relative path
        .and_then(|relative| {
            hash.get(&PathBuf::from(relative)).map(|file_name| {
                format!(
                    "http://www.davidfaure.fr/kvideomanager/Films/{}/{}",
                    relative,
                    file_name.display()
                )
            })
        })
        .unwrap_or_default() // If anything fails, return empty string
}

pub fn download_image(
    ui_handle: &slint::Weak<AppWindow>,
    current_image_download_url: &Rc<RefCell<Option<String>>>,
    url: String,
) {
    log::debug!("download_image({})", url);
    *current_image_download_url.borrow_mut() = Some(url.clone());
    let ui_handle = ui_handle.clone();
    let current_image_download_url = current_image_download_url.clone();
    if let Err(e) = slint::spawn_local(async_compat::Compat::new(async move {
        let result = download_image_data(&url).await;
        match result {
            Ok(image) => {
                if Some(url) == *current_image_download_url.borrow() {
                    // if still relevant...
                    log::debug!("on_image_downloaded");
                    ui_handle.unwrap().set_details_image(image);
                }
            }
            Err(e) => {
                log::error!("Failed to download image: {e}");
            }
        }
    })) {
        log::error!("Failed to schedule download: {e}");
    }
}
