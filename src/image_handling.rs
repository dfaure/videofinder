use anyhow::anyhow;
use std::path::PathBuf;

fn relative_path(path : &str) -> anyhow::Result<&str> {
    let prefixes = [
        "/d/home/sabine/Films/",
        "/home/sabine/Films/",
        "/mnt/big/video/Films/",
        "/d/more/src/perso/Films/",
    ];
    for prefix in prefixes {
        if let Some(stripped) = path.strip_prefix(prefix) {
            log::info!(r#"Stripped "{}" to "{}""#, path, stripped);
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
    log::info!("image_url({:?})", maybe_image_path);
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
