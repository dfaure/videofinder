use anyhow::Context;
use rusqlite::Connection;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// One Tape row as produced by scripts/scan_hdd.py. CODE_TAPE is assigned
/// at merge time, not by the scanner.
#[derive(Deserialize)]
struct TapeRow {
    path: String,
    title: String,
    location: String,
    shelf: i32,
    row: i32,
    position: i32,
    #[serde(rename = "type")]
    type_: i32,
    date_purchase: String,
    duration: i32,
}

/// Produce `merged_db` by copying the Qt-curated `qt_db` and appending the
/// HDD Tape rows from each JSONL file. Missing JSONL files are warned about
/// and skipped, so a partial source set still yields a usable DB.
pub fn merge(
    qt_db: &Path,
    jsonl_paths: &[std::path::PathBuf],
    merged_db: &Path,
) -> Result<(), anyhow::Error> {
    log::info!(
        "Merging {} into {} (+ {} HDD slice(s))",
        qt_db.display(),
        merged_db.display(),
        jsonl_paths.len()
    );

    // Remove any stale merged DB before copying, so a half-written previous
    // run can't leak across.
    if merged_db.exists() {
        fs::remove_file(merged_db).context("removing stale merged DB")?;
    }
    fs::copy(qt_db, merged_db).context("copying Qt DB to merged path")?;

    let mut conn = Connection::open(merged_db).context("opening merged DB")?;

    let mut next_code: i32 = conn
        .query_row("SELECT COALESCE(MAX(CODE_TAPE), 0) + 1 FROM Tape", [], |row| row.get(0))
        .context("reading max CODE_TAPE")?;

    let tx = conn.transaction().context("starting transaction")?;
    let mut inserted: usize = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO Tape \
               (CODE_TAPE, TITLE, LOCATION, SHELF, ROW, POSITION, PATH, TYPE, DATE_PURCHASE, DURATION) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;

        for jsonl_path in jsonl_paths {
            if !jsonl_path.exists() {
                log::warn!("JSONL file missing, skipping: {}", jsonl_path.display());
                continue;
            }
            let f = File::open(jsonl_path)
                .with_context(|| format!("opening {}", jsonl_path.display()))?;
            let reader = BufReader::new(f);
            let mut count_in_file: usize = 0;
            for (line_no, line) in reader.lines().enumerate() {
                let line = line.with_context(|| {
                    format!("reading line {} of {}", line_no + 1, jsonl_path.display())
                })?;
                if line.trim().is_empty() {
                    continue;
                }
                let row: TapeRow = serde_json::from_str(&line).with_context(|| {
                    format!("parsing line {} of {}", line_no + 1, jsonl_path.display())
                })?;
                stmt.execute(rusqlite::params![
                    next_code,
                    &row.title,
                    &row.location,
                    row.shelf,
                    row.row,
                    row.position,
                    &row.path,
                    row.type_,
                    &row.date_purchase,
                    row.duration,
                ])?;
                next_code += 1;
                count_in_file += 1;
            }
            log::info!("Merged {} rows from {}", count_in_file, jsonl_path.display());
            inserted += count_in_file;
        }
    }
    tx.commit().context("committing transaction")?;

    log::info!("Merge complete: {} HDD rows inserted into {}", inserted, merged_db.display());
    Ok(())
}
