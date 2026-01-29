use crate::models::{ClipboardItem, ClipboardItemKind};
use crate::paths::db_path;
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[path = "migrations/mod.rs"]
mod migrations;

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn open(app: &tauri::AppHandle) -> Result<Connection, String> {
    let path = db_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create db parent dir: {e}"))?;
    }
    let mut conn = Connection::open(path).map_err(|e| format!("failed to open sqlite db: {e}"))?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<(), String> {
    // Enable WAL mode
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| format!("failed to enable WAL: {e}"))?;

    // Create migrations table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY, applied_at_ms INTEGER NOT NULL)",
        [],
    )
    .map_err(|e| format!("failed to init migrations table: {e}"))?;

    let tx = conn.transaction().map_err(|e| format!("failed to start migration tx: {e}"))?;

    for (name, sql) in migrations::MIGRATIONS {
        let count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM _migrations WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|e| format!("failed to check migration {name}: {e}"))?;

        if count == 0 {
            tx.execute_batch(sql)
                .map_err(|e| format!("failed to apply migration {name}: {e}"))?;
            
            tx.execute(
                "INSERT INTO _migrations (name, applied_at_ms) VALUES (?1, ?2)",
                params![name, now_ms()],
            )
            .map_err(|e| format!("failed to record migration {name}: {e}"))?;
            
            eprintln!("[powerpaste] applied migration: {name}");
        }
    }

    tx.commit().map_err(|e| format!("failed to commit migrations: {e}"))?;
    Ok(())
}

pub fn insert_text_if_new(app: &tauri::AppHandle, text: &str) -> Result<Option<ClipboardItem>, String> {
    let text = text.trim_end_matches(['\n', '\r']);
    if text.is_empty() {
        return Ok(None);
    }

    let conn = open(app)?;

    // Skip duplicates of the most recent entry.
    let mut stmt = conn
        .prepare("SELECT text FROM clipboard_items ORDER BY created_at_ms DESC LIMIT 1")
        .map_err(|e| format!("failed to prepare query: {e}"))?;
    let last: Option<String> = stmt
        .query_row([], |row| row.get(0))
        .optional()
        .map_err(|e| format!("failed to read last item: {e}"))?;

    if let Some(last) = last {
        if last == text {
            return Ok(None);
        }
    }

    let item = ClipboardItem {
        id: Uuid::new_v4(),
        kind: ClipboardItemKind::Text,
        text: text.to_string(),
        created_at_ms: now_ms(),
        pinned: false,
    };

    conn.execute(
        "INSERT INTO clipboard_items (id, kind, text, created_at_ms, pinned) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![item.id.to_string(), "text", item.text, item.created_at_ms, 0],
    )
    .map_err(|e| format!("failed to insert item: {e}"))?;

    Ok(Some(item))
}

pub fn list_items(app: &tauri::AppHandle, limit: u32, query: Option<String>) -> Result<Vec<ClipboardItem>, String> {
    let conn = open(app)?;
    let limit = limit.clamp(1, 5000) as i64;

    let mut items = Vec::new();

    if let Some(q) = query.filter(|s| !s.trim().is_empty()) {
        let q = format!("%{}%", q.trim());
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, text, created_at_ms, pinned \
                 FROM clipboard_items \
                 WHERE text LIKE ?1 \
                 ORDER BY pinned DESC, created_at_ms DESC \
                 LIMIT ?2",
            )
            .map_err(|e| format!("failed to prepare list query: {e}"))?;
        let rows = stmt
            .query_map(params![q, limit], |row| {
                let id_str: String = row.get(0)?;
                Ok(ClipboardItem {
                    id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    kind: ClipboardItemKind::Text,
                    text: row.get(2)?,
                    created_at_ms: row.get(3)?,
                    pinned: row.get::<_, i64>(4)? != 0,
                })
            })
            .map_err(|e| format!("failed to query items: {e}"))?;

        for r in rows {
            items.push(r.map_err(|e| format!("failed to read row: {e}"))?);
        }
        return Ok(items);
    }

    let mut stmt = conn
        .prepare(
            "SELECT id, kind, text, created_at_ms, pinned \
             FROM clipboard_items \
             ORDER BY pinned DESC, created_at_ms DESC \
             LIMIT ?1",
        )
        .map_err(|e| format!("failed to prepare list query: {e}"))?;
    let rows = stmt
        .query_map(params![limit], |row| {
            let id_str: String = row.get(0)?;
            Ok(ClipboardItem {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                kind: ClipboardItemKind::Text,
                text: row.get(2)?,
                created_at_ms: row.get(3)?,
                pinned: row.get::<_, i64>(4)? != 0,
            })
        })
        .map_err(|e| format!("failed to query items: {e}"))?;

    for r in rows {
        items.push(r.map_err(|e| format!("failed to read row: {e}"))?);
    }

    Ok(items)
}

pub fn upsert_items(app: &tauri::AppHandle, items: &[ClipboardItem]) -> Result<u32, String> {
    let mut conn = open(app)?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("failed to start transaction: {e}"))?;

    let mut inserted = 0u32;
    for item in items {
        let changed = tx
            .execute(
                "INSERT OR IGNORE INTO clipboard_items (id, kind, text, created_at_ms, pinned) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    item.id.to_string(),
                    "text",
                    item.text,
                    item.created_at_ms,
                    if item.pinned { 1 } else { 0 }
                ],
            )
            .map_err(|e| format!("failed to upsert item: {e}"))?;
        if changed > 0 {
            inserted += 1;
        }
    }

    tx.commit()
        .map_err(|e| format!("failed to commit transaction: {e}"))?;
    Ok(inserted)
}

pub fn set_pinned(app: &tauri::AppHandle, id: Uuid, pinned: bool) -> Result<(), String> {
    let conn = open(app)?;
    conn.execute(
        "UPDATE clipboard_items SET pinned = ?1 WHERE id = ?2",
        params![if pinned { 1 } else { 0 }, id.to_string()],
    )
    .map_err(|e| format!("failed to update pinned: {e}"))?;
    Ok(())
}

pub fn delete_item(app: &tauri::AppHandle, id: Uuid) -> Result<(), String> {
    let conn = open(app)?;
    conn.execute("DELETE FROM clipboard_items WHERE id = ?1", params![id.to_string()])
        .map_err(|e| format!("failed to delete item: {e}"))?;
    Ok(())
}

// Needed for rusqlite::OptionalExtension.
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
