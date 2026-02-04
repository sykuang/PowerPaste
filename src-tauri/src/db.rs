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
            // Handle migrations that might fail due to column already existing
            let result = tx.execute_batch(sql);
            match result {
                Ok(_) => {},
                Err(e) => {
                    let err_msg = e.to_string();
                    // Ignore "duplicate column name" errors - column was added manually or by previous partial run
                    if !err_msg.contains("duplicate column name") {
                        return Err(format!("failed to apply migration {name}: {e}"));
                    }
                    eprintln!("[powerpaste] migration {name}: column already exists, skipping");
                }
            }
            
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

/// Insert a text item with optional content type detection.
/// If an item with the same text already exists, move it to the top by updating its timestamp.
pub fn insert_text_if_new_with_type(
    app: &tauri::AppHandle,
    text: &str,
    content_type: Option<String>,
) -> Result<Option<ClipboardItem>, String> {
    insert_text_with_source_app(app, text, content_type, None, None)
}

/// Insert a text item with source app information.
/// If an item with the same text already exists, move it to the top by updating its timestamp.
pub fn insert_text_with_source_app(
    app: &tauri::AppHandle,
    text: &str,
    content_type: Option<String>,
    source_app_name: Option<String>,
    source_app_bundle_id: Option<String>,
) -> Result<Option<ClipboardItem>, String> {
    let text = text.trim_end_matches(['\n', '\r']);
    if text.is_empty() {
        return Ok(None);
    }

    let conn = open(app)?;
    let now = now_ms();

    // Check if this exact text already exists anywhere in the clipboard
    // Also check for file items since file paths can be read as text by arboard
    let mut stmt = conn
        .prepare("SELECT id, kind, text, created_at_ms, pinned, pinboard, image_width, image_height, image_size_bytes, file_paths, content_type, source_app_name, source_app_bundle_id, is_trashed, deleted_at_ms FROM clipboard_items WHERE (kind = 'text' OR kind = 'file' OR content_type = 'file') AND text = ?1 LIMIT 1")
        .map_err(|e| format!("failed to prepare query: {e}"))?;
    
    let existing: Option<ClipboardItem> = stmt
        .query_row(params![text], row_to_item)
        .optional()
        .map_err(|e| format!("failed to check existing item: {e}"))?;

    if let Some(mut existing_item) = existing {
        // Item already exists - check if it's already the most recent
        let mut latest_stmt = conn
            .prepare("SELECT created_at_ms FROM clipboard_items ORDER BY created_at_ms DESC LIMIT 1")
            .map_err(|e| format!("failed to prepare latest query: {e}"))?;
        let latest_time: Option<i64> = latest_stmt
            .query_row([], |row| row.get(0))
            .optional()
            .map_err(|e| format!("failed to get latest time: {e}"))?;

        if latest_time == Some(existing_item.created_at_ms) {
            // Already the most recent item, no change needed
            return Ok(None);
        }

        // Move the existing item to the top by updating its timestamp
        // Also restore from trash if it was trashed
        conn.execute(
            "UPDATE clipboard_items SET created_at_ms = ?1, is_trashed = NULL, deleted_at_ms = NULL WHERE id = ?2",
            params![now, existing_item.id.to_string()],
        )
        .map_err(|e| format!("failed to update item timestamp: {e}"))?;

        existing_item.created_at_ms = now;
        existing_item.is_trashed = None;
        existing_item.deleted_at_ms = None;
        return Ok(Some(existing_item));
    }

    // No existing item found, create a new one
    let item = ClipboardItem {
        id: Uuid::new_v4(),
        kind: ClipboardItemKind::Text,
        text: text.to_string(),
        created_at_ms: now,
        pinned: false,
        pinboard: None,
        image_width: None,
        image_height: None,
        image_size_bytes: None,
        file_paths: None,
        content_type: content_type.clone(),
        source_app_name: source_app_name.clone(),
        source_app_bundle_id: source_app_bundle_id.clone(),
        is_trashed: None,
        deleted_at_ms: None,
    };

    conn.execute(
        "INSERT INTO clipboard_items (id, kind, text, created_at_ms, pinned, pinboard, content_type, source_app_name, source_app_bundle_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![item.id.to_string(), "text", item.text, item.created_at_ms, 0, Option::<String>::None, content_type, source_app_name, source_app_bundle_id],
    )
    .map_err(|e| format!("failed to insert item: {e}"))?;

    Ok(Some(item))
}

