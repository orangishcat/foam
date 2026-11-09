use std::path::Path;

use flexi_logger::writers::FileLogWriter;
use flexi_logger::{Age, Cleanup, Criterion, Duplicate, FileSpec, Logger, LoggerHandle, Naming};

pub fn init(data_dir: &Path) -> Result<LoggerHandle, flexi_logger::FlexiLoggerError> {
    logger(data_dir)?.start()
}

fn logger(data_dir: &Path) -> Result<Logger, flexi_logger::FlexiLoggerError> {
    let file_writer = FileLogWriter::builder(
        FileSpec::default()
            .directory(data_dir.join("logs"))
            .suppress_basename()
            .suppress_timestamp(),
    )
    .format(flexi_logger::detailed_format)
    .max_level(log::LevelFilter::Info)
    .append()
    .rotate(
        Criterion::Age(Age::Day),
        Naming::TimestampsCustomFormat {
            current_infix: None,
            format: "%Y-%m-%d",
        },
        // Compress all previous days without deleting older history.
        // The crate adds one to this limit for the active file.
        Cleanup::KeepCompressedFiles(usize::MAX - 1),
    )
    .try_build()?;
    Ok(Logger::try_with_str("debug")?
        .format(flexi_logger::detailed_format)
        .log_to_writer(Box::new(file_writer))
        .duplicate_to_stdout(Duplicate::Debug))
}
