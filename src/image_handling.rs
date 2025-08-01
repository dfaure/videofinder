use anyhow::anyhow;
use std::path::PathBuf;
use crate::download::download_image_data;
use crate::Rc;
use crate::RefCell;
use crate::App;

fn relative_path(path : &str) -> anyhow::Result<&str> {
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
    return Err(anyhow!("unknown prefix"));
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
                    relative, file_name.display()
                )
            })
        })
        .unwrap_or_default() // If anything fails, return empty string
}

pub fn download_image(app_rc: &Rc<RefCell<App>>, url: String) {
    log::debug!("download_image({})", url);
    app_rc.borrow_mut().current_image_download_url = Some(url.clone());
    let app_rc = app_rc.clone();
    if let Err(e) = slint::spawn_local(async_compat::Compat::new(async move {
        let result = download_image_data(&url).await;
        match result {
            Ok(image) => {
                if Some(url) == app_rc.borrow().current_image_download_url {
                    // if still relevant...
                    app_rc.borrow_mut().on_image_downloaded(image);
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

impl crate::App {
    pub fn on_image_downloaded(&self, image: slint::Image) {
        log::debug!("on_image_downloaded");
        self.ui.set_details_image(image);
    }
    pub fn cancel_image_downloads(&mut self) {
        log::debug!("cancel_image_downloads");
        self.current_image_download_url = None;
    }
}