pub fn insert_text_if_new(app: &tauri::AppHandle, text: &str) -> Result<Option<ClipboardItem>, String> {
    insert_text_if_new_with_type(app, text, None)
}

/// Insert an image item from raw RGBA bytes.
/// If an image with the same hash already exists, move it to the top by updating its timestamp.
pub fn insert_image_if_new(
    app: &tauri::AppHandle,
    image_data: &[u8],
    width: u32,
    height: u32,
) -> Result<Option<ClipboardItem>, String> {
    insert_image_with_source_app(app, image_data, width, height, None, None)
}

/// Insert an image item with source app information.
pub fn insert_image_with_source_app(
    app: &tauri::AppHandle,
    image_data: &[u8],
    width: u32,
    height: u32,
    source_app_name: Option<String>,
    source_app_bundle_id: Option<String>,
) -> Result<Option<ClipboardItem>, String> {
    let conn = open(app)?;
    let now = now_ms();

    // Generate a simple hash of first 1KB for dedup check
    let hash_sample: Vec<u8> = image_data.iter().take(1024).copied().collect();
    let hash_str = format!("{:x}", md5_hash(&hash_sample));
    
    eprintln!("[powerpaste] insert_image_with_source_app: hash={}, size={}", hash_str, image_data.len());
    
    // Check if we already have this image anywhere (by hash stored in text field)
    let mut stmt = conn
        .prepare("SELECT id, kind, text, created_at_ms, pinned, pinboard, image_width, image_height, image_size_bytes, file_paths, content_type, source_app_name, source_app_bundle_id, is_trashed, deleted_at_ms FROM clipboard_items WHERE kind = 'image' AND text = ?1 LIMIT 1")
        .map_err(|e| format!("failed to prepare query: {e}"))?;
    
    let existing: Option<ClipboardItem> = stmt
        .query_row(params![hash_str], row_to_item)
        .optional()
        .map_err(|e| format!("failed to check existing image: {e}"))?;

    if let Some(mut existing_item) = existing {
        eprintln!("[powerpaste] found existing image with same hash, id={}", existing_item.id);
        // Image already exists - check if it's already the most recent
        let mut latest_stmt = conn
            .prepare("SELECT created_at_ms FROM clipboard_items ORDER BY created_at_ms DESC LIMIT 1")
            .map_err(|e| format!("failed to prepare latest query: {e}"))?;
        let latest_time: Option<i64> = latest_stmt
            .query_row([], |row| row.get(0))
            .optional()
            .map_err(|e| format!("failed to get latest time: {e}"))?;

        if latest_time == Some(existing_item.created_at_ms) {
            eprintln!("[powerpaste] existing image is already most recent, skipping");
            // Already the most recent item, no change needed
            return Ok(None);
        }

        eprintln!("[powerpaste] moving existing image to top: {} -> {}", existing_item.created_at_ms, now);
        // Move the existing item to the top by updating its timestamp
        // Also restore from trash if it was trashed
        conn.execute(
            "UPDATE clipboard_items SET created_at_ms = ?1, is_trashed = NULL, deleted_at_ms = NULL WHERE id = ?2",
            params![now, existing_item.id.to_string()],
        )
        .map_err(|e| format!("failed to update image timestamp: {e}"))?;

        // Verify the update worked
        let mut verify_stmt = conn
            .prepare("SELECT created_at_ms, pinboard, is_trashed FROM clipboard_items WHERE id = ?1")
            .map_err(|e| format!("failed to prepare verify query: {e}"))?;
        let (updated_time, pinboard, is_trashed): (i64, Option<String>, Option<i64>) = verify_stmt
            .query_row(params![existing_item.id.to_string()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|e| format!("failed to verify update: {e}"))?;
        eprintln!("[powerpaste] verified image after update: timestamp={}, pinboard={:?}, is_trashed={:?}", 
            updated_time, pinboard, is_trashed);

        existing_item.created_at_ms = now;
        existing_item.is_trashed = None;
        existing_item.deleted_at_ms = None;
        return Ok(Some(existing_item));
    }

    eprintln!("[powerpaste] inserting new image into database");
    // No existing image found, create a new one
    // Convert RGBA bytes to PNG
    let png_data = rgba_to_png(image_data, width, height)?;
    let size_bytes = png_data.len() as u64;
    
    let item = ClipboardItem {
        id: Uuid::new_v4(),
        kind: ClipboardItemKind::Image,
        text: hash_str.clone(), // Store hash as text for dedup
        created_at_ms: now,
        pinned: false,
        pinboard: None,
        image_width: Some(width),
        image_height: Some(height),
        image_size_bytes: Some(size_bytes),
        file_paths: None,
        content_type: Some("image".to_string()),
        source_app_name: source_app_name.clone(),
        source_app_bundle_id: source_app_bundle_id.clone(),
        is_trashed: None,
        deleted_at_ms: None,
    };

    conn.execute(
        "INSERT INTO clipboard_items (id, kind, text, created_at_ms, pinned, pinboard, image_width, image_height, image_size_bytes, content_type, image_data, source_app_name, source_app_bundle_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            item.id.to_string(),
            "image",
            item.text,
            item.created_at_ms,
            0,
            Option::<String>::None,
            width,
            height,
            size_bytes as i64,
            "image",
            png_data,
            source_app_name,
            source_app_bundle_id
        ],
    )
    .map_err(|e| format!("failed to insert image: {e}"))?;

    eprintln!("[powerpaste] image inserted into DB successfully, id={}, created_at_ms={}", item.id, item.created_at_ms);
    Ok(Some(item))
}

