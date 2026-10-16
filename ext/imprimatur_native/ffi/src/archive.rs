//! `Imprimatur::Native::ZipWriter` and `ZipReader`: zip archives of any
//! files, compressed and read without the GVL.

use std::cell::RefCell;
use std::fs::File;

use imprimatur::archive::{Archive, Writer};
use magnus::{Error, RString, Ruby};

use crate::blocking::without_gvl;
use crate::{OrRaise, borrow};

#[magnus::wrap(class = "Imprimatur::Native::ZipWriter", free_immediately, size)]
pub struct ZipWriter(RefCell<Option<Writer<File>>>);

fn finished(ruby: &Ruby) -> Error {
    Error::new(ruby.exception_runtime_error(), "the archive is finished")
}

#[expect(clippy::needless_pass_by_value, reason = "magnus passes Ruby arguments as owned values")]
impl ZipWriter {
    /// A new archive at `path`.
    pub fn new(ruby: &Ruby, path: String) -> Result<Self, Error> {
        Ok(Self(RefCell::new(Some(Writer::create(path).or_raise(ruby)?))))
    }

    /// Adds the file at `source` as `name`, streamed.
    pub fn add_file(
        ruby: &Ruby,
        rb_self: &Self,
        name: String,
        source: String,
        comment: Option<String>,
    ) -> Result<(), Error> {
        let mut slot = borrow(ruby, &rb_self.0, "zip writer")?;
        let writer = slot.as_mut().ok_or_else(|| finished(ruby))?;
        without_gvl(ruby, || writer.add_file(&name, &source, comment.as_deref()))?.or_raise(ruby)
    }

    /// Adds `data` as `name`.
    pub fn add(
        ruby: &Ruby,
        rb_self: &Self,
        name: String,
        data: RString,
        comment: Option<String>,
    ) -> Result<(), Error> {
        let data = crate::bytes(data);
        let mut slot = borrow(ruby, &rb_self.0, "zip writer")?;
        let writer = slot.as_mut().ok_or_else(|| finished(ruby))?;
        without_gvl(ruby, || writer.add(&name, &data, comment.as_deref()))?.or_raise(ruby)
    }

    /// The archive's own comment.
    pub fn set_comment(ruby: &Ruby, rb_self: &Self, comment: String) -> Result<(), Error> {
        let mut slot = borrow(ruby, &rb_self.0, "zip writer")?;
        slot.as_mut().ok_or_else(|| finished(ruby))?.set_comment(&comment).or_raise(ruby)
    }

    /// Writes the archive's directory; the writer takes no more entries.
    pub fn finish(ruby: &Ruby, rb_self: &Self) -> Result<(), Error> {
        let writer =
            borrow(ruby, &rb_self.0, "zip writer")?.take().ok_or_else(|| finished(ruby))?;
        without_gvl(ruby, || writer.finish())?.or_raise(ruby).map(drop)
    }
}

/// An entry for Ruby: name, size, compressed size, CRC-32, comment.
type EntryRow = (String, u64, u64, u32, String);

#[magnus::wrap(class = "Imprimatur::Native::ZipReader", free_immediately, size)]
pub struct ZipReader(RefCell<Archive<File>>);

#[expect(clippy::needless_pass_by_value, reason = "magnus passes Ruby arguments as owned values")]
impl ZipReader {
    /// The archive at `path`.
    pub fn new(ruby: &Ruby, path: String) -> Result<Self, Error> {
        Ok(Self(RefCell::new(Archive::open(path).or_raise(ruby)?)))
    }

    pub fn names(ruby: &Ruby, rb_self: &Self) -> Result<Vec<String>, Error> {
        Ok(borrow(ruby, &rb_self.0, "zip archive")?.names())
    }

    /// Each entry's name, size, compressed size, CRC-32 and comment.
    pub fn entries(ruby: &Ruby, rb_self: &Self) -> Result<Vec<EntryRow>, Error> {
        let entries = borrow(ruby, &rb_self.0, "zip archive")?.entries().or_raise(ruby)?;
        Ok(entries
            .into_iter()
            .map(|entry| {
                (entry.name, entry.size, entry.compressed_size, entry.crc32, entry.comment)
            })
            .collect())
    }

    /// The entry's bytes, `nil` when there is none.
    pub fn read(
        ruby: &Ruby,
        rb_self: &Self,
        name: String,
        limit: u64,
    ) -> Result<Option<RString>, Error> {
        let mut archive = borrow(ruby, &rb_self.0, "zip archive")?;
        let data = without_gvl(ruby, || archive.read(&name, limit))?.or_raise(ruby)?;
        Ok(data.map(|data| ruby.str_from_slice(&data)))
    }

    /// Writes the entry to `path`; whether there was one.
    pub fn extract(
        ruby: &Ruby,
        rb_self: &Self,
        name: String,
        path: String,
        limit: u64,
    ) -> Result<bool, Error> {
        let mut archive = borrow(ruby, &rb_self.0, "zip archive")?;
        without_gvl(ruby, || archive.extract(&name, &path, limit))?.or_raise(ruby)
    }
}
