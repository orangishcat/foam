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
CREATE INDEX IF NOT EXISTS assignments_id ON assignments(id);
CREATE INDEX IF NOT EXISTS materials_parent ON materials(course_id, parent_id);
CREATE TRIGGER IF NOT EXISTS assignment_title_updated AFTER UPDATE OF title ON assignments
BEGIN
    UPDATE materials SET title = NEW.title
    WHERE course_id = NEW.course_id AND type = 'assignment' AND material_id = NEW.id;
END;
