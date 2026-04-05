// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{DateTime, Local};
use std::cell::RefCell;
use std::error::Error;
use std::fs::File;
use std::rc::Rc;
use std::time::Instant;

mod download;
mod enums;
mod image_handling;
mod sqlsearch;

use crate::download::ImageForDirHash;
use crate::download::download_db;
use crate::download::parse_file_list;
use crate::image_handling::download_image;
use crate::image_handling::image_url;
use crate::sqlsearch::sqlite_get_record;
use crate::sqlsearch::sqlite_search;
use slint::VecModel;

// Include the slint-generated code
slint::include_modules!();

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) -> Result<(), Box<dyn Error>> {
    // Log to file, on Android
    flexi_logger::Logger::try_with_env_or_str("debug,android_activity::activity_impl::glue=off")?
        .log_to_file(flexi_logger::FileSpec::try_from(
            "/storage/emulated/0/Download/videofinder_log.txt",
        )?)
        .format(flexi_logger::detailed_format)
        .start()?;

    log::info!("videofinder started");
    slint::android::init(app).unwrap();
    log::debug!("slint::android initialized");
    let ret = videofinder_main();
    if let Err(ref e) = ret {
        log::error!("{:?}", e);
    }
    // When we get here, exit process so Android restarts fresh next time
    std::process::exit(0);
}

fn show_db_status(ui: &AppWindow, image_for_dir_hash: &Rc<RefCell<ImageForDirHash>>) {
    let db_full_path = download::db_full_path();
    if !db_full_path.exists() {
        // Not an error, if it's a first time user. Just let them download it.
        let status = "No DB, click here to download:";
        ui.set_status(status.into());
    } else {
        // Check if readable, to debug permission problems on Android
        match File::open(db_full_path) {
            Ok(file) => {
                match file.metadata() {
                    Ok(metadata) => {
                        if let Ok(modified) = metadata.modified() {
                            let datetime: DateTime<Local> = modified.into();
                            let time_str = format!(
                                "DB last updated: {}",
                                datetime.format("%d/%m/%Y %H:%M:%S")
                            );
                            ui.set_status(time_str.into());

                            if let Ok(hash) = parse_file_list() {
                                *image_for_dir_hash.borrow_mut() = hash;
                            }
                            return;
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to get metadata: {}", e);
                    }
                }
                ui.set_status("DB last updated: unknown".into());
            }
            Err(e) => {
                log::error!("File::open failed: {}", e);
                let error_msg = format!("DB exists but cannot be opened: {}", e);
                ui.set_status(error_msg.into());
            }
        }
    }
}

fn open_details_window(
    ui: &AppWindow,
    film_code: i32,
    support_code: i32,
    image_for_dir_hash: &ImageForDirHash,
    current_image_download_url: &Rc<RefCell<Option<String>>>,
) -> String {
    ui.set_details_error("".into());
    ui.set_details_image(slint::Image::default());
    *current_image_download_url.borrow_mut() = None;
    log::info!("item clicked film {} support {}", film_code, support_code);
    match sqlite_get_record(film_code, support_code) {
        Ok((record, image_path)) => {
            ui.set_details_record(record);
            image_url(image_path, image_for_dir_hash)
        }
        Err(e) => {
            let error_msg = format!("Error: {}", e);
            log::warn!("{}", error_msg);
            ui.set_details_error(error_msg.into());
            String::new()
        }
    }
}

pub fn videofinder_main() -> Result<(), Box<dyn Error>> {
    std::panic::set_hook(Box::new(|info| {
        log::error!("Panic occurred: {}", info);
    }));

    let ui = AppWindow::new()?;
    let image_for_dir_hash: Rc<RefCell<ImageForDirHash>> =
        Rc::new(RefCell::new(ImageForDirHash::new()));
    let current_image_download_url: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // Show initial status and fill in image_for_dir_hash if the file is already present
    show_db_status(&ui, &image_for_dir_hash);

    ui.on_search({
        let ui_handle = ui.as_weak();
        move |text| {
            log::info!("searching for {:?}", text);
            let ui = ui_handle.unwrap();
            ui.set_search_error("".into());
            let start_time_sql = Instant::now();
            match sqlite_search(text.to_string()) {
                Ok(results) => {
                    log::info!("SQL search: {:?}", start_time_sql.elapsed());
                    log::info!("displaying {} results", results.len());
                    if results.is_empty() {
                        ui.set_search_error("No results found".into());
                    }
                    let start_time_vec = Instant::now();
                    let model: Rc<VecModel<ResultItemData>> = Rc::new(VecModel::from(results));
                    log::info!("creating VecModel: {:?}", start_time_vec.elapsed());
                    let start_time_set = Instant::now();
                    ui.set_result_items(model.clone().into());
                    log::info!("set_result_items: {:?}", start_time_set.elapsed());
                }
                Err(e) => {
                    let error_msg = format!("Error: {}", e);
                    log::warn!("{}", error_msg);
                    ui.set_search_error(error_msg.into());
                }
            }
        }
    });

    ui.on_item_clicked({
        let ui_handle = ui.as_weak();
        let image_for_dir_hash = image_for_dir_hash.clone();
        let current_image_download_url = current_image_download_url.clone();
        move |film_code, support_code| {
            let ui = ui_handle.unwrap();
            let image_url = open_details_window(
                &ui,
                film_code,
                support_code,
                &image_for_dir_hash.borrow(),
                &current_image_download_url,
            );
            if !image_url.is_empty() {
                download_image(&ui_handle, &current_image_download_url, image_url);
            }
        }
    });

    ui.on_notify_details_window_closed({
        let current_image_download_url = current_image_download_url.clone();
        move || {
            log::debug!("cancel_image_downloads");
            *current_image_download_url.borrow_mut() = None;
        }
    });

    ui.on_download_db({
        let ui_handle = ui.as_weak();
        let image_for_dir_hash = image_for_dir_hash.clone();

        move || {
            let ui = ui_handle.unwrap();
            ui.set_status("Downloading...".into());
            let ui_handle = ui_handle.clone();
            let ui_handle_for_progress = ui_handle.clone();
            let image_for_dir_hash = image_for_dir_hash.clone();
            let progress_func = Box::new(move |progress: f32| {
                ui_handle_for_progress.unwrap().set_progress(progress);
            });
            log::info!("on_download_db");
            if let Err(e) = slint::spawn_local(async_compat::Compat::new(async move {
                let ui = ui_handle.unwrap();
                ui.set_download_enabled(false); // prevent re-entrancy
                let result = download_db(progress_func).await;
                if let Err(e) = result {
                    log::warn!("Download error: {e}");
                    ui.set_status(format!("Download error: {}", e).into());
                } else {
                    log::debug!("Download complete");
                    ui.set_status("Download complete".into());
                    show_db_status(&ui, &image_for_dir_hash);
                }
                ui.set_download_enabled(true);
            })) {
                log::error!("Failed to schedule download: {e}");
            }
        }
    });

    log::debug!("calling run");
    ui.run()?;
    Ok(())
}
