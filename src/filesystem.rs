use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Error},
    path::{Path, PathBuf},
};

use log::{error, warn};
use serde::{Serialize, de::DeserializeOwned};

use crate::{config::config, types::course::Course};

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
