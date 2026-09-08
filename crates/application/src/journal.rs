//! SQLite outbox + immutable JSON files. An execution never submits before its journal is flushed.
use chrono::{DateTime, FixedOffset, Utc};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::{fs, io::Write, path::Path};

pub const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS trade_log_outbox (id TEXT PRIMARY KEY, sequence INTEGER NOT NULL, path TEXT NOT NULL, payload TEXT NOT NULL, exported INTEGER NOT NULL DEFAULT 0);";
pub fn relative_path(timestamp: u64, id: &str) -> Result<String, String> {
    uuid::Uuid::parse_str(id).map_err(|_| "Invalid trade log identifier")?;
    let date = DateTime::<Utc>::from_timestamp(
        i64::try_from(timestamp).map_err(|_| "Invalid log time")?,
        0,
    )
    .ok_or("Invalid log time")?
    .with_timezone(&FixedOffset::east_opt(7 * 3600).unwrap());
    Ok(format!("{}/{id}.json", date.format("%Y/%m/%d")))
}
pub fn enqueue(db: &Connection, records: &[Value]) -> Result<(), String> {
    for record in records {
        let event = &record["event"];
        let id = event["id"].as_str().ok_or("Journal ID missing")?;
        let path = relative_path(
            event["timestamp"].as_u64().ok_or("Journal time missing")?,
            id,
        )?;
        db.execute(
            "INSERT OR IGNORE INTO trade_log_outbox(id,sequence,path,payload) VALUES(?1,?2,?3,?4)",
            params![
                id,
                event["sequence"].as_u64(),
                path,
                serde_json::to_string_pretty(record).map_err(|_| "Cannot encode trade log")?
            ],
        )
        .map_err(|_| "Cannot persist trade log outbox")?;
    }
    Ok(())
}
pub fn flush(db: &Connection, root: &Path) -> Result<(), String> {
    let mut statement = db
        .prepare("SELECT id,path,payload FROM trade_log_outbox WHERE exported=0 ORDER BY sequence")
        .map_err(|_| "Cannot read trade log outbox")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| "Cannot read trade log outbox")?;
    for row in rows {
        let (id, relative, payload) = row.map_err(|_| "Cannot read trade log row")?;
        let path = root.join(relative);
        if path.exists() {
            if fs::read(&path).map_err(|_| "Cannot verify trade log")? != payload.as_bytes() {
                return Err("Trade log integrity mismatch; execution paused".into());
            }
        } else {
            fs::create_dir_all(path.parent().ok_or("Invalid journal path")?)
                .map_err(|_| "Cannot create daily trade log directory")?;
            let temporary = path.with_extension("tmp");
            let mut options = fs::OpenOptions::new();
            options.create(true).truncate(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temporary)
                .map_err(|_| "Cannot write trade log")?;
            file.write_all(payload.as_bytes())
                .and_then(|_| file.sync_all())
                .map_err(|_| "Cannot sync trade log")?;
            drop(file);
            fs::rename(&temporary, &path).map_err(|_| "Cannot publish trade log")?;
            #[cfg(unix)]
            fs::File::open(path.parent().ok_or("Invalid journal directory")?)
                .and_then(|file| file.sync_all())
                .map_err(|_| "Cannot sync journal directory")?;
        }
        db.execute("UPDATE trade_log_outbox SET exported=1 WHERE id=?1", [id])
            .map_err(|_| "Cannot acknowledge trade log export")?;
    }
    Ok(())
}
pub fn recover(db: &Connection, root: &Path) -> Result<(), String> {
    // Restore missing exports from the durable outbox after a crash or incomplete backup.
    let mut statement = db
        .prepare("SELECT id,path FROM trade_log_outbox WHERE exported=1")
        .map_err(|_| "Cannot inspect journal recovery")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| "Cannot inspect journal recovery")?;
    for row in rows {
        let (id, path) = row.map_err(|_| "Cannot inspect journal recovery")?;
        if !root.join(path).is_file() {
            db.execute("UPDATE trade_log_outbox SET exported=0 WHERE id=?1", [id])
                .map_err(|_| "Cannot restore journal outbox")?;
        }
    }
    flush(db, root)
}
pub fn memory(db: &Connection, root: &Path) -> Result<Vec<Value>, String> {
    // Both modes are useful evidence, explicitly tagged; legacy replay never enters this outbox.
    let mut statement = db.prepare("SELECT path,payload FROM trade_log_outbox WHERE exported=1 AND json_extract(payload,'$.event.kind') IN ('execution.virtual_filled','position.closed','execution.reconciled','risk.rejected','position.exit_blocked') ORDER BY sequence DESC LIMIT 32").map_err(|_| "Cannot retrieve trade memory")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| "Cannot retrieve trade memory")?;
    let mut result = Vec::new();
    let mut budget = 64_000usize;
    for row in rows {
        let (relative, expected) = row.map_err(|_| "Cannot read trade memory")?;
        let bytes = fs::read(root.join(relative))
            .map_err(|_| "Trade memory file missing; restore journal before analysis")?;
        if bytes != expected.as_bytes() {
            return Err("Trade memory integrity mismatch".into());
        }
        if bytes.len() > budget {
            break;
        }
        budget -= bytes.len();
        result.push(serde_json::from_slice(&bytes).map_err(|_| "Invalid trade memory")?);
    }
    result.reverse();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jakarta_midnight_and_recovery_are_idempotent() {
        let id = uuid::Uuid::new_v4().to_string();
        assert!(relative_path(1_783_526_400, &id)
            .unwrap()
            .ends_with(&format!("/{id}.json")));
        let time = DateTime::parse_from_rfc3339("2026-09-08T17:00:00Z")
            .unwrap()
            .timestamp() as u64;
        assert_eq!(
            relative_path(time, &id).unwrap(),
            format!("2026/09/09/{id}.json")
        );
        assert!(relative_path(time, "../escape").is_err());
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        let root = std::env::temp_dir().join(&id);
        let record = serde_json::json!({"event":{"id":id,"timestamp":time,"sequence":1,"kind":"position.closed"}});
        enqueue(&db, std::slice::from_ref(&record)).unwrap();
        flush(&db, &root).unwrap();
        flush(&db, &root).unwrap();
        assert_eq!(memory(&db, &root).unwrap(), vec![record]);
        db.execute("UPDATE trade_log_outbox SET exported=0", [])
            .unwrap();
        flush(&db, &root).unwrap();
        fs::remove_file(root.join(relative_path(time, &id).unwrap())).unwrap();
        recover(&db, &root).unwrap();
        fs::write(root.join(relative_path(time, &id).unwrap()), "tampered").unwrap();
        assert!(memory(&db, &root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
