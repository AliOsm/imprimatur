//! Zip archives of any files, for what is not a document.
//!
//! Entries are written from disk (streamed) or from memory, with their
//! modification times and comments, ZIP64 when an entry is large; they are
//! read one by one, each within a size limit. Behind the `archive` feature,
//! which brings in the `zip` crate; documents' own zips (XLSX, DOCX) never
//! need it.
//!
//! ```
//! use std::io::Cursor;
//! use imprimatur::archive::{Archive, Writer};
//!
//! let mut writer = Writer::new(Cursor::new(Vec::new()));
//! writer.add("notes/readme.txt", b"hello", Some("first"))?;
//! let bytes = writer.finish()?.into_inner();
//!
//! let mut archive = Archive::new(Cursor::new(bytes))?;
//! assert_eq!(archive.names(), ["notes/readme.txt"]);
//! assert_eq!(archive.read("notes/readme.txt", 1024)?.as_deref(), Some(&b"hello"[..]));
//! # Ok::<(), imprimatur::Error>(())
//! ```

#[cfg(test)]
mod tests;

use std::fs::File;
use std::io::{self, Read, Seek, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ::zip::result::ZipError;
use ::zip::write::FullFileOptions;
use ::zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::xlsx::civil_from_days;
use crate::{Error, Result};

/// An archive being written; [`finish`](Self::finish) writes its directory.
pub struct Writer<W: Write + Seek> {
    zip: ZipWriter<W>,
}

impl Writer<File> {
    /// A new archive at `path`, replacing any file there.
    ///
    /// # Errors
    /// [`Error::Io`] when the file cannot be created.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::new(File::create(path)?))
    }
}

impl<W: Write + Seek> Writer<W> {
    pub fn new(sink: W) -> Self {
        Self { zip: ZipWriter::new(sink) }
    }

    /// Adds the file at `source` as `name`, streamed from disk, with its
    /// modification time.
    ///
    /// # Errors
    /// [`Error::Io`] when the file cannot be read or the archive written.
    pub fn add_file(
        &mut self,
        name: &str,
        source: impl AsRef<Path>,
        comment: Option<&str>,
    ) -> Result<()> {
        let mut file = File::open(source)?;
        let metadata = file.metadata()?;
        self.zip
            .start_file(name, options(metadata.modified().ok(), metadata.len(), comment))
            .map_err(error)?;
        io::copy(&mut file, &mut self.zip)?;
        Ok(())
    }

    /// Adds `data` as `name`, dated now.
    ///
    /// # Errors
    /// [`Error::Io`] when the archive cannot be written.
    pub fn add(&mut self, name: &str, data: &[u8], comment: Option<&str>) -> Result<()> {
        let size = u64::try_from(data.len()).unwrap_or(u64::MAX);
        self.zip
            .start_file(name, options(Some(SystemTime::now()), size, comment))
            .map_err(error)?;
        self.zip.write_all(data)?;
        Ok(())
    }

    /// The archive's own comment, shown by most zip tools.
    ///
    /// # Errors
    /// [`Error::Malformed`] for a comment over 65,535 bytes.
    pub fn set_comment(&mut self, comment: &str) -> Result<()> {
        self.zip.set_comment(comment).map_err(error)
    }

    /// Writes the central directory and gives the sink back.
    ///
    /// # Errors
    /// [`Error::Io`] when it cannot be written.
    pub fn finish(self) -> Result<W> {
        self.zip.finish().map_err(error)
    }
}

/// How an entry is written: deflated, dated, ZIP64 when it needs to be.
fn options(
    modified: Option<SystemTime>,
    size: u64,
    comment: Option<&str>,
) -> FullFileOptions<'static, 'static> {
    let options = FullFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(dos_time(modified))
        .large_file(size >= u64::from(u32::MAX));
    match comment {
        Some(comment) => options.with_file_comment(comment),
        None => options,
    }
}

/// A time as a zip entry's (UTC, two-second steps, 1980 to 2107); the
/// format's epoch outside that range.
fn dos_time(time: Option<SystemTime>) -> DateTime {
    let Some(seconds) =
        time.and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map(|since| since.as_secs())
    else {
        return DateTime::default();
    };
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let (year, month, day) = civil_from_days(days);
    let clock = seconds % 86_400;
    let part = |value: u64| u8::try_from(value).unwrap_or(0);
    let (month, day) = (u8::try_from(month).unwrap_or(1), u8::try_from(day).unwrap_or(1));
    u16::try_from(year)
        .ok()
        .and_then(|year| {
            DateTime::from_date_and_time(
                year,
                month,
                day,
                part(clock / 3600),
                part(clock / 60 % 60),
                part(clock % 60),
            )
            .ok()
        })
        .unwrap_or_default()
}