/// Move an item to the top of the list by updating its created_at_ms to now.
/// Returns true if the item was found and updated, false otherwise.
pub fn touch_item(app: &tauri::AppHandle, id: Uuid) -> Result<bool, String> {
    let conn = open(app)?;
    let now = now_ms();
    
    let rows_affected = conn
        .execute(
            "UPDATE clipboard_items SET created_at_ms = ?1 WHERE id = ?2 AND is_trashed = 0",
            params![now, id.to_string()],
        )
        .map_err(|e| format!("failed to touch item: {e}"))?;
    
    Ok(rows_affected > 0)
}

/// Convert raw RGBA bytes to PNG format
fn rgba_to_png(rgba_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    use image::{ImageBuffer, RgbaImage};
    use std::io::Cursor;
    
    // Create an image buffer from the RGBA data
    let img: RgbaImage = ImageBuffer::from_raw(width, height, rgba_data.to_vec())
        .ok_or_else(|| "failed to create image buffer from RGBA data".to_string())?;
    
    // Encode as PNG
    let mut png_bytes: Vec<u8> = Vec::new();
    let mut cursor = Cursor::new(&mut png_bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    
    Ok(png_bytes)
}

/// Simple hash function for image deduplication
fn md5_hash(data: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

/// Get image data (as base64 data URL) for a clipboard item
pub fn get_image_data(app: &tauri::AppHandle, id: Uuid) -> Result<Option<String>, String> {
    let conn = open(app)?;
    
    let mut stmt = conn
        .prepare("SELECT image_data FROM clipboard_items WHERE id = ?1 AND kind = 'image'")
        .map_err(|e| format!("failed to prepare query: {e}"))?;
    
    let data: Option<Vec<u8>> = stmt
        .query_row(params![id.to_string()], |row| row.get(0))
        .optional()
        .map_err(|e| format!("failed to query image: {e}"))?;
    
    match data {
        Some(bytes) if !bytes.is_empty() => {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            // Return as PNG data URL (arboard provides RGBA, but we'll treat as raw for now)
            Ok(Some(format!("data:image/png;base64,{}", b64)))
        }
        _ => Ok(None),
    }
}

fn row_to_item(row: &rusqlite::Row) -> Result<ClipboardItem, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let kind_str: String = row.get(1)?;
    let kind = match kind_str.as_str() {
        "image" => ClipboardItemKind::Image,
        "file" => ClipboardItemKind::File,
        _ => ClipboardItemKind::Text,
    };
    
    Ok(ClipboardItem {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
        kind,
        text: row.get(2)?,
        created_at_ms: row.get(3)?,
        pinned: row.get::<_, i64>(4)? != 0,
        pinboard: row.get(5)?,
        image_width: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
        image_height: row.get::<_, Option<i64>>(7)?.map(|v| v as u32),
        image_size_bytes: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
        file_paths: row.get(9)?,
        content_type: row.get(10)?,
        source_app_name: row.get(11)?,
        source_app_bundle_id: row.get(12)?,
        is_trashed: row.get::<_, Option<i64>>(13)?.map(|v| v != 0),
        deleted_at_ms: row.get(14)?,
    })
}

pub fn list_items(app: &tauri::AppHandle, limit: u32, query: Option<String>) -> Result<Vec<ClipboardItem>, String> {
    let conn = open(app)?;
    let limit = limit.clamp(1, 5000) as i64;

    let mut items = Vec::new();
    
    let base_cols = "id, kind, text, created_at_ms, pinned, pinboard, image_width, image_height, image_size_bytes, file_paths, content_type, source_app_name, source_app_bundle_id, is_trashed, deleted_at_ms";

    if let Some(q) = query.filter(|s| !s.trim().is_empty()) {
        let q = format!("%{}%", q.trim());
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} \
                 FROM clipboard_items \
                 WHERE (is_trashed IS NULL OR is_trashed = 0) AND (text LIKE ?1 OR content_type LIKE ?1) \
                 ORDER BY pinned DESC, created_at_ms DESC \
                 LIMIT ?2",
                base_cols
            ))
            .map_err(|e| format!("failed to prepare list query: {e}"))?;
        let rows = stmt
            .query_map(params![q, limit], row_to_item)
            .map_err(|e| format!("failed to query items: {e}"))?;

        for r in rows {
            items.push(r.map_err(|e| format!("failed to read row: {e}"))?);
        }
        return Ok(items);
    }

    let mut stmt = conn
        .prepare(&format!(
            "SELECT {} \
             FROM clipboard_items \
             WHERE is_trashed IS NULL OR is_trashed = 0 \
             ORDER BY pinned DESC, created_at_ms DESC \
             LIMIT ?1",
            base_cols
        ))
        .map_err(|e| format!("failed to prepare list query: {e}"))?;
    let rows = stmt
        .query_map(params![limit], row_to_item)
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
                "INSERT OR IGNORE INTO clipboard_items (id, kind, text, created_at_ms, pinned, pinboard) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    item.id.to_string(),
                    "text",
                    item.text,
                    item.created_at_ms,
                    if item.pinned { 1 } else { 0 },
                    &item.pinboard
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

