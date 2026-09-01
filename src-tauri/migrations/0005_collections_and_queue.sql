CREATE TABLE playlists (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 120),
    kind TEXT NOT NULL CHECK (kind IN ('normal', 'inbox', 'branch')),
    parent_playlist_id TEXT,
    base_parent_revision INTEGER CHECK (base_parent_revision IS NULL OR base_parent_revision >= 0),
    branch_status TEXT CHECK (branch_status IS NULL OR branch_status IN ('open', 'merged')),
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (parent_playlist_id) REFERENCES playlists(id) ON DELETE RESTRICT,
    CHECK (
        (kind = 'branch'
            AND parent_playlist_id IS NOT NULL
            AND base_parent_revision IS NOT NULL
            AND branch_status IS NOT NULL)
        OR
        (kind IN ('normal', 'inbox')
            AND parent_playlist_id IS NULL
            AND base_parent_revision IS NULL
            AND branch_status IS NULL)
    )
);

CREATE UNIQUE INDEX ux_playlists_single_inbox
    ON playlists(kind)
    WHERE kind = 'inbox';

CREATE INDEX idx_playlists_parent_status
    ON playlists(parent_playlist_id, branch_status);

CREATE TRIGGER playlists_inbox_immutable_update
BEFORE UPDATE OF name, kind, parent_playlist_id, base_parent_revision, branch_status ON playlists
WHEN OLD.id = '00000000-0000-0000-0000-000000000001'
BEGIN
    SELECT RAISE(ABORT, 'Inbox is immutable');
END;

CREATE TRIGGER playlists_inbox_immutable_delete
BEFORE DELETE ON playlists
WHEN OLD.id = '00000000-0000-0000-0000-000000000001'
BEGIN
    SELECT RAISE(ABORT, 'Inbox cannot be deleted');
END;

CREATE TABLE playlist_items (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    playlist_id TEXT NOT NULL,
    track_id TEXT NOT NULL,
    requested_source_id TEXT,
    position INTEGER NOT NULL CHECK (position >= 0),
    origin_base_item_id TEXT,
    added_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (requested_source_id) REFERENCES track_sources(id) ON DELETE SET NULL
);

CREATE INDEX idx_playlist_items_playlist_position
    ON playlist_items(playlist_id, position, id);

CREATE INDEX idx_playlist_items_track_id
    ON playlist_items(track_id);

CREATE TRIGGER playlist_items_requested_source_same_track_insert
BEFORE INSERT ON playlist_items
WHEN NEW.requested_source_id IS NOT NULL
 AND NOT EXISTS (
     SELECT 1 FROM track_sources
     WHERE id = NEW.requested_source_id AND track_id = NEW.track_id
 )
BEGIN
    SELECT RAISE(ABORT, 'requested source must belong to track');
END;

CREATE TRIGGER playlist_items_requested_source_same_track_update
BEFORE UPDATE OF requested_source_id, track_id ON playlist_items
WHEN NEW.requested_source_id IS NOT NULL
 AND NOT EXISTS (
     SELECT 1 FROM track_sources
     WHERE id = NEW.requested_source_id AND track_id = NEW.track_id
 )
BEGIN
    SELECT RAISE(ABORT, 'requested source must belong to track');
END;

CREATE TABLE playlist_branch_base_items (
    branch_playlist_id TEXT NOT NULL,
    base_item_id TEXT NOT NULL,
    track_id TEXT NOT NULL,
    requested_source_id TEXT,
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (branch_playlist_id, base_item_id),
    FOREIGN KEY (branch_playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (requested_source_id) REFERENCES track_sources(id) ON DELETE SET NULL
);

CREATE INDEX idx_playlist_branch_base_position
    ON playlist_branch_base_items(branch_playlist_id, position, base_item_id);

CREATE TRIGGER playlist_branch_base_requested_source_same_track_insert
BEFORE INSERT ON playlist_branch_base_items
WHEN NEW.requested_source_id IS NOT NULL
 AND NOT EXISTS (
     SELECT 1 FROM track_sources
     WHERE id = NEW.requested_source_id AND track_id = NEW.track_id
 )
BEGIN
    SELECT RAISE(ABORT, 'requested source must belong to track');
END;

CREATE TABLE likes (
    track_id TEXT PRIMARY KEY NOT NULL,
    liked_at TEXT NOT NULL,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

CREATE TABLE ratings (
    track_id TEXT PRIMARY KEY NOT NULL,
    rating INTEGER NOT NULL CHECK (rating BETWEEN 1 AND 5),
    updated_at TEXT NOT NULL,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

CREATE TABLE tags (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 64),
    normalized_name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(normalized_name)) BETWEEN 1 AND 64),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE track_tags (
    track_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (track_id, tag_id),
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE INDEX idx_track_tags_tag_id
    ON track_tags(tag_id, track_id);

CREATE TABLE queue_entries (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    track_id TEXT NOT NULL,
    requested_source_id TEXT,
    section TEXT NOT NULL CHECK (section IN ('up_next', 'later', 'autoplay')),
    position INTEGER NOT NULL CHECK (position >= 0),
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (requested_source_id) REFERENCES track_sources(id) ON DELETE SET NULL
);

CREATE INDEX idx_queue_entries_section_position
    ON queue_entries(section, position, id);

CREATE TRIGGER queue_entries_requested_source_same_track_insert
BEFORE INSERT ON queue_entries
WHEN NEW.requested_source_id IS NOT NULL
 AND NOT EXISTS (
     SELECT 1 FROM track_sources
     WHERE id = NEW.requested_source_id AND track_id = NEW.track_id
 )
BEGIN
    SELECT RAISE(ABORT, 'requested source must belong to track');
END;

CREATE TRIGGER queue_entries_requested_source_same_track_update
BEFORE UPDATE OF requested_source_id, track_id ON queue_entries
WHEN NEW.requested_source_id IS NOT NULL
 AND NOT EXISTS (
     SELECT 1 FROM track_sources
     WHERE id = NEW.requested_source_id AND track_id = NEW.track_id
 )
BEGIN
    SELECT RAISE(ABORT, 'requested source must belong to track');
END;

CREATE TABLE queue_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    current_entry_id TEXT,
    current_position_ms INTEGER NOT NULL DEFAULT 0 CHECK (current_position_ms >= 0),
    repeat_mode TEXT NOT NULL DEFAULT 'off' CHECK (repeat_mode IN ('off', 'one', 'all')),
    shuffle_enabled INTEGER NOT NULL DEFAULT 0 CHECK (shuffle_enabled IN (0, 1)),
    history_order_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(history_order_json)),
    shuffle_order_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(shuffle_order_json)),
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    updated_at TEXT NOT NULL,
    FOREIGN KEY (current_entry_id) REFERENCES queue_entries(id) ON DELETE SET NULL
);