/// What the archive's directory says of an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// Its size once decompressed, as the directory states it.
    pub size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub comment: String,
}

/// An archive being read.
pub struct Archive<R: Read + Seek> {
    zip: ZipArchive<R>,
}

impl Archive<File> {
    /// The archive at `path`.
    ///
    /// # Errors
    /// [`Error::Io`] when it cannot be opened, [`Error::Malformed`] when it
    /// is no zip.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(File::open(path)?)
    }
}

impl<R: Read + Seek> Archive<R> {
    /// # Errors
    /// [`Error::Malformed`] when `source` is no zip.
    pub fn new(source: R) -> Result<Self> {
        Ok(Self { zip: ZipArchive::new(source).map_err(error)? })
    }

    /// The entries' names, in the archive's order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.zip
            .file_names()
            .filter_map(std::result::Result::ok)
            .map(std::borrow::Cow::into_owned)
            .collect()
    }

    /// Every entry's name, sizes, checksum and comment, in the archive's
    /// order, without decompressing anything.
    ///
    /// # Errors
    /// [`Error::Malformed`] for a damaged directory.
    pub fn entries(&mut self) -> Result<Vec<Entry>> {
        (0..self.zip.len())
            .map(|index| {
                let entry = self.zip.by_index_raw(index).map_err(error)?;
                Ok(Entry {
                    name: entry.name().map_err(error)?.into_owned(),
                    size: entry.size(),
                    compressed_size: entry.compressed_size(),
                    crc32: entry.crc32(),
                    comment: entry.comment().to_owned(),
                })
            })
            .collect()
    }

    /// The entry `name`'s bytes, `None` when there is none.
    ///
    /// # Errors
    /// [`Error::LimitExceeded`] past `limit` bytes, [`Error::PasswordRequired`]
    /// for an encrypted entry, [`Error::Malformed`] for a damaged one.
    pub fn read(&mut self, name: &str, limit: u64) -> Result<Option<Vec<u8>>> {
        let mut out = Vec::new();
        self.copy(name, &mut out, limit).map(|found| found.then_some(out))
    }

    /// Writes the entry `name` to the file at `path`; whether there was one.
    ///
    /// # Errors
    /// As [`read`](Self::read), and [`Error::Io`] for the file.
    pub fn extract(&mut self, name: &str, path: impl AsRef<Path>, limit: u64) -> Result<bool> {
        if !self.names().iter().any(|entry| entry == name) {
            return Ok(false);
        }
        let mut file = File::create(path)?;
        self.copy(name, &mut file, limit)
    }

    /// Copies an entry's bytes to `out`, at most `limit` of them.
    fn copy(&mut self, name: &str, out: &mut impl Write, limit: u64) -> Result<bool> {
        let entry = match self.zip.by_name(name) {
            Ok(entry) => entry,
            Err(ZipError::FileNotFound) => return Ok(false),
            Err(other) => return Err(error(other)),
        };
        // One byte past the limit tells a full entry from a cut one.
        let copied = io::copy(&mut entry.take(limit.saturating_add(1)), out)?;
        if copied > limit {
            return Err(Error::LimitExceeded("decompressed size"));
        }
        Ok(true)
    }
}

impl<W: Write + Seek> std::fmt::Debug for Writer<W> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Writer").finish_non_exhaustive()
    }
}

impl<R: Read + Seek> std::fmt::Debug for Archive<R> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Archive").field("entries", &self.zip.len()).finish()
    }
}

fn error(error: ZipError) -> Error {
    match error {
        ZipError::Io(error) => Error::Io(error),
        ZipError::InvalidPassword => Error::PasswordRequired,
        ZipError::UnsupportedArchive(reason) if reason.contains("assword") => {
            Error::PasswordRequired
        }
        ZipError::UnsupportedArchive(reason) => {
            Error::UnsupportedDocument(format!("a zip entry: {reason}"))
        }
        other => Error::malformed("zip", other.to_string()),
    }
}
