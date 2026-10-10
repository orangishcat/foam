use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Error, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::{AppWindow, config::config, database, thread_manager, ui};

static NEXT_DOWNLOAD: AtomicU64 = AtomicU64::new(0);

pub fn asset_from_url(
    url: &str,
    extension: &str,
    on_complete: impl FnOnce(io::Result<PathBuf>) + Send + 'static,
) {
    let url = url.to_owned();
    let extension = extension.to_owned();
    thread_manager::spawn_thread("download asset", move || {
        let result = (|| -> io::Result<PathBuf> {
            if url.trim().is_empty() {
                log::debug!("Skipping empty asset URL");
                return Err(Error::new(io::ErrorKind::InvalidInput, "empty asset URL"));
            }
            match database::from_sql_map(
                "SELECT file_path FROM attachments WHERE url = ?".into(),
                &[&url],
                |row| row.get::<_, String>(0).map_err(Error::other),
            ) {
                Ok(paths) => {
                    if let Some(path) = paths.into_iter().next().map(PathBuf::from) {
                        if path.is_file() {
                            log::debug!("Asset cache hit for {url}: {}", path.display());
                            return Ok(path);
                        }
                        log::info!("Cached asset missing for {url}: {}", path.display());
                    }
                }
                Err(error) => log::warn!("Asset cache lookup failed for {url}: {error}"),
            }

            let extension = extension.trim_start_matches('.');
            let extension = if !extension.is_empty()
                && extension.len() <= 16
                && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
            {
                extension.to_ascii_lowercase()
            } else {
                // guess extension from url, e.g. /asdf/foo.txt?param=1 -> txt, falling back to empty string
                let after_dot = url.rsplit_once('.').map_or("", |(_, r)| r);
                let unchecked = after_dot.split_once('?').map_or(after_dot, |(l, _)| l).to_string();
                unchecked.find("/").map_or(unchecked, |_| "".to_string())
            };
            log::debug!("Downloading asset from {url}");
            let request = crate::account::active_account()?
                .get_resource(&url)
                .map_err(Error::other)?;
            let mut response = request
                .send()
                .map_err(Error::other)?
                .error_for_status()
                .map_err(Error::other)?;
            let max_bytes = config().max_attachment_bytes;
            if response
                .content_length()
                .is_some_and(|length| length > max_bytes)
            {
                return Err(Error::other(format!(
                    "asset exceeds configured limit of {max_bytes} bytes"
                )));
            }
            let mut bytes = Vec::new();
            response
                .by_ref()
                .take(max_bytes.saturating_add(1))
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > max_bytes {
                return Err(Error::other(format!(
                    "asset exceeds configured limit of {max_bytes} bytes"
                )));
            }
            let hash = format!("{:x}", Sha256::digest(&bytes));
            let mut path = config()
                .data_dir()
                .join("attachments")
                .join(&hash[..2])
                .join(&hash);
            if !extension.is_empty() {
                path.set_extension(&extension);
            } else if bytes.starts_with(b"%PDF-") {
                path.set_extension("pdf");
            }
            std::fs::create_dir_all(path.parent().expect("attachment has parent"))?;
            if !path.exists() {
                let temporary = path.with_extension(format!(
                    "{}.{}.tmp",
                    std::process::id(),
                    NEXT_DOWNLOAD.fetch_add(1, Ordering::Relaxed)
                ));

                // two step write then rename; partial write only creates an obviously failed cache write instead of corrupted cache file
                std::fs::write(&temporary, &bytes)?;
                std::fs::rename(&temporary, &path)?;
            }
            database::execute(
                "INSERT INTO attachments (url, file_path) VALUES (?, ?) ON CONFLICT(url) DO UPDATE SET file_path = excluded.file_path".into(),
                &[&url, &path.to_string_lossy().to_string()],
            )?;
            Ok(path)
        })().inspect_err(|err| log::warn!("Downloading asset from {url} failed: {err}"));
        on_complete(result);
    }).inspect_err(|err| log::warn!("thread spawning failed: {err}")).ok();
}

pub fn asset_as_slint_img(
    url: &str,
    extension: &str,
    on_complete: impl FnOnce(&AppWindow, io::Result<slint::Image>) + Send + 'static,
) {
    asset_from_url(url, extension, move |result| {
        ui::run_on_ui_thread(|ui| match result {
            Ok(path) => on_complete(
                &ui,
                slint::Image::load_from_path(&path).map_err(io::Error::other),
            ),
            Err(err) => on_complete(&ui, Err(err)),
        });
    });
}

pub(super) fn read_json<T>(path: &Path) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let reader = BufReader::new(File::open(path)?);
    serde_json::from_reader(reader).map_err(Error::other)
}

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    create_if_missing(path)?;
    let writer = BufWriter::new(OpenOptions::new().write(true).truncate(true).open(path)?);
    serde_json::to_writer_pretty(writer, value)?;
    Ok(())
}

pub(super) fn create_if_missing(path: &Path) -> io::Result<()> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}
