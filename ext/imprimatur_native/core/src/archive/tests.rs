use std::assert_matches;
use std::io::Cursor;
use std::time::Duration;

use super::*;

fn archive(entries: &[(&str, &[u8], Option<&str>)]) -> Vec<u8> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    for (name, data, comment) in entries {
        writer.add(name, data, *comment).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn writes_what_it_reads() {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer.add("a/one.txt", b"first", Some("a comment")).unwrap();
    writer.add("two.csv", b"x,y\n1,2\n", None).unwrap();
    writer.set_comment("an export").unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let mut read = Archive::new(Cursor::new(bytes.clone())).unwrap();
    let entries = read.entries().unwrap();
    let first = Entry {
        name: "a/one.txt".into(),
        size: 5,
        compressed_size: entries[0].compressed_size,
        crc32: crc32fast::hash(b"first"),
        comment: "a comment".into(),
    };
    assert_eq!((entries.len(), &entries[0]), (2, &first));
    assert_eq!(read.names(), ["a/one.txt", "two.csv"]);
    assert_eq!(read.read("two.csv", 1024).unwrap().as_deref(), Some(&b"x,y\n1,2\n"[..]));
    assert_eq!(read.read("missing", 1024).unwrap(), None);
    // An independent look at what was written: the comment and the method.
    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert_eq!(zip.comment(), b"an export");
    let entry = zip.by_name("a/one.txt").unwrap();
    assert_eq!((entry.comment(), entry.compression()), ("a comment", CompressionMethod::Deflated));
}

#[test]
fn streams_files_with_their_times() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("contract.pdf");
    std::fs::write(&source, vec![b'%'; 100_000]).unwrap();
    let modified = UNIX_EPOCH + Duration::from_mins(27_981_210); // 2023-03-15 09:30:00 UTC
    File::options().write(true).open(&source).unwrap().set_modified(modified).unwrap();
    let target = directory.path().join("export.zip");
    let mut writer = Writer::create(&target).unwrap();
    writer.add_file("contracts/0contract.pdf", &source, Some("Contract - signed")).unwrap();
    writer.finish().unwrap();

    let mut read = Archive::open(&target).unwrap();
    let extracted = directory.path().join("out.pdf");
    assert!(read.extract("contracts/0contract.pdf", &extracted, 1 << 20).unwrap());
    assert_eq!(std::fs::read(&extracted).unwrap(), std::fs::read(&source).unwrap());
    assert!(!read.extract("nope", directory.path().join("nope"), 1 << 20).unwrap());
    let mut zip = ZipArchive::new(File::open(&target).unwrap()).unwrap();
    let time = zip.by_name("contracts/0contract.pdf").unwrap().last_modified().unwrap();
    let parts = (time.year(), time.month(), time.day(), time.hour(), time.minute(), time.second());
    assert_eq!(parts, (2023, 3, 15, 9, 30, 0));
}

#[test]
fn reads_within_limits() {
    let bytes = archive(&[("big.txt", &[b'a'; 10_000], None)]);
    let mut read = Archive::new(Cursor::new(bytes)).unwrap();
    assert_matches!(read.read("big.txt", 9_999), Err(Error::LimitExceeded(_)));
    assert_eq!(read.read("big.txt", 10_000).unwrap().map(|data| data.len()), Some(10_000));
    assert_matches!(Archive::new(Cursor::new(b"not a zip".to_vec())), Err(Error::Malformed { .. }));
}

#[test]
fn dates_entries_in_utc() {
    const CASES: &[(u64, (i64, u8, u8))] =
        &[(0, (1970, 1, 1)), (951_782_400, (2000, 2, 29)), (1_678_872_600, (2023, 3, 15))];
    for &(seconds, date) in CASES {
        assert_eq!(civil(i64::try_from(seconds / 86_400).unwrap()), date, "{seconds}");
    }
    assert_eq!(dos_time(Some(UNIX_EPOCH)), DateTime::default(), "before 1980: the format's epoch");
    assert_eq!(dos_time(None), DateTime::default());
}
