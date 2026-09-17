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
    due STRING NOT NULL,
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
    created STRING NOT NULL,
    resource_id STRING NOT NULL,
    material_type STRING,
    course_id STRING REFERENCES courses(course_id),
    course_title STRING,
    PRIMARY KEY (id),
);