CREATE TABLE queue_snapshots (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    current_track_id TEXT,
    current_source_id TEXT,
    current_position_ms INTEGER NOT NULL CHECK (current_position_ms >= 0),
    repeat_mode TEXT NOT NULL CHECK (repeat_mode IN ('off', 'one', 'all')),
    shuffle_enabled INTEGER NOT NULL CHECK (shuffle_enabled IN (0, 1)),
    current_snapshot_entry_id TEXT,
    history_order_json TEXT NOT NULL CHECK (json_valid(history_order_json)),
    shuffle_order_json TEXT NOT NULL CHECK (json_valid(shuffle_order_json)),
    created_at TEXT NOT NULL,
    FOREIGN KEY (current_track_id) REFERENCES tracks(id) ON DELETE SET NULL,
    FOREIGN KEY (current_source_id) REFERENCES track_sources(id) ON DELETE SET NULL,
    FOREIGN KEY (current_snapshot_entry_id) REFERENCES queue_snapshot_entries(id) ON DELETE SET NULL
);

CREATE INDEX idx_queue_snapshots_created_at
    ON queue_snapshots(created_at DESC, id);

CREATE TABLE queue_snapshot_entries (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    snapshot_id TEXT NOT NULL,
    track_id TEXT NOT NULL,
    requested_source_id TEXT,
    section TEXT NOT NULL CHECK (section IN ('up_next', 'later', 'autoplay')),
    position INTEGER NOT NULL CHECK (position >= 0),
    pinned INTEGER NOT NULL CHECK (pinned IN (0, 1)),
    traversal_position INTEGER NOT NULL CHECK (traversal_position >= 0),
    FOREIGN KEY (snapshot_id) REFERENCES queue_snapshots(id) ON DELETE CASCADE,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (requested_source_id) REFERENCES track_sources(id) ON DELETE SET NULL
);

CREATE INDEX idx_queue_snapshot_entries_snapshot_position
    ON queue_snapshot_entries(snapshot_id, traversal_position, position, id);

CREATE TRIGGER queue_snapshot_entries_requested_source_same_track_insert
BEFORE INSERT ON queue_snapshot_entries
WHEN NEW.requested_source_id IS NOT NULL
 AND NOT EXISTS (
     SELECT 1 FROM track_sources
     WHERE id = NEW.requested_source_id AND track_id = NEW.track_id
 )
BEGIN
    SELECT RAISE(ABORT, 'requested source must belong to track');
END;

CREATE TRIGGER queue_snapshots_current_entry_delete
AFTER DELETE ON queue_snapshot_entries
WHEN OLD.id IS NOT NULL
BEGIN
    UPDATE queue_snapshots
    SET current_snapshot_entry_id = NULL
    WHERE current_snapshot_entry_id = OLD.id;
END;

INSERT INTO playlists (
    id, name, kind, revision, created_at, updated_at
) VALUES (
    '00000000-0000-0000-0000-000000000001', 'Inbox', 'inbox', 0,
    '1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z'
);

INSERT INTO queue_state (
    id, current_entry_id, current_position_ms, repeat_mode, shuffle_enabled,
    history_order_json, shuffle_order_json, revision, updated_at
) VALUES (
    1, NULL, 0, 'off', 0, '[]', '[]', 0, '1970-01-01T00:00:00Z'
);

UPDATE schema_metadata
SET metadata_value = '5', updated_at = '1970-01-01T00:00:00Z'
WHERE metadata_key = 'schema_version';
