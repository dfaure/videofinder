// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fs::File;
use std::rc::Rc; // "reference counted"

mod download;
mod enums;

use crate::enums::SupportType;
use crate::enums::FilmType;
//use crate::download::download_db;
use slint::VecModel;

// Include the slint-generated code
slint::include_modules!();

use rusqlite::{Connection, OpenFlags, Result};

// do not use unwrap in this code, let errors propagate up to the UI
// ResultItemData is a GUI type, defined in the slint code
// Using this here is a bit arguable in terms of core/ui separation,
// but avoids conversions & code duplication.
fn sqlite_search(text : String) -> Result<Vec<ResultItemData>> {

    let conn = Connection::open_with_flags(download::db_full_path(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    // Prepend/append '%'
    let pattern = format!("%{}%", text);
    log::debug!("  pattern={:?}", pattern);

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

    log::debug!("prepared, now running");

    let iter = stmt.query_map([&pattern, &pattern, &pattern, &pattern, &pattern, &pattern, &pattern], |row| {
        let serie_name = row.get::<_, Option<String>>(0)?;
        log::debug!("serie_name: {:?}", serie_name);
        let name = row.get::<_, Option<String>>(1)?;
        log::debug!("name: {:?}", name);
        let title = row.get::<_, String>(10)?;
        log::debug!("title: {:?}", title);
        let film_type = row.get::<_, Option<i32>>(2)?;
        log::debug!("film_type: {:?}", film_type);
        let support_type = row.get::<_, i32>(3)?;
        log::debug!("support_type: {:?}", support_type);

        /*
        let origin = row.get(6)?;
        let on_loan = row.get(7)?;
        let code_tape = row.get(8)?;
        let code_film = row.get(9)?;
        let path = row.get(11)?;
        */

        let film_name = {
            if support_type == SupportType::COMPUTERFILE as i32 {
                title
            } else if film_type == Some(FilmType::TELEVISION as i32) {
                let mut film_name : String;
                if let (Some(serie), Some(n)) = (&serie_name, &name) {
                    // Inside this block, 'serie' and 'n' are &String (references to String)
                    // You can dereference them (*serie, *n) or use .clone() if you need owned String
                   film_name = format!("{} -- {}", serie, n);
                } else {
                   film_name = name.unwrap_or_else(|| String::new());
                }
                let season : i32 = row.get::<_, i32>(4)?;
                let episode : i32 = row.get::<_, i32>(5)?;
                let episode_number = season * 100 + episode;
                film_name = format!("{} ({})", film_name, episode_number);
                film_name
            } else { // Film
                name.unwrap_or(String::new())
            }
        };

        Ok(
            ResultItemData {
                film_name: film_name.into(),
                support_type: support_type,
            })
    })?;

    log::debug!("Done running");

    let mut results : Vec<ResultItemData> = vec![];
    for search_result in iter {
        //log::debug!("Search result {:?}", search_result?);
        results.push(search_result?);
    }

    Ok(results)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) -> Result<(), Box<dyn Error>> {

    // Log to file, on Android
    flexi_logger::Logger::with(LevelFilter::Debug)?
    .log_to_file(FileSpec::try_from("/sdcard/Download/videofinder_logs.txt"))         // write logs to file
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
            log::debug!("searching for {:?}", text);
            match sqlite_search(text.to_string()) {
                Ok(results) => {
                    log::debug!("displaying {} results", results.len());
                    let model: Rc<VecModel<ResultItemData>> = Rc::new(VecModel::from(results));
                    ui.set_result_items(model.clone().into());
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
