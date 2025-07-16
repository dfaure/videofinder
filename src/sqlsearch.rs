use crate::enums::SupportType;
use crate::enums::FilmType;
use crate::download;
use crate::ResultItemData;

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

