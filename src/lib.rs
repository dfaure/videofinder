// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fs::File;
use std::rc::Rc; // "reference counted"
use std::cell::RefCell;
use std::time::Instant;
use chrono::{DateTime, Local};

mod download;
mod enums;
mod sqlsearch;
mod image_handling;

use crate::download::download_db;
use crate::download::parse_file_list;
use crate::download::ImageForDirHash;
use crate::image_handling::image_url;
use slint::VecModel;
use crate::sqlsearch::sqlite_search;
use crate::sqlsearch::sqlite_get_record;

// Include the slint-generated code
slint::include_modules!();

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) -> Result<(), Box<dyn Error>> {

    // Log to file, on Android
    flexi_logger::Logger::try_with_env_or_str("debug,android_activity::activity_impl::glue=off")?
    .log_to_file(flexi_logger::FileSpec::try_from("/storage/emulated/0/Download/videofinder_logs.txt")?)
    .format(flexi_logger::detailed_format)
    .start()?;

    log::info!("videofinder started");
    slint::android::init(app).unwrap();
    log::debug!("slint::android initialized");
    let ret = videofinder_main();
    if let Err(ref e) = ret {
        log::error!("{:?}", e);
    }
    ret
}

fn show_db_status(ui: &AppWindow, image_for_dir_hash: &mut ImageForDirHash) {
    let db_full_path = download::db_full_path();
    if !db_full_path.exists() {
        let status = format!("DB file does not exist: {}", db_full_path.display());
        ui.set_status(status.into());
    } else {
        // Check if readable, to debug permission problems on Android
        match File::open(db_full_path) {
            Ok(file) => {
                match file.metadata() {
                    Ok(metadata) => {
                        if let Ok(modified) = metadata.modified() {
                            let datetime: DateTime<Local> = modified.into();
                            let time_str = format!("DB last updated: {}", datetime.format("%d/%m/%Y %H:%M:%S"));
                            ui.set_status(time_str.into());

                            if let Ok(hash) = parse_file_list() {
                                *image_for_dir_hash = hash;
                            }
                            return;
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to get metadata: {}", e);
                    }
                }
                ui.set_status("DB last updated: unknown".into());
            },
            Err(e) => {
                log::error!("File::open failed: {}", e);
                let error_msg = format!("DB exists but cannot be opened: {}", e);
                ui.set_status(error_msg.into());
            }
        }
    }
}

pub fn videofinder_main() -> Result<(), Box<dyn Error>> {

    std::panic::set_hook(Box::new(|info| {
        log::error!("Panic occurred: {}", info);
    }));

    let ui = AppWindow::new()?;

    // Can't do that because we're modifying it from the async task (see spawn_local)
    //let mut image_for_dir_hash = ImageForDirHash::new();
    let image_for_dir_hash = Rc::new(RefCell::new(ImageForDirHash::new()));

    show_db_status(&ui, &mut *image_for_dir_hash.borrow_mut());

    ui.on_search({
        let ui_handle = ui.as_weak();
        move |text| {
            let ui = ui_handle.unwrap();
            log::info!("searching for {:?}", text);
            ui.set_search_error("".into());
            let start_time_sql = Instant::now();
            match sqlite_search(text.to_string()) {
                Ok(results) => {
                    log::info!("SQL search: {:?}", start_time_sql.elapsed());
                    log::info!("displaying {} results", results.len());
                    let start_time_vec = Instant::now();
                    let model: Rc<VecModel<ResultItemData>> = Rc::new(VecModel::from(results));
                    log::info!("creating VecModel: {:?}", start_time_vec.elapsed());
                    let start_time_set = Instant::now();
                    ui.set_result_items(model.clone().into());
                    log::info!("set_result_items: {:?}", start_time_set.elapsed());
                },
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
        let hash_ref = image_for_dir_hash.clone(); // clone the Rc (not the hash)
        move |film_code, support_code| {
            let ui = ui_handle.unwrap();
            ui.set_details_error("".into());
            log::info!("item clicked film {} support {}", film_code, support_code);
            match sqlite_get_record(film_code, support_code) {
                Ok((mut record, image_path)) => {
                    record.image_url = image_url(image_path, &hash_ref.borrow()).into();
                    ui.set_details_record(record);
                },
                Err(e) => {
                    let error_msg = format!("Error: {}", e);
                    log::warn!("{}", error_msg);
                    ui.set_details_error(error_msg.into());
                }
            }
        }
    });

    ui.on_download_db({
        // executed immediately
        let ui_handle = ui.as_weak();
        let hash_ref = image_for_dir_hash.clone(); // clone the Rc (not the hash)

        move || {
            // executed on click
            ui_handle.unwrap().set_status("Downloading...".into());
            // local vars for move-captures
            let hash_ref = hash_ref.clone();
            let ui_handle = ui_handle.clone();
            let ui_handle_for_progress = ui_handle.clone();
            let progress_func = Box::new(move |progress: f32| {
                if let Some(ui) = ui_handle_for_progress.upgrade() {
                    ui.set_progress(progress);
                }
            });
            log::info!("on_download_db");
            if let Err(e) = slint::spawn_local(async_compat::Compat::new(async move {
                let ui = ui_handle.unwrap();
                if let Err(e) = download_db(progress_func).await {
                    log::warn!("Download error: {e}");
                    ui.set_status(format!("Download error: {}", e).into());
                } else {
                    log::debug!("Download complete");
                    ui.set_status("Download complete".into());
                    let mut hash = hash_ref.borrow_mut(); // mutable borrow from RefCell
                    show_db_status(&ui, &mut *hash);
                }
            })) {
                log::error!("Failed to schedule download: {e}");
            }
        }
    });

    ui.run()?;
    Ok(())
}
