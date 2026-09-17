use crate::{
    api::schoology::{self, RequestResult},
    database,
};

/// Retry an interrupted initial fetch even if some courses were already saved.
pub fn ensure_loaded() -> RequestResult<()> {
    let loaded = database::from_sql_map(
        "SELECT data FROM sync_state WHERE key = 'courses_loaded'".to_owned(),
        &[],
        |row| row.get::<_, String>(0).map_err(std::io::Error::other),
    )?;
    if loaded.is_empty() {
        schoology::course::courses::scrape_courses()?;
        database::execute("INSERT INTO sync_state (key, data) VALUES ('courses_loaded', 'true') ON CONFLICT(key) DO NOTHING".to_owned(), &[])?;
    }
    Ok(())
}
