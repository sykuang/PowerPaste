pub const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial_schema",
        "\
        CREATE TABLE IF NOT EXISTS clipboard_items (\
          id TEXT PRIMARY KEY,\
          kind TEXT NOT NULL,\
          text TEXT NOT NULL,\
          created_at_ms INTEGER NOT NULL,\
          pinned INTEGER NOT NULL DEFAULT 0\
        );\
        CREATE INDEX IF NOT EXISTS idx_clipboard_items_created_at ON clipboard_items(created_at_ms DESC);\
        "
    ),
    (
        "002_add_pin_category",
        "\
        ALTER TABLE clipboard_items ADD COLUMN pin_category TEXT DEFAULT NULL;\
        CREATE INDEX IF NOT EXISTS idx_clipboard_items_pin_category ON clipboard_items(pin_category);\
        "
    ),
    (
        "003_add_image_file_support",
        "\
        ALTER TABLE clipboard_items ADD COLUMN image_width INTEGER DEFAULT NULL;\
        ALTER TABLE clipboard_items ADD COLUMN image_height INTEGER DEFAULT NULL;\
        ALTER TABLE clipboard_items ADD COLUMN image_size_bytes INTEGER DEFAULT NULL;\
        ALTER TABLE clipboard_items ADD COLUMN file_paths TEXT DEFAULT NULL;\
        ALTER TABLE clipboard_items ADD COLUMN content_type TEXT DEFAULT NULL;\
        ALTER TABLE clipboard_items ADD COLUMN image_data BLOB DEFAULT NULL;\
        "
    ),
    (
        "004_add_source_app",
        "\
        ALTER TABLE clipboard_items ADD COLUMN source_app_name TEXT DEFAULT NULL;\
        ALTER TABLE clipboard_items ADD COLUMN source_app_bundle_id TEXT DEFAULT NULL;\
        "
    ),
    (
        "005_add_trash_support",
        "\
        ALTER TABLE clipboard_items ADD COLUMN is_trashed INTEGER DEFAULT 0;\
        ALTER TABLE clipboard_items ADD COLUMN deleted_at_ms INTEGER DEFAULT NULL;\
        CREATE INDEX IF NOT EXISTS idx_clipboard_items_trashed ON clipboard_items(is_trashed, deleted_at_ms DESC);\
        "
    ),
    (
        "006_rename_pin_category_to_pinboard",
        "\
        ALTER TABLE clipboard_items ADD COLUMN pinboard TEXT DEFAULT NULL;\
        "
    ),
    (
        "007_add_perf_indexes",
        "\
        CREATE INDEX IF NOT EXISTS idx_clipboard_items_text ON clipboard_items(text);\
        CREATE INDEX IF NOT EXISTS idx_clipboard_items_kind_text ON clipboard_items(kind, text);\
        CREATE INDEX IF NOT EXISTS idx_clipboard_items_active_pinned_created_at \
          ON clipboard_items(is_trashed, pinned, created_at_ms DESC);\
        CREATE INDEX IF NOT EXISTS idx_clipboard_items_active_pinboard_created_at \
          ON clipboard_items(is_trashed, pinboard, created_at_ms DESC);\
        "
    ),
    (
        "008_add_fts",
        "\
        CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_items_fts USING fts5(\
          text,\
          content_type,\
          content='clipboard_items',\
          content_rowid='rowid'\
        );\
        CREATE TRIGGER IF NOT EXISTS clipboard_items_fts_ai AFTER INSERT ON clipboard_items \
        BEGIN \
          INSERT INTO clipboard_items_fts(rowid, text, content_type) VALUES (new.rowid, new.text, new.content_type); \
        END; \
        CREATE TRIGGER IF NOT EXISTS clipboard_items_fts_ad AFTER DELETE ON clipboard_items \
        BEGIN \
          INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, text, content_type) VALUES ('delete', old.rowid, old.text, old.content_type); \
        END; \
        CREATE TRIGGER IF NOT EXISTS clipboard_items_fts_au AFTER UPDATE ON clipboard_items \
        BEGIN \
          INSERT INTO clipboard_items_fts(clipboard_items_fts, rowid, text, content_type) VALUES ('delete', old.rowid, old.text, old.content_type); \
          INSERT INTO clipboard_items_fts(rowid, text, content_type) VALUES (new.rowid, new.text, new.content_type); \
        END; \
        INSERT INTO clipboard_items_fts(clipboard_items_fts) VALUES ('rebuild');\
        "
    ),
    (
        "009_add_image_mime",
        "\
        ALTER TABLE clipboard_items ADD COLUMN image_mime TEXT DEFAULT NULL;\
        "
    ),
];
