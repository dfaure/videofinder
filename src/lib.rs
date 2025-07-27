// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fs::File;
use std::rc::Rc; // "reference counted"
use std::time::Instant;
use chrono::{DateTime, Local};

mod download;
mod enums;
mod sqlsearch;

use crate::download::download_db;
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
    log::debug!("test_main returned");
    if let Err(ref e) = ret {
        log::error!("{:?}", e);
    }
    ret
}

fn show_db_status(ui: &AppWindow) {
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

    let ui = AppWindow::new()?;

    show_db_status(&ui);

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
        move |film_code, support_code| {
            let ui = ui_handle.unwrap();
            ui.set_details_error("".into());
            log::info!("item clicked film {} support {}", film_code, support_code);
            match sqlite_get_record(film_code, support_code) {
                Ok(record) => {
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
        let ui_handle = ui.as_weak();
        move || {
            let ui_handle = ui_handle.clone();
            log::info!("on_download_db");
            if let Err(e) = slint::spawn_local(async_compat::Compat::new(async move {
                let ui = ui_handle.unwrap();
                if let Err(e) = download_db().await {
                    log::warn!("Download error: {e}");
                    ui.set_status(format!("Download error: {}", e).into());
                } else {
                    log::debug!("Download complete");
                    ui.set_status("Download complete".into());
                    show_db_status(&ui);
                }
            })) {
                log::error!("Failed to schedule download: {e}");
            }
        }
    });

    ui.run()?;
    Ok(())
}
