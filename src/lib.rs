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

struct App {
    ui: AppWindow,
    image_for_dir_hash: ImageForDirHash,
    current_image_download_url: Option<String>,
}

impl App {
fn show_db_status(&mut self) {

    let ui = &self.ui;
    let image_for_dir_hash = &mut self.image_for_dir_hash;

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

fn on_download_finished(&mut self, result: Result<(), Box<dyn Error>>) {
    if let Err(e) = result {
        log::warn!("Download error: {e}");
        self.ui.set_status(format!("Download error: {}", e).into());
    } else {
        log::debug!("Download complete");
        self.ui.set_status("Download complete".into());
        self.show_db_status();
    }
}

fn open_details_window(&mut self, film_code: i32, support_code: i32) {
    self.ui.set_details_error("".into());
    self.cancel_image_downloads();
    log::info!("item clicked film {} support {}", film_code, support_code);
    match sqlite_get_record(film_code, support_code) {
        Ok((record, image_path)) => {
            self.ui.set_details_record(record);
            let image_url = image_url(image_path, &self.image_for_dir_hash);
            if !image_url.is_empty() {
                self.download_image(image_url);
            }
        },
        Err(e) => {
            let error_msg = format!("Error: {}", e);
            log::warn!("{}", error_msg);
            self.ui.set_details_error(error_msg.into());
        }
    }
}
} // impl

// This needs a ref to Rc, so it's not an impl for App (self wouldn't be a Rc)
fn setup_ui(app: &Rc<RefCell<App>>) {
    let app_ref = app.borrow();
    app_ref.ui.on_search({
        let ui_handle = app_ref.ui.as_weak();
        move |text| {
            log::info!("searching for {:?}", text);
            let ui = ui_handle.unwrap();
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

    app_ref.ui.on_item_clicked({
        // executed immediately
        let app_rc = app.clone(); // clone the Rc
        move |film_code, support_code| {
            // executed on click
            app_rc.borrow_mut().open_details_window(film_code, support_code);

        }
    });

    app_ref.ui.on_notify_details_window_closed({
        // executed immediately
        let app_rc = app.clone(); // clone the Rc
        move || {
            // executed when closing the details window
            app_rc.borrow_mut().cancel_image_downloads();
        }
    });

    app_ref.ui.on_download_db({
        // executed immediately
        let app_rc = app.clone(); // clone the Rc

        move || {
            // executed on click
            app_rc.borrow().ui.set_status("Downloading...".into());
            // local vars for move-captures
            let app_rc = app_rc.clone();
            let app_rc_for_progress = app_rc.clone();
            let progress_func = Box::new(move |progress: f32| {
                app_rc_for_progress.borrow().ui.set_progress(progress);
            });
            log::info!("on_download_db");
            if let Err(e) = slint::spawn_local(async_compat::Compat::new(async move {
                app_rc.borrow().ui.set_download_enabled(false); // prevent re-entrancy
                let result = download_db(progress_func).await;
                app_rc.borrow_mut().on_download_finished(result);
                app_rc.borrow().ui.set_download_enabled(true);
            })) {
                log::error!("Failed to schedule download: {e}");
            }
        }
    });
}

pub fn videofinder_main() -> Result<(), Box<dyn Error>> {

    std::panic::set_hook(Box::new(|info| {
        log::error!("Panic occurred: {}", info);
    }));

    // Use Rc<RefCell>> because we're modifying image_for_dir_hash from the async task (see spawn_local)
    let app = Rc::new(RefCell::new(App {
        ui: AppWindow::new()?,
        image_for_dir_hash: ImageForDirHash::new(),
        current_image_download_url: None,
    }));

    // Show initial status and fill in image_for_dir_hash if the file is already present
    app.borrow_mut().show_db_status();

    // The setup_ui function provides a scope for app_ref (to avoid writing app.borrow() 50 times)
    setup_ui(&app);

    // Grab a ref to UI using a temporary app.borrow (which MUST be released before calling run())
    let ui_ref = app.borrow().ui.as_weak();
    log::debug!("calling run");
    ui_ref.unwrap().run()?;
    Ok(())
}
