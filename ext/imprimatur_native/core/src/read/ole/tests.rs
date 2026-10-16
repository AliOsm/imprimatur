use std::assert_matches;

use super::*;
use crate::read::Limits;
use crate::test_support::ole_of;

fn read(file: &[u8], name: &str) -> Result<Option<Vec<u8>>> {
    let limits = Limits::default();
    let budget = Budget::new(&limits, file.len())?;
    Ole::open(file, &budget)?.stream(name, &budget)
}

#[test]
fn reads_streams_from_sectors_and_the_mini_stream() {
    let large: Vec<u8> = (0..10_000_u32).map(|n| u8::try_from(n % 251).unwrap()).collect();
    for stream in [b"tiny".to_vec(), vec![7; 1000], large] {
        let file = ole_of("Workbook", &stream);
        assert_eq!(
            read(&file, "Workbook").unwrap(),
            Some(stream.clone()),
            "{} bytes",
            stream.len()
        );
        assert_eq!(read(&file, "WORKBOOK").unwrap(), Some(stream.clone()), "names ignore case");
        assert_eq!(read(&file, "Book").unwrap(), None);
    }
}

#[test]
fn refuses_damaged_files() {
    let file = ole_of("Workbook", &[1; 5000]);
    let mut looping = file.clone();
    // The data's first sector points back at itself.
    looping[512 + 2 * 4..512 + 3 * 4].copy_from_slice(&2_u32.to_le_bytes());
    let mut oversized = file.clone();
    oversized[1024 + 128 + 0x78..1024 + 128 + 0x7C].copy_from_slice(&1_000_000_u32.to_le_bytes());
    let mut unknown_sector = file.clone();
    unknown_sector[0x1E] = 10;
    for (damaged, why) in [
        (&looping, "a chain that loops"),
        (&oversized, "a stream larger than the file"),
        (&unknown_sector, "an unknown sector size"),
        (&file[..600].to_vec(), "a cut file"),
        (&b"\xD0\xCF\x11\xE0 not one".to_vec(), "no header"),
    ] {
        assert_matches!(read(damaged, "Workbook"), Err(Error::Malformed { .. }), "{why}");
    }
}
