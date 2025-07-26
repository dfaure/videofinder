// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fs::File;
use std::rc::Rc; // "reference counted"
use std::time::Instant;

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
    test_main()
    //videofinder_main()
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

use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::fs;

pub fn test_main() -> Result<(), Box<dyn Error>> {
    log::info!("test_main: starting");
    let slint_future = async move {
        log::info!("test_main (future): running");
        if let Err(e) = download_db().await {
            log::warn!("Error: {e}");
        } else {
            log::debug!("Download complete");
        }
        log::info!("test_main (future): calling quit_event_loop");
        slint::quit_event_loop().unwrap();
    };
    log::info!("test_main: future created");

    // Spawn the future on Slint's event loop
    slint::spawn_local(async_compat::Compat::new(slint_future))?;
    log::info!("test_main: after spawn_local");

    // Start the event loop
    slint::run_event_loop_until_quit()?;

    log::info!("test_main: done");
    Ok(())
}

pub fn socket_test_main() -> Result<(), Box<dyn Error>> {

    let mut socket_path: PathBuf = std::env::temp_dir();
    socket_path.push("my_app_socket");
    log::info!("socket_test_main: {}", socket_path.display());

    // Clean up any existing socket file
    let _ = fs::remove_file(&socket_path);

    // Set up a Unix domain socket listener
    let listener = UnixListener::bind(&socket_path).unwrap();
    log::info!("socket_test_main: listener");

    let socket_path_clone = socket_path.clone();

    // Spawn the server thread
    let server = std::thread::spawn(move || {
        let mut stream = listener.incoming().next().unwrap().unwrap();
        stream.write_all(b"Hello World").unwrap();
    });
    log::info!("socket_test_main: server");

    // The future that connects and reads using Tokio
    let slint_future = async move {
        use tokio::io::AsyncReadExt;
        use tokio::net::UnixStream;

        log::info!("socket_test_main (future): starting");
        let mut stream = UnixStream::connect(&socket_path_clone).await.unwrap();
        log::info!("socket_test_main (future): connected");
        let mut data = Vec::new();
        stream.read_to_end(&mut data).await.unwrap();

        assert_eq!(data, b"Hello World");
        log::info!("socket_test_main (future): got data, calling quit_event_loop");
        slint::quit_event_loop().unwrap();
    };

    // Spawn the future on Slint's event loop
    slint::spawn_local(async_compat::Compat::new(slint_future))?;
    log::info!("socket_test_main: after spawn_local");

    // Start the event loop
    slint::run_event_loop_until_quit()?;

    log::info!("socket_test_main: after run_event_loop_until_quit");
    // Wait for the server thread to finish
    if let Err(e) = server.join() {
        log::warn!("socket_test_main: ERROR {:?}", e);
    }
    log::info!("socket_test_main: after server.join()");

    // Clean up the socket file
    let remove_result = fs::remove_file(&socket_path);
    log::info!("socket_test_main: remove_file returned {:?}", remove_result);

    Ok(())
}

pub fn videofinder_main() -> Result<(), Box<dyn Error>> {

    let ui = AppWindow::new()?;

    log::info!("1");
    show_db_status(&ui);
    log::info!("2");

    //let download_ui_handle = ui.as_weak();
    //log::info!("3");

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
    log::info!("4");

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

    
    let download_handler = move || {
        log::info!("on_download_db");
        if let Err(e) = slint::spawn_local(async_compat::Compat::new(async move {
            if let Err(e) = download_db().await {
                log::warn!("Error: {e}");
            } else {
                log::debug!("Download complete");
            }
        })) {
            log::error!("Failed to schedule download: {e}");
        }
    };
    ui.on_download_db(download_handler);

    /*
    ui.on_download_db({
        let ui_handle = ui.as_weak();
        move || {
            let ui_handle = ui_handle.clone();
            log::info!("on_download_db");

            if let Err(e) = slint::spawn_local(async_compat::Compat::new(async move {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_status("Downloading...".into());
                    log::debug!("downloading...");
                    if let Err(e) = download_db().await {
                        log::warn!("Error: {e}");
                        ui.set_status(format!("Error: {e}").into());
                    } else {
                        log::debug!("Download complete");
                        ui.set_status("Download complete".into());
                    }
                } else {
                    log::warn!("UI handle invalid");
                }
            })) {
                log::error!("Failed to schedule download: {e}");
            }
        }
    });*/

    let ui_handle = ui.as_weak();

    slint::Timer::default().start(
        slint::TimerMode::SingleShot,
        std::time::Duration::from_millis(10),
        move || {
            if let Some(ui) = ui_handle.upgrade() {
                ui.on_download_db({
                    move || {
                        log::info!("on_download_db");
                        if let Err(e) = slint::spawn_local(::async_compat::Compat::new(async move {
                            if let Err(e) = download_db().await {
                                log::warn!("Download error: {e}");
                            } else {
                                log::debug!("Download complete");
                            }
                        })) {
                            log::error!("Failed to schedule download: {e}");
                        }
                    }
                });
            }
        },
        );

    log::info!("before run");
    ui.run()?;

    log::info!("after run");
    //if let Some(ui) = ui_handle.upgrade() {
        //log::info!("setting up on_download_db");
        //ui.on_download_db(download_handler);
    //}

            /*
    download_ui_handle.upgrade().unwrap().on_download_db(
        move || {
            log::info!("downloading...");
            let ui_handle = download_ui_handle.clone();

            // Code from https://github.com/slint-ui/slint/issues/2793
            // Spawn a Tokio task
            //(tokio::runtime::Handle::current()).spawn(async move {
            let result = slint::spawn_local(async move {

                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_status("Downloading...".into());

                    if let Err(e) = download_db().await {
                        ui.set_status(format!("Error: {e}").into());
                    } else {
                        ui.set_status("Download complete".into());
                    }
                } else {
                    log::warn!("UI handle is invalid");
                }
            });

            if let Err(e) = result {
                log::warn!("Failed to run spawn_local: {e}");
            }
        }
    );*/

    Ok(())

}
