use dm::{BlockDevice, DmCryptDevice, DmCryptTable, MemoryBlockDevice};
use std::str::FromStr;

fn key_hex() -> String {
    (0u8..64).map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn parses_linux_dm_crypt_table() {
    let text = format!(
        "aes-xts-plain64 {} 17 /dev/vdb 8 1 allow_discards",
        key_hex()
    );
    let table = DmCryptTable::from_str(&text).unwrap();
    assert_eq!(table.cipher, "aes-xts-plain64");
    assert_eq!(table.key.len(), 64);
    assert_eq!(table.iv_offset, 17);
    assert_eq!(table.device, "/dev/vdb");
    assert_eq!(table.offset, 8);
    assert_eq!(table.options, ["1", "allow_discards"]);
}

#[test]
fn aes_xts_round_trip_uses_offset_and_plain64_sector_number() {
    let lower = MemoryBlockDevice::new(4, 4096).unwrap();
    let table = DmCryptTable::from_str(&format!(
        "aes-xts-plain64 {} 9 /dev/vdb 8",
        key_hex()
    ))
    .unwrap();
    let crypt = DmCryptDevice::from_table(lower.clone(), &table).unwrap();
    assert_eq!(crypt.num_blocks(), 3);

    let plain: Vec<u8> = (0..4096).map(|index| (index % 251) as u8).collect();
    crypt.write_block(0, &plain).unwrap();
    assert_eq!(lower.snapshot_block(0).unwrap(), vec![0; 4096]);
    assert_ne!(lower.snapshot_block(1).unwrap(), plain);

    let mut recovered = vec![0; 4096];
    crypt.read_block(0, &mut recovered).unwrap();
    assert_eq!(recovered, plain);
}

#[test]
fn rejects_non_xts_cipher_and_wrong_key_size() {
    let lower = MemoryBlockDevice::new(2, 512).unwrap();
    let unsupported = DmCryptTable::from_str("aes-cbc-essiv:sha256 00 0 /dev/vdb 0").unwrap();
    assert!(DmCryptDevice::from_table(lower.clone(), &unsupported).is_err());

    let short_key = DmCryptTable::from_str("aes-xts-plain64 0011 0 /dev/vdb 0").unwrap();
    assert!(DmCryptDevice::from_table(lower, &short_key).is_err());
}
