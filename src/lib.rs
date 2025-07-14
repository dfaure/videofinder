// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fs::File;

mod simple_log;
mod download;
//use crate::download::download_db;

slint::include_modules!();

use rusqlite::{Connection, OpenFlags, Result};

#[derive(Debug)]
struct SearchResult {
    serie_name: String,
    name: String,
    film_type: i32,
    tape_type: i32,
    season: i32,
    episode: i32,
    origin: String,
    on_loan: bool,
    code_tape: i32,
    code_film: i32,
    title: String,
    path: String
}

fn sqlite_search(text : String) -> Result<()> {


    let conn = Connection::open_with_flags(download::db_full_path(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    // Prepend/append '%'
    let pattern = format!("%{}%", text);
    log!("  pattern={:?}", pattern);

    let mut stmt = conn.prepare("SELECT Film.SERIE_NAME, Film.NAME, Film.TYPE, Tape.type, Film.SEASON, Film.EPISODE_NR, \
          Tape.ORIGIN, Tape.ON_LOAN, Tape.code_tape, Film.code, Tape.TITLE, Tape.PATH \
         FROM Tape LEFT JOIN (TapeFilm JOIN Film ON TapeFilm.code_film=Film.code) TapeFilm ON TapeFilm.code_tape=Tape.code_tape \
         WHERE ( \
           Tape.TITLE LIKE ?1 \
           OR Film.SERIE_NAME LIKE ?2 \
           OR Film.NAME LIKE ?3 \
           OR Film.DIRECTOR LIKE ?4 \
           OR Film.PRODUCER LIKE ?5 \
           OR Film.COMPOSER LIKE ?6 \
           OR Film.CODE IN (select CODE_FILM from Actor where ACTOR LIKE ?7) \
         ) \
         ORDER BY Film.SERIE_NAME, Film.NAME")?;

    log!("prepared, now running");

    let iter = stmt.query_map([&pattern, &pattern, &pattern, &pattern, &pattern, &pattern, &pattern], |row| {
        Ok(SearchResult {
            serie_name: row.get(0)?,
            name: row.get(1)?,
            film_type: row.get(2)?,
            tape_type: row.get(3)?,
            season: row.get(4)?,
            episode: row.get(5)?,
            origin: row.get(6)?,
            on_loan: row.get(7)?,
            code_tape: row.get(8)?,
            code_film: row.get(9)?,
            title: row.get(10)?,
            path: row.get(11)?,
        })
    })?;

    log!("Done running");

    for search_result in iter {
        log!("Search result {:?}", search_result.unwrap());
    }

    Ok(())
}

// Assuming you have an async runtime set up for Slint
//use slint::ComponentHandle; // You might need this or similar for your Slint setup


#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) -> Result<(), Box<dyn Error>> {

    log!("videofinder started");
    slint::android::init(app).unwrap();

    log!("slint::android initialized");

    let ui = AppWindow::new()?;

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
                log!("File::open failed: {}", e);
                let error_msg = format!("DB exists but cannot be opened: {}", e);
                ui.set_status(error_msg.into());
            }
        }
    }

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

                // log!("download initiated"); // Changed log to info, as "donwload" was a typo
                //download_db().await;
                // log!("download finished");

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
            log!("searching for {:?}", text);
            match sqlite_search(text.to_string()) {
                Ok(_) => {
                    log!("TODO: display results");
                    ui.set_search_error("TODO display results".to_owned().into());
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
