CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY NOT NULL,
    applied_at TEXT NOT NULL,
    checksum TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS aliases (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE BINARY,
    name_folded TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    target_type TEXT NOT NULL,
    executable TEXT NOT NULL,
    fixed_args_json TEXT NOT NULL,
    pass_args INTEGER NOT NULL DEFAULT 1,
    working_directory TEXT,
    environment_json TEXT NOT NULL DEFAULT '{}',
    shells_json TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    advanced_shell_mode INTEGER NOT NULL DEFAULT 0,
    tags_json TEXT NOT NULL DEFAULT '[]',
    path_mode TEXT NOT NULL DEFAULT 'absolute',
    path_origin TEXT NOT NULL DEFAULT 'cli_argument',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    record_checksum TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 1
);

CREATE UNIQUE INDEX IF NOT EXISTS aliases_name_unique ON aliases(name COLLATE BINARY);
CREATE INDEX IF NOT EXISTS aliases_name_folded_idx ON aliases(name_folded);
