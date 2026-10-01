use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Error, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use reqwest::blocking::RequestBuilder;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::{config::config, database};

static NEXT_DOWNLOAD: AtomicU64 = AtomicU64::new(0);

pub fn load_slint_img(path: &Path) -> io::Result<slint::Image> {
    let bytes = std::fs::read(path)?;
    slint::Image::load_from_data(&bytes, None).map_err(Error::other)
}

pub fn load_slint_img_thumbnail(path: &Path, size: u32) -> io::Result<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") {
        let tree = resvg::usvg::Tree::from_data(&bytes, &resvg::usvg::Options::default())
            .map_err(Error::other)?;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
            .ok_or_else(|| Error::other("invalid image thumbnail size"))?;
        let source = tree.size();
        let scale = (size as f32 / source.width()).min(size as f32 / source.height());
        let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(
            (size as f32 - source.width() * scale) / 2.0,
            (size as f32 - source.height() * scale) / 2.0,
        );
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        pixmap.encode_png().map_err(Error::other)
    } else {
        let image = image::load_from_memory(&bytes).map_err(Error::other)?;
        let image = image.resize_to_fill(size, size, image::imageops::FilterType::Lanczos3);
        let mut output = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut output, image::ImageFormat::Png)
            .map_err(Error::other)?;
        Ok(output.into_inner())
    }
}

pub fn asset_from_url(
    request: RequestBuilder,
    url: &str,
    on_download_complete: impl FnOnce(PathBuf) + Send + 'static,
) -> Option<PathBuf> {
    if url.trim().is_empty() {
        log::debug!("Skipping empty asset URL");
        return None;
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
                    return Some(path);
                }
                log::info!("Cached asset missing for {url}: {}", path.display());
            }
        }
        Err(error) => log::warn!("Asset cache lookup failed for {url}: {error}"),
    }

    let url = url.to_owned();
    std::thread::spawn(move || {
        log::debug!("Downloading asset from {url}");
        let result = (|| -> io::Result<PathBuf> {
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
            let path = config()
                .data_dir()
                .join("attachments")
                .join(&hash[..2])
                .join(&hash);
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
        })();
        match result {
            Ok(path) => {
                log::info!("Downloaded asset from {url} to {}", path.display());
                on_download_complete(path);
            }
            Err(error) => log::warn!("Downloading asset from {url} failed: {error}"),
        }
    });
    None
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
