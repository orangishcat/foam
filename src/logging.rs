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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Days, Local};
    use std::{fs, time::SystemTime};

    #[test]
    fn appends_today_and_compresses_previous_days() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let data_dir =
            std::env::temp_dir().join(format!("foam-logs-{}-{unique}", std::process::id()));
        let logs = data_dir.join("logs");
        fs::create_dir_all(&logs).unwrap();
        let today = Local::now().date_naive();
        let yesterday = today.checked_sub_days(Days::new(1)).unwrap();
        let current = logs.join(format!("{today}.log"));
        let old = logs.join(format!("{yesterday}.log"));
        fs::write(&current, "existing entry\n").unwrap();
        fs::write(&old, "yesterday's entry\n").unwrap();

        for _ in 0..2 {
            let (logger, handle) = logger(&data_dir).unwrap().build().unwrap();
            assert!(logger.enabled(&log::Metadata::builder().level(log::Level::Debug).build()));
            assert!(!logger.enabled(&log::Metadata::builder().level(log::Level::Trace).build()));
            logger.log(
                &log::Record::builder()
                    .level(log::Level::Debug)
                    .target("foam::logging::tests")
                    .args(format_args!("stdout-only entry"))
                    .build(),
            );
            logger.log(
                &log::Record::builder()
                    .level(log::Level::Info)
                    .target("foam::logging::tests")
                    .args(format_args!("new entry"))
                    .build(),
            );
            logger.flush();
            handle.shutdown();
        }

        let contents = fs::read_to_string(&current).unwrap();
        assert!(contents.starts_with("existing entry\n"));
        assert_eq!(contents.matches("new entry").count(), 2);
        assert!(!contents.contains("stdout-only entry"));
        assert!(!old.exists());
        let compressed = fs::read(logs.join(format!("{yesterday}.log.gz"))).unwrap();
        assert_eq!(&compressed[..2], &[0x1f, 0x8b]);
        assert_eq!(fs::read_dir(&logs).unwrap().count(), 2);
        fs::remove_dir_all(data_dir).unwrap();
    }
}
