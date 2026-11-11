-- tables
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS folders (
    folder_id TEXT PRIMARY KEY NOT NULL,
    source TEXT NOT NULL DEFAULT 'builtin', -- 'builtin' | 'plugin:<id>'
    parent_id TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL,
    custom_icon TEXT, -- nullable, plugin-populated
    data TEXT NOT NULL -- period, meeting_days, colors, order, hidden, description…
);
CREATE TABLE IF NOT EXISTS resources (
    folder_id TEXT NOT NULL REFERENCES folders(folder_id) ON DELETE CASCADE,
    resource_id TEXT NOT NULL,
    type TEXT NOT NULL,
    parent_id TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'builtin',
    title TEXT NOT NULL,
    custom_icon TEXT, -- nullable, plugin-populated
    data TEXT NOT NULL,
    last_updated TEXT,
    PRIMARY KEY (folder_id, type, resource_id)
);
CREATE TABLE IF NOT EXISTS tasks (
    folder_id TEXT NOT NULL,
    id TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'builtin',
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    due TEXT, -- nullable; calendar shows when set
    attachments TEXT NOT NULL,
    type TEXT GENERATED ALWAYS AS ('task') VIRTUAL,
    PRIMARY KEY (folder_id, id),
    FOREIGN KEY (folder_id, type, id)
        REFERENCES resources(folder_id, type, resource_id) ON DELETE CASCADE
);
-- Completion is native Foam task state, not source/plugin-specific data.
CREATE TABLE IF NOT EXISTS task_state (
    folder_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    completed INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (folder_id, task_id),
    FOREIGN KEY (folder_id, task_id) REFERENCES tasks(folder_id, id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS notifications (
    id TEXT NOT NULL,
    is_unread INTEGER NOT NULL DEFAULT 1,
    event TEXT NOT NULL,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    created TEXT NOT NULL,
    resource_id TEXT NOT NULL UNIQUE,
    resource_type TEXT,
    folder_id TEXT,
    folder_title TEXT,
    source TEXT NOT NULL DEFAULT 'builtin',
    seq INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);
CREATE TABLE IF NOT EXISTS sync_state (
    key TEXT PRIMARY KEY NOT NULL,
    data TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS attachments (
    url TEXT PRIMARY KEY NOT NULL,
    file_path TEXT NOT NULL
);

--  indexes
CREATE INDEX IF NOT EXISTS tasks_id ON tasks(id);
CREATE INDEX IF NOT EXISTS resources_parent ON resources(folder_id, parent_id);

CREATE TRIGGER IF NOT EXISTS task_title_updated AFTER UPDATE OF title ON tasks
BEGIN
    UPDATE resources SET title = NEW.title
    WHERE folder_id = NEW.folder_id AND type = 'task' AND resource_id = NEW.id;
END;

-- fts5 search mapppings
CREATE TABLE IF NOT EXISTS search_mapping (
    id INTEGER PRIMARY KEY,
    item_type TEXT NOT NULL,
    item_id TEXT NOT NULL,
    folder_id TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'builtin', -- 'builtin' | 'plugin:<id>'
    UNIQUE(folder_id, item_type, item_id)
);

-- fts5 search index (supports folders, resources, tasks)
CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5 (
    title,
    content,
    item_type UNINDEXED,
    item_id,
    tokenize="porter unicode61"
);

-- fts5 search index trigger: update index when table `tasks` changes
CREATE TRIGGER IF NOT EXISTS task_add AFTER INSERT ON tasks BEGIN
    INSERT OR IGNORE INTO search_mapping(item_type, item_id, folder_id)
    VALUES ('task', new.id, new.folder_id);
    INSERT OR REPLACE INTO search_index(rowid, title, content, item_type, item_id)
    SELECT id, new.title, new.description, 'task', new.id FROM search_mapping
    WHERE item_type = 'task' AND item_id = new.id AND folder_id = new.folder_id;
END;
CREATE TRIGGER IF NOT EXISTS task_update AFTER UPDATE OF title, description, folder_id, id ON tasks BEGIN
    DELETE FROM search_index WHERE rowid IN (
        SELECT id FROM search_mapping
        WHERE item_type = 'task' AND item_id = old.id AND folder_id = old.folder_id
    );
    DELETE FROM search_mapping
    WHERE item_type = 'task' AND item_id = old.id AND folder_id = old.folder_id;
    INSERT INTO search_mapping(item_type, item_id, folder_id)
    VALUES ('task', new.id, new.folder_id);
    INSERT INTO search_index(rowid, title, content, item_type, item_id)
    VALUES (last_insert_rowid(), new.title, new.description, 'task', new.id);
END;
CREATE TRIGGER IF NOT EXISTS task_delete AFTER DELETE ON tasks BEGIN
    DELETE FROM search_index
    WHERE rowid = (
        SELECT id from search_mapping
        WHERE item_type = 'task' AND item_id = old.id AND folder_id = old.folder_id
    );
    DELETE FROM search_mapping
    WHERE item_type = 'task' AND item_id = old.id AND folder_id = old.folder_id;
END;

-- fts5 search index trigger: update index when table `resources` changes
CREATE TRIGGER IF NOT EXISTS resource_add AFTER INSERT ON resources
WHEN new.type != 'task' BEGIN
    -- Subfolder-type resource rows must not take the 'folder' item_type of
    -- the folders-table rows, so their mapping gets its own item_type and
    -- search can target the inner folder directly (folder_id + item_id).
    INSERT OR IGNORE INTO search_mapping(item_type, item_id, folder_id)
    VALUES (CASE new.type WHEN 'folder' THEN 'subfolder' ELSE new.type END, new.resource_id, new.folder_id);
    INSERT OR REPLACE INTO search_index(rowid, title, content, item_type, item_id)
    SELECT id, new.title, coalesce(json_extract(new.data, '$.resource.description'),
        json_extract(new.data, '$.resource.body'), json_extract(new.data, '$.resource.url'), ''),
        CASE new.type WHEN 'folder' THEN 'subfolder' ELSE new.type END, new.resource_id
    FROM search_mapping
    WHERE item_type = CASE new.type WHEN 'folder' THEN 'subfolder' ELSE new.type END
        AND item_id = new.resource_id AND folder_id = new.folder_id;
END;
CREATE TRIGGER IF NOT EXISTS resource_update
AFTER UPDATE OF title, data, folder_id, resource_id, type ON resources BEGIN
    DELETE FROM search_index WHERE rowid IN (
        SELECT id FROM search_mapping
        WHERE item_type = CASE old.type WHEN 'folder' THEN 'subfolder' ELSE old.type END
            AND item_id = old.resource_id AND folder_id = old.folder_id
            AND old.type != 'task'
    );
    DELETE FROM search_mapping
    WHERE item_type = CASE old.type WHEN 'folder' THEN 'subfolder' ELSE old.type END
        AND item_id = old.resource_id AND folder_id = old.folder_id
        AND old.type != 'task';
    INSERT OR IGNORE INTO search_mapping(item_type, item_id, folder_id)
    SELECT CASE new.type WHEN 'folder' THEN 'subfolder' ELSE new.type END, new.resource_id, new.folder_id WHERE new.type != 'task';
    INSERT OR REPLACE INTO search_index(rowid, title, content, item_type, item_id)
    SELECT id, new.title, coalesce(json_extract(new.data, '$.resource.description'),
        json_extract(new.data, '$.resource.body'), json_extract(new.data, '$.resource.url'), ''),
        CASE new.type WHEN 'folder' THEN 'subfolder' ELSE new.type END, new.resource_id
    FROM search_mapping
    WHERE item_type = CASE new.type WHEN 'folder' THEN 'subfolder' ELSE new.type END
        AND item_id = new.resource_id AND folder_id = new.folder_id
        AND new.type != 'task';
END;
CREATE TRIGGER IF NOT EXISTS resource_delete AFTER DELETE ON resources
WHEN old.type != 'task' BEGIN
    DELETE FROM search_index WHERE rowid IN (
        SELECT id FROM search_mapping
        WHERE item_type = old.type AND item_id = old.resource_id AND folder_id = old.folder_id
    );
    DELETE FROM search_mapping
    WHERE item_type = old.type AND item_id = old.resource_id AND folder_id = old.folder_id;
END;

-- fts5 search index trigger: update index when table `folders` changes
CREATE TRIGGER IF NOT EXISTS folder_add AFTER INSERT ON folders BEGIN
    INSERT OR IGNORE INTO search_mapping(item_type, item_id, folder_id)
    VALUES ('folder', new.folder_id, new.folder_id);
    INSERT OR REPLACE INTO search_index(rowid, title, content, item_type, item_id)
    SELECT id, new.title, coalesce(json_extract(new.data, '$.description'), ''), 'folder', new.folder_id
    FROM search_mapping
    WHERE item_type = 'folder' AND item_id = new.folder_id AND folder_id = new.folder_id;
END;
CREATE TRIGGER IF NOT EXISTS folder_update
AFTER UPDATE OF title, data, folder_id ON folders BEGIN
    DELETE FROM search_index WHERE rowid IN (
        SELECT id FROM search_mapping
        WHERE item_type = 'folder' AND item_id = old.folder_id AND folder_id = old.folder_id
    );
    DELETE FROM search_mapping
    WHERE item_type = 'folder' AND item_id = old.folder_id AND folder_id = old.folder_id;
    INSERT INTO search_mapping(item_type, item_id, folder_id)
    VALUES ('folder', new.folder_id, new.folder_id);
    INSERT INTO search_index(rowid, title, content, item_type, item_id)
    VALUES (last_insert_rowid(), new.title, coalesce(json_extract(new.data, '$.description'), ''), 'folder', new.folder_id);
END;
CREATE TRIGGER IF NOT EXISTS folder_delete AFTER DELETE ON folders BEGIN
    DELETE FROM search_index WHERE rowid IN (
        SELECT id FROM search_mapping
        WHERE item_type = 'folder' AND item_id = old.folder_id AND folder_id = old.folder_id
    );
    DELETE FROM search_mapping
    WHERE item_type = 'folder' AND item_id = old.folder_id AND folder_id = old.folder_id;
END;