/// Set the pinboard for an item. Pass None to remove from a pinboard.
pub fn set_pinboard(app: &tauri::AppHandle, id: Uuid, pinboard: Option<String>) -> Result<(), String> {
    let conn = open(app)?;
    conn.execute(
        "UPDATE clipboard_items SET pinboard = ?1, pinned = 1 WHERE id = ?2",
        params![pinboard, id.to_string()],
    )
    .map_err(|e| format!("failed to update pinboard: {e}"))?;
    Ok(())
}

/// List all unique pinboard names (non-null).
pub fn list_pinboards(app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    let conn = open(app)?;
    let mut stmt = conn
        .prepare("SELECT DISTINCT pinboard FROM clipboard_items WHERE pinboard IS NOT NULL ORDER BY pinboard")
        .map_err(|e| format!("failed to prepare categories query: {e}"))?;
    
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("failed to query categories: {e}"))?;
    
    let mut categories = Vec::new();
    for r in rows {
        categories.push(r.map_err(|e| format!("failed to read category: {e}"))?);
    }
    Ok(categories)
}

pub fn delete_item(app: &tauri::AppHandle, id: Uuid) -> Result<(), String> {
    let conn = open(app)?;
    conn.execute("DELETE FROM clipboard_items WHERE id = ?1", params![id.to_string()])
        .map_err(|e| format!("failed to delete item: {e}"))?;
    Ok(())
}

/// Move an item to trash (soft delete)
pub fn trash_item(app: &tauri::AppHandle, id: Uuid) -> Result<(), String> {
    let conn = open(app)?;
    let now = now_ms();
    conn.execute(
        "UPDATE clipboard_items SET is_trashed = 1, deleted_at_ms = ?1 WHERE id = ?2",
        params![now, id.to_string()],
    )
    .map_err(|e| format!("failed to trash item: {e}"))?;
    Ok(())
}

/// Restore an item from trash
pub fn restore_from_trash(app: &tauri::AppHandle, id: Uuid) -> Result<(), String> {
    let conn = open(app)?;
    conn.execute(
        "UPDATE clipboard_items SET is_trashed = 0, deleted_at_ms = NULL WHERE id = ?1",
        params![id.to_string()],
    )
    .map_err(|e| format!("failed to restore item: {e}"))?;
    Ok(())
}

