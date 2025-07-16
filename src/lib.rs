// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fs::File;
use std::rc::Rc; // "reference counted"
use std::time::Instant;

mod download;
mod enums;
mod sqlsearch;

//use crate::download::download_db;
use slint::VecModel;
use crate::sqlsearch::sqlite_search;

// Include the slint-generated code
slint::include_modules!();

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) -> Result<(), Box<dyn Error>> {

    // Log to file, on Android
    flexi_logger::Logger::with(flexi_logger::LevelFilter::Info)
    .log_to_file(flexi_logger::FileSpec::try_from("/storage/emulated/0/Download/videofinder_logs.txt")?)
    .start()?;

    log::info!("videofinder started");
    slint::android::init(app).unwrap();
    log::debug!("slint::android initialized");
    videofinder_main()
}

fn show_db_status(ui: &AppWindow) {
    let db_full_path = download::db_full_path();
    if !db_full_path.exists() {
        let status = format!("DB file does not exist: {}", db_full_path.display());
        ui.set_status(status.into());
    } else {
        // Check if readable, to debug permission problems on Android
        match File::open(db_full_path) {
            Ok(_) => {
                ui.set_status("DB last updated: never".into());
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

    ui.on_download_db({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            ui.set_status("Downloading...".into());
        }

        // Spawn a new thread
        /*
	std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {

                // log::debug!("download initiated");
                //download_db().await;
                // log::debug!("download finished");

                // You might want to update the UI after the download completes
                // For example, if you have a `download_complete` callback on your UI
                // ui_handle.upgrade_in_event_loop(move |ui| {
                //     ui.set_download_status("Completed");
                // });
            });
        });*/
    });

    ui.on_search({
        let ui_handle = ui.as_weak();
        move |text| {
            let ui = ui_handle.unwrap();
            log::info!("searching for {:?}", text);
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
                    ui.set_search_error(error_msg.into());
                }
            }
        }
    });

    ui.run()?;

    Ok(())

}
