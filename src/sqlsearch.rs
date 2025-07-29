use crate::enums::SupportType;
use crate::enums::FilmType;
use crate::download;
use crate::ResultItemData;
use crate::RecordWrapper;
use std::rc::Rc;

use slint::VecModel;
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
pub fn sqlite_search(text : String) -> rusqlite::Result<Vec<ResultItemData>> {

    let conn = Connection::open_with_flags(download::db_full_path(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    // Prepend/append '%'
    let pattern = format!("%{}%", text);
    log::debug!("  pattern={:?}", pattern);

    let mut stmt = conn.prepare("SELECT Film.SERIE_NAME, Film.NAME, Film.TYPE, Tape.type, Film.SEASON, Film.EPISODE_NR, \
          Tape.ORIGIN, Tape.ON_LOAN, Tape.code_tape, Film.code, Tape.TITLE \
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
        //log::debug!("support_type: {:?}", support_type);

        let origin = row.get::<_, String>(6).unwrap_or(String::new());
        let on_loan = row.get::<_, bool>(7).unwrap_or(false);
        let support_code = row.get::<_, i32>(8).unwrap_or(0);
        let film_code = row.get::<_, i32>(9).unwrap_or(0);

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
                film_code: film_code,
                support_code: support_code,
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

pub fn sqlite_get_record(film_code : i32, support_code : i32) -> rusqlite::Result<(RecordWrapper, Option<String>)> {
    let conn = Connection::open_with_flags(download::db_full_path(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut support_query = conn.prepare("SELECT type, shelf, row, position, location, path FROM Tape WHERE Tape.code_tape=?1")?;
    log::info!("Doing support query for support code {}", support_code);
    let mut record_wrapper = support_query.query_row([support_code],
        |row| {
            //log::info!("Support row: {:?}", row);
            Ok(RecordWrapper {
                isComputerFile: row.get::<_, SupportType>(0)? == SupportType::COMPUTERFILE,
                shelf: row.get(1)?,
                row: row.get(2)?,
                position: row.get(3)?,
                location: row.get::<_, String>(4)?.into(),
                path: row.get::<_, String>(5).unwrap_or(String::new()).into(),
                // these will be set further below
                film_code: 0,
                duration: 0,
                year: 0,
                actors: [].into(),
                image_url: "".into(),
            })
            }
        )?;
    if record_wrapper.isComputerFile {
        return Ok((record_wrapper, None));
    }

    let mut image_path : Option<String> = None;
    if film_code != 0 {
        log::info!("Doing film query for film code {}", film_code);
        let mut film_query = conn.prepare("SELECT year, duration FROM Film WHERE Film.code=?1")?;
        film_query.query_row([film_code], |row| {
            //log::info!("Film row: {:?}", row);
            record_wrapper.year = row.get(0).unwrap_or(0);
            record_wrapper.duration = row.get(1).unwrap_or(0);
            record_wrapper.film_code = film_code;
            Ok(())
        })?;

        log::info!("Doing actor query for film code {}", film_code);
        let mut actor_query = conn.prepare("SELECT ACTOR FROM Actor WHERE code_film=?1")?;
        let iter = actor_query.query_map([film_code], |row| row.get::<_, String>(0))?;

        let mut actors : Vec<slint::SharedString> = vec![];
        for actor in iter {
            //log::debug!("Actor {:?}", actor?);
            actors.push(actor?.into());
        }
        let model: Rc<VecModel<slint::SharedString>> = Rc::new(VecModel::from(actors));
        record_wrapper.actors = model.into();

        log::info!("Doing image query for film code {}", film_code);
        let mut image_query = conn.prepare("SELECT N_IMAGE FROM Image WHERE code_film=?1")?;
        image_query.query_row([film_code], |row| {
            //log::debug!("Image row {:?}", row);
            image_path = Some(row.get(0)?);
            log::debug!("image_path: {:?}", image_path);
            Ok(())
        })?;
    }

    return Ok((record_wrapper, image_path));
}

