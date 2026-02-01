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
];
