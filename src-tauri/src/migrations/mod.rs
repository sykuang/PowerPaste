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
];
