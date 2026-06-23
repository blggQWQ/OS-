use dm::{BlockDevice, DmCryptDevice, MemoryBlockDevice};

#[test]
fn crypt_round_trip_hides_plaintext_on_lower_device() {
    let lower = MemoryBlockDevice::new(4, 64).unwrap();
    let crypt = DmCryptDevice::new(lower.clone(), b"demo-key").unwrap();
    let plaintext = *b"hello from dm-crypt minimal demo block, padded for testing......";

    crypt.write_block(1, &plaintext).unwrap();

    let raw = lower.snapshot_block(1).unwrap();
    assert_ne!(&raw[..], &plaintext[..]);
    assert!(!raw.windows(b"hello".len()).any(|part| part == b"hello"));

    let mut recovered = [0u8; 64];
    crypt.read_block(1, &mut recovered).unwrap();
    assert_eq!(recovered, plaintext);
}

#[test]
fn wrong_key_does_not_recover_plaintext() {
    let lower = MemoryBlockDevice::new(2, 32).unwrap();
    let crypt = DmCryptDevice::new(lower.clone(), b"right-key").unwrap();
    let wrong = DmCryptDevice::new(lower, b"wrong-key").unwrap();
    let plaintext = *b"0123456789abcdef0123456789abcdef";

    crypt.write_block(0, &plaintext).unwrap();

    let mut recovered = [0u8; 32];
    wrong.read_block(0, &mut recovered).unwrap();
    assert_ne!(recovered, plaintext);
}
