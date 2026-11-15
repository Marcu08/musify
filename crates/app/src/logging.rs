//! File logging for launches without a terminal (desktop/Start Menu
//! shortcuts). Logs still go to stdout, but they are also appended to a
//! size-capped rotating file under the application data directory.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing_subscriber::fmt::writer::MakeWriter;

pub(crate) const MAX_BYTES: u64 = 2 * 1024 * 1024;
pub(crate) const MAX_FILES: usize = 5;
const FILE_NAME: &str = "musify.log";

pub(crate) struct RotatingLogs {
    dir: PathBuf,
    max_bytes: u64,
    max_files: usize,
    state: Mutex<LogState>,
}

struct LogState {
    file: Option<File>,
    size: u64,
}

impl RotatingLogs {
    pub(crate) fn new(dir: PathBuf, max_bytes: u64, max_files: usize) -> io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let (file, size) = open_log(&dir)?;
        let logs = Self {
            dir,
            max_bytes: max_bytes.max(1),
            max_files: max_files.max(1),
            state: Mutex::new(LogState {
                file: Some(file),
                size,
            }),
        };
        if size >= logs.max_bytes {
            let mut state = logs
                .state
                .lock()
                .map_err(|_| io::Error::other("log lock poisoned"))?;
            state.rotate(&logs.dir, logs.max_files)?;
        }
        Ok(logs)
    }
}

impl<'a> MakeWriter<'a> for RotatingLogs {
    type Writer = LogWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter { logs: self }
    }
}

pub(crate) struct LogWriter<'a> {
    logs: &'a RotatingLogs,
}

impl Write for LogWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self
            .logs
            .state
            .lock()
            .map_err(|_| io::Error::other("log lock poisoned"))?;
        if state.file.is_none() {
            let (file, size) = open_log(&self.logs.dir)?;
            state.file = Some(file);
            state.size = size;
        }
        let incoming = u64::try_from(buf.len()).unwrap_or(u64::MAX);
        if state.size > 0 && state.size.saturating_add(incoming) > self.logs.max_bytes {
            state.rotate(&self.logs.dir, self.logs.max_files)?;
        }
        let file = state
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file unavailable"))?;
        file.write_all(buf)?;
        state.size = state.size.saturating_add(incoming);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut state = self
            .logs
            .state
            .lock()
            .map_err(|_| io::Error::other("log lock poisoned"))?;
        if let Some(file) = state.file.as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}

impl LogState {
    fn rotate(&mut self, dir: &Path, max_files: usize) -> io::Result<()> {
        self.file.take();
        let max_files = max_files.max(1);
        let base = dir.join(FILE_NAME);
        if max_files == 1 {
            let _ = std::fs::remove_file(&base);
        } else {
            let _ = std::fs::remove_file(archive(dir, max_files - 1));
            for index in (1..max_files - 1).rev() {
                let from = archive(dir, index);
                if !from.exists() {
                    continue;
                }
                let to = archive(dir, index + 1);
                let _ = std::fs::remove_file(&to);
                std::fs::rename(&from, &to)?;
            }
            if base.exists() {
                let first = archive(dir, 1);
                let _ = std::fs::remove_file(&first);
                std::fs::rename(&base, &first)?;
            }
        }
        let (file, size) = open_log(dir)?;
        self.file = Some(file);
        self.size = size;
        Ok(())
    }
}

fn open_log(dir: &Path) -> io::Result<(File, u64)> {
    let path = dir.join(FILE_NAME);
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    let size = file.metadata()?.len();
    Ok((file, size))
}

fn archive(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("musify.{index}.log"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::io::Write;

    use super::*;

    fn write(logs: &RotatingLogs, bytes: &[u8]) {
        logs.make_writer()
            .write_all(bytes)
            .expect("write log bytes");
    }

    #[test]
    fn rotation_moves_the_current_file_and_keeps_the_limit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let logs = RotatingLogs::new(dir.path().to_path_buf(), 10, 2).expect("logs");
        write(&logs, b"abcdef");
        assert_eq!(
            std::fs::read(dir.path().join("musify.log")).expect("base"),
            b"abcdef"
        );
        write(&logs, b"ghijkl");
        assert_eq!(
            std::fs::read(dir.path().join("musify.log")).expect("base"),
            b"ghijkl"
        );
        assert_eq!(
            std::fs::read(dir.path().join("musify.1.log")).expect("archive"),
            b"abcdef"
        );
        write(&logs, b"mnopqr");
        assert_eq!(
            std::fs::read(dir.path().join("musify.log")).expect("base"),
            b"mnopqr"
        );
        assert_eq!(
            std::fs::read(dir.path().join("musify.1.log")).expect("archive"),
            b"ghijkl"
        );
        assert!(!dir.path().join("musify.2.log").exists());
    }

    #[test]
    fn a_new_writer_appends_to_an_existing_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        {
            let logs = RotatingLogs::new(dir.path().to_path_buf(), 100, 3).expect("logs");
            write(&logs, b"abc");
        }
        let logs = RotatingLogs::new(dir.path().to_path_buf(), 100, 3).expect("logs");
        write(&logs, b"def");
        assert_eq!(
            std::fs::read(dir.path().join("musify.log")).expect("base"),
            b"abcdef"
        );
    }

    #[test]
    fn one_file_mode_replaces_instead_of_archiving() {
        let dir = tempfile::tempdir().expect("temp dir");
        let logs = RotatingLogs::new(dir.path().to_path_buf(), 10, 1).expect("logs");
        write(&logs, b"abcdef");
        write(&logs, b"ghijkl");
        assert_eq!(
            std::fs::read(dir.path().join("musify.log")).expect("base"),
            b"ghijkl"
        );
        assert!(!dir.path().join("musify.1.log").exists());
    }
}