/// Permanently delete an item (bypass trash)
pub fn delete_item_forever(app: &tauri::AppHandle, id: Uuid) -> Result<(), String> {
    let conn = open(app)?;
    conn.execute("DELETE FROM clipboard_items WHERE id = ?1", params![id.to_string()])
        .map_err(|e| format!("failed to delete item forever: {e}"))?;
    Ok(())
}

/// Empty the trash (permanently delete all trashed items)
pub fn empty_trash(app: &tauri::AppHandle) -> Result<u32, String> {
    let conn = open(app)?;
    let deleted = conn
        .execute("DELETE FROM clipboard_items WHERE is_trashed = 1", [])
        .map_err(|e| format!("failed to empty trash: {e}"))?;
    Ok(deleted as u32)
}

/// Get count of items in trash
pub fn get_trash_count(app: &tauri::AppHandle) -> Result<u32, String> {
    let conn = open(app)?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE is_trashed = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("failed to count trash: {e}"))?;
    Ok(count as u32)
}

/// List trashed items with pagination
pub fn list_trashed_items(app: &tauri::AppHandle, limit: u32, offset: u32) -> Result<(Vec<ClipboardItem>, u32), String> {
    let conn = open(app)?;
    let limit = limit.clamp(1, 100) as i64;
    let offset = offset as i64;

    let base_cols = "id, kind, text, created_at_ms, pinned, pinboard, image_width, image_height, image_size_bytes, file_paths, content_type, source_app_name, source_app_bundle_id, is_trashed, deleted_at_ms";

    // Get total count
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE is_trashed = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("failed to count trashed items: {e}"))?;

    let mut items = Vec::new();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {} \
             FROM clipboard_items \
             WHERE is_trashed = 1 \
             ORDER BY deleted_at_ms DESC \
             LIMIT ?1 OFFSET ?2",
            base_cols
        ))
        .map_err(|e| format!("failed to prepare trashed query: {e}"))?;
    
    let rows = stmt
        .query_map(params![limit, offset], row_to_item)
        .map_err(|e| format!("failed to query trashed items: {e}"))?;

    for r in rows {
        items.push(r.map_err(|e| format!("failed to read row: {e}"))?);
    }

    Ok((items, total as u32))
}

/// List items with pagination (excludes trashed)
pub fn list_items_paginated(
    app: &tauri::AppHandle,
    limit: u32,
    offset: u32,
    query: Option<String>,
) -> Result<(Vec<ClipboardItem>, u32), String> {
    let conn = open(app)?;
    let limit = limit.clamp(1, 100) as i64;
    let offset = offset as i64;

    let base_cols = "id, kind, text, created_at_ms, pinned, pinboard, image_width, image_height, image_size_bytes, file_paths, content_type, source_app_name, source_app_bundle_id, is_trashed, deleted_at_ms";

    let mut items = Vec::new();

    if let Some(q) = query.filter(|s| !s.trim().is_empty()) {
        let q = format!("%{}%", q.trim());
        
        // Get total count for search
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE (is_trashed IS NULL OR is_trashed = 0) AND (text LIKE ?1 OR content_type LIKE ?1)",
                params![q],
                |row| row.get(0),
            )
            .map_err(|e| format!("failed to count items: {e}"))?;

        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} \
                 FROM clipboard_items \
                 WHERE (is_trashed IS NULL OR is_trashed = 0) AND (text LIKE ?1 OR content_type LIKE ?1) \
                 ORDER BY pinned DESC, created_at_ms DESC \
                 LIMIT ?2 OFFSET ?3",
                base_cols
            ))
            .map_err(|e| format!("failed to prepare paginated query: {e}"))?;
        
        let rows = stmt
            .query_map(params![q, limit, offset], row_to_item)
            .map_err(|e| format!("failed to query items: {e}"))?;

        for r in rows {
            items.push(r.map_err(|e| format!("failed to read row: {e}"))?);
        }
        return Ok((items, total as u32));
    }

    // Get total count
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE is_trashed IS NULL OR is_trashed = 0",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("failed to count items: {e}"))?;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT {} \
             FROM clipboard_items \
             WHERE is_trashed IS NULL OR is_trashed = 0 \
             ORDER BY pinned DESC, created_at_ms DESC \
             LIMIT ?1 OFFSET ?2",
            base_cols
        ))
        .map_err(|e| format!("failed to prepare paginated query: {e}"))?;
    
    let rows = stmt
        .query_map(params![limit, offset], row_to_item)
        .map_err(|e| format!("failed to query items: {e}"))?;

    for r in rows {
        items.push(r.map_err(|e| format!("failed to read row: {e}"))?);
    }

    Ok((items, total as u32))
}

