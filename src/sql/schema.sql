-- tables
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS materials (
    course_id TEXT NOT NULL REFERENCES courses(course_id) ON DELETE CASCADE,
    material_id TEXT NOT NULL,
    type TEXT NOT NULL,
    parent_id TEXT NOT NULL,
    title TEXT NOT NULL,
    data TEXT NOT NULL,
    PRIMARY KEY (course_id, type, material_id)
);
CREATE TABLE IF NOT EXISTS courses (
    course_id TEXT PRIMARY KEY NOT NULL,
    aliases TEXT NOT NULL,
    course_title TEXT NOT NULL,
    course_code TEXT NOT NULL,
    course_url TEXT NOT NULL,
    section_title TEXT NOT NULL,
    section_code TEXT NOT NULL,
    active INTEGER NOT NULL,
    period STRING NOT NULL,
    hidden INTEGER NOT NULL,
    course_order INTEGER NOT NULL,
    description TEXT NOT NULL,
    logo_img_src TEXT NOT NULL,
    location TEXT NOT NULL,
    meeting_days TEXT NOT NULL,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    weight TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS assignments (
    course_id TEXT NOT NULL,
    id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    due TEXT NOT NULL,
    manual_mark INTEGER,
    max_points REAL NOT NULL,
    score REAL,
    letter_grade TEXT,
    allow_submissions INTEGER NOT NULL,
    attachments TEXT NOT NULL,
    submissions TEXT NOT NULL,
    type TEXT GENERATED ALWAYS AS ('assignment') VIRTUAL,
    PRIMARY KEY (course_id, id),
    FOREIGN KEY (course_id, type, id)
        REFERENCES materials(course_id, type, material_id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS notifications (
    id TEXT NOT NULL,
    manual_mark INTEGER,
    event TEXT NOT NULL,
    title TEXT NOT NULL,
    viewed INTEGER NOT NULL,
    is_processed INTEGER NOT NULL,
    created TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    material_type TEXT,
    course_id TEXT NOT NULL,
    course_title TEXT,
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
CREATE INDEX IF NOT EXISTS assignments_id ON assignments(id);
CREATE INDEX IF NOT EXISTS materials_parent ON materials(course_id, parent_id);

CREATE TRIGGER IF NOT EXISTS assignment_title_updated AFTER UPDATE OF title ON assignments
BEGIN
    UPDATE materials SET title = NEW.title
    WHERE course_id = NEW.course_id AND type = 'assignment' AND material_id = NEW.id;
END;

-- fts5 search mapppings
CREATE TABLE IF NOT EXISTS search_mapping (
    id INTEGER PRIMARY KEY,
    item_type TEXT NOT NULL,
    item_id TEXT NOT NULL,
    course_id TEXT NOT NULL,
    UNIQUE(course_id, item_type, item_id)
);

-- fts5 search index (supports assignments, materials, courses)
CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5 (
    title,
    content,
    item_type UNINDEXED,
    tokenize="porter unicode61"
);

-- fts5 search index trigger: update index when table `assignments` changes
CREATE TRIGGER IF NOT EXISTS assignment_add AFTER INSERT ON assignments BEGIN
    INSERT OR IGNORE INTO search_mapping(item_type, item_id, course_id)
    VALUES ('assignment', new.id, new.course_id);
    INSERT OR REPLACE INTO search_index(rowid, title, content, item_type)
    SELECT id, new.title, new.description, 'assignment' FROM search_mapping
    WHERE item_type = 'assignment' AND item_id = new.id AND course_id = new.course_id;
END;
CREATE TRIGGER IF NOT EXISTS assignment_update AFTER UPDATE OF title, description, course_id, id ON assignments BEGIN
    DELETE FROM search_index WHERE rowid IN (
        SELECT id FROM search_mapping
        WHERE item_type = 'assignment' AND item_id = old.id AND course_id = old.course_id
    );
    DELETE FROM search_mapping
    WHERE item_type = 'assignment' AND item_id = old.id AND course_id = old.course_id;
    INSERT INTO search_mapping(item_type, item_id, course_id)
    VALUES ('assignment', new.id, new.course_id);
    INSERT INTO search_index(rowid, title, content, item_type)
    VALUES (last_insert_rowid(), new.title, new.description, 'assignment');
END;
CREATE TRIGGER IF NOT EXISTS assignment_delete AFTER DELETE ON assignments BEGIN
    DELETE FROM search_index
    WHERE rowid = (
        SELECT id from search_mapping
        WHERE item_type = 'assignment' AND item_id = old.id AND course_id = old.course_id
    );
    DELETE FROM search_mapping
    WHERE item_type = 'assignment' AND item_id = old.id AND course_id = old.course_id;
END;

-- fts5 search index trigger: update index when table `materials` changes
CREATE TRIGGER IF NOT EXISTS material_add AFTER INSERT ON materials
WHEN new.type != 'assignment' BEGIN
    INSERT OR IGNORE INTO search_mapping(item_type, item_id, course_id)
    VALUES (new.type, new.material_id, new.course_id);
    INSERT OR REPLACE INTO search_index(rowid, title, content, item_type)
    SELECT id, new.title, coalesce(json_extract(new.data, '$.material.description'),
        json_extract(new.data, '$.material.body'), json_extract(new.data, '$.material.url'), ''), 'document'
    FROM search_mapping
    WHERE item_type = new.type AND item_id = new.material_id AND course_id = new.course_id;
END;
CREATE TRIGGER IF NOT EXISTS material_update
AFTER UPDATE OF title, data, course_id, material_id, type ON materials BEGIN
    DELETE FROM search_index WHERE rowid IN (
        SELECT id FROM search_mapping
        WHERE item_type = old.type AND item_id = old.material_id AND course_id = old.course_id
            AND old.type != 'assignment'
    );
    DELETE FROM search_mapping
    WHERE item_type = old.type AND item_id = old.material_id AND course_id = old.course_id
        AND old.type != 'assignment';
    INSERT OR IGNORE INTO search_mapping(item_type, item_id, course_id)
    SELECT new.type, new.material_id, new.course_id WHERE new.type != 'assignment';
    INSERT OR REPLACE INTO search_index(rowid, title, content, item_type)
    SELECT id, new.title, coalesce(json_extract(new.data, '$.material.description'),
        json_extract(new.data, '$.material.body'), json_extract(new.data, '$.material.url'), ''), 'document'
    FROM search_mapping
    WHERE item_type = new.type AND item_id = new.material_id AND course_id = new.course_id
        AND new.type != 'assignment';
END;
CREATE TRIGGER IF NOT EXISTS material_delete AFTER DELETE ON materials
WHEN old.type != 'assignment' BEGIN
    DELETE FROM search_index WHERE rowid IN (
        SELECT id FROM search_mapping
        WHERE item_type = old.type AND item_id = old.material_id AND course_id = old.course_id
    );
    DELETE FROM search_mapping
    WHERE item_type = old.type AND item_id = old.material_id AND course_id = old.course_id;
END;

-- fts5 search index trigger: update index when table `courses` changes
CREATE TRIGGER IF NOT EXISTS course_add AFTER INSERT ON courses BEGIN
    INSERT OR IGNORE INTO search_mapping(item_type, item_id, course_id)
    VALUES ('course', new.course_id, new.course_id);
    INSERT OR REPLACE INTO search_index(rowid, title, content, item_type)
    SELECT id, new.course_title, new.description, 'course' FROM search_mapping
    WHERE item_type = 'course' AND item_id = new.course_id AND course_id = new.course_id;
END;
CREATE TRIGGER IF NOT EXISTS course_update
AFTER UPDATE OF course_title, description, course_id ON courses BEGIN
    DELETE FROM search_index WHERE rowid IN (
        SELECT id FROM search_mapping
        WHERE item_type = 'course' AND item_id = old.course_id AND course_id = old.course_id
    );
    DELETE FROM search_mapping
    WHERE item_type = 'course' AND item_id = old.course_id AND course_id = old.course_id;
    INSERT INTO search_mapping(item_type, item_id, course_id)
    VALUES ('course', new.course_id, new.course_id);
    INSERT INTO search_index(rowid, title, content, item_type)
    VALUES (last_insert_rowid(), new.course_title, new.description, 'course');
END;
CREATE TRIGGER IF NOT EXISTS course_delete AFTER DELETE ON courses BEGIN
    DELETE FROM search_index WHERE rowid IN (
        SELECT id FROM search_mapping
        WHERE item_type = 'course' AND item_id = old.course_id AND course_id = old.course_id
    );
    DELETE FROM search_mapping
    WHERE item_type = 'course' AND item_id = old.course_id AND course_id = old.course_id;
END;
