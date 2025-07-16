// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fs::File;
use std::rc::Rc; // "reference counted"
use std::time::Instant;

mod download;
mod enums;

use crate::enums::SupportType;
use crate::enums::FilmType;
//use crate::download::download_db;
use slint::VecModel;

// Include the slint-generated code
slint::include_modules!();

//////////////////// SEARCH SUPPORT /////////////

use rusqlite::{Connection, OpenFlags};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult};

impl FromSql for SupportType {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_i64() {
            Ok(1) => Ok(SupportType::TAPE),
            Ok(2) => Ok(SupportType::DVD),
            Ok(4) => Ok(SupportType::COMPUTERFILE),
            Ok(8) => Ok(SupportType::BLURAY),
            // Handle any other i32 value that doesn't correspond to a variant.
            Ok(_) => Err(FromSqlError::InvalidType),
            Err(e) => Err(e)
        }
    }
}

// do not use unwrap in this code, let errors propagate up to the UI
// ResultItemData is a GUI type, defined in the slint code
// Using this here is a bit arguable in terms of core/ui separation,
// but avoids conversions & code duplication.
fn sqlite_search(text : String) -> rusqlite::Result<Vec<ResultItemData>> {

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
        //log::debug!("serie_name: {:?}", serie_name);
        let name = row.get::<_, Option<String>>(1)?;
        //log::debug!("name: {:?}", name);
        let title = row.get::<_, String>(10)?;
        //log::debug!("title: {:?}", title);
        let film_type = row.get::<_, Option<i32>>(2)?;
        //log::debug!("film_type: {:?}", film_type);
        let support_type = row.get::<_, SupportType>(3)?;
        /*
        let support_type : SupportType = support_type_raw.try_into()
        .map_err(|e| {
                // Convert your custom &'static str error into a String,
                // and then box it into a dynamic error trait object,
                // which rusqlite::Error::FromSql::Other expects.
                RusqliteError::FromSql(
                    rusqlite::types::FromSqlError::Other(
                        Box::new(CustomConversionError(e.to_string()))
                    )
                )
        });
        */
        //log::debug!("support_type: {:?}", support_type);

        let origin = row.get::<_, String>(6).unwrap_or(String::new());
        let on_loan = row.get::<_, bool>(7).unwrap_or(false);
        /*
        let code_tape = row.get(8)?;
        let code_film = row.get(9)?;
        let path = row.get(11)?;
        */

        let film_name = {
            if support_type == SupportType::COMPUTERFILE {
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
                let maybe_season = row.get::<_, Option<i32>>(4).unwrap_or(None); // some are String("")
                let maybe_episode = row.get::<_, Option<i32>>(5).unwrap_or(None);
                if let (Some(season), Some(episode)) = (maybe_season, maybe_episode) {
                    let episode_number = season * 100 + episode;
                    film_name = format!("{} ({})", film_name, episode_number);
                }
                film_name
            } else if name.is_some() { // Film
                name.unwrap()
            } else { // Tape without a film
                title
            }
        };

        Ok(
            ResultItemData {
                film_name: film_name.into(),
                support_color: crate::enums::color_for_support(support_type, origin, on_loan),
                support_type_text: crate::enums::letter_for_support_type(support_type).into(),
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