/// List pinboard items with pagination
pub fn list_pinboard_items_paginated(
    app: &tauri::AppHandle,
    limit: u32,
    offset: u32,
    pinboard: Option<String>,
) -> Result<(Vec<ClipboardItem>, u32), String> {
    let conn = open(app)?;
    let limit = limit.clamp(1, 100) as i64;
    let offset = offset as i64;

    let base_cols = "id, kind, text, created_at_ms, pinned, pinboard, image_width, image_height, image_size_bytes, file_paths, content_type, source_app_name, source_app_bundle_id, is_trashed, deleted_at_ms";

    let mut items = Vec::new();

    if let Some(pb) = pinboard.filter(|s| !s.trim().is_empty()) {
        // Get total count for specific pinboard
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE (is_trashed IS NULL OR is_trashed = 0) AND pinboard = ?1",
                params![pb],
                |row| row.get(0),
            )
            .map_err(|e| format!("failed to count pinboard items: {e}"))?;

        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} \
                 FROM clipboard_items \
                 WHERE (is_trashed IS NULL OR is_trashed = 0) AND pinboard = ?1 \
                 ORDER BY created_at_ms DESC \
                 LIMIT ?2 OFFSET ?3",
                base_cols
            ))
            .map_err(|e| format!("failed to prepare pinboard query: {e}"))?;
        
        let rows = stmt
            .query_map(params![pb, limit, offset], row_to_item)
            .map_err(|e| format!("failed to query pinboard items: {e}"))?;

        for r in rows {
            items.push(r.map_err(|e| format!("failed to read row: {e}"))?);
        }
        return Ok((items, total as u32));
    }

    // Get all pinned items (all pinboards)
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE (is_trashed IS NULL OR is_trashed = 0) AND pinned = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("failed to count pinned items: {e}"))?;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT {} \
             FROM clipboard_items \
             WHERE (is_trashed IS NULL OR is_trashed = 0) AND pinned = 1 \
             ORDER BY created_at_ms DESC \
             LIMIT ?1 OFFSET ?2",
            base_cols
        ))
        .map_err(|e| format!("failed to prepare pinned query: {e}"))?;
    
    let rows = stmt
        .query_map(params![limit, offset], row_to_item)
        .map_err(|e| format!("failed to query pinned items: {e}"))?;

    for r in rows {
        items.push(r.map_err(|e| format!("failed to read row: {e}"))?);
    }

    Ok((items, total as u32))
}

/// Clean up items older than retention period (move to trash if enabled, or delete)
pub fn cleanup_old_items(app: &tauri::AppHandle, retention_days: i32, trash_enabled: bool) -> Result<u32, String> {
    let conn = open(app)?;
    let cutoff_ms = now_ms() - (retention_days as i64 * 24 * 60 * 60 * 1000);

    if trash_enabled {
        // Move old items to trash
        let now = now_ms();
        let affected = conn
            .execute(
                "UPDATE clipboard_items SET is_trashed = 1, deleted_at_ms = ?1 \
                 WHERE (is_trashed IS NULL OR is_trashed = 0) AND pinned = 0 AND created_at_ms < ?2",
                params![now, cutoff_ms],
            )
            .map_err(|e| format!("failed to trash old items: {e}"))?;
        Ok(affected as u32)
    } else {
        // Permanently delete old items
        let affected = conn
            .execute(
                "DELETE FROM clipboard_items \
                 WHERE (is_trashed IS NULL OR is_trashed = 0) AND pinned = 0 AND created_at_ms < ?1",
                params![cutoff_ms],
            )
            .map_err(|e| format!("failed to delete old items: {e}"))?;
        Ok(affected as u32)
    }
}

/// Clean up trashed items older than trash retention period
pub fn cleanup_old_trash(app: &tauri::AppHandle, trash_retention_days: i32) -> Result<u32, String> {
    let conn = open(app)?;
    let cutoff_ms = now_ms() - (trash_retention_days as i64 * 24 * 60 * 60 * 1000);

    let affected = conn
        .execute(
            "DELETE FROM clipboard_items WHERE is_trashed = 1 AND deleted_at_ms < ?1",
            params![cutoff_ms],
        )
        .map_err(|e| format!("failed to cleanup old trash: {e}"))?;
    Ok(affected as u32)
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
