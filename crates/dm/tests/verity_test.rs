use dm::{BlockDevice, DmError, DmVerityDevice, MemoryBlockDevice};

#[test]
fn verity_accepts_untampered_blocks() {
    let lower = MemoryBlockDevice::new(3, 16).unwrap();
    lower.write_block(0, b"block-0000000000").unwrap();
    lower.write_block(1, b"block-1111111111").unwrap();
    lower.write_block(2, b"block-2222222222").unwrap();

    let verity = DmVerityDevice::build(lower).unwrap();
    let mut out = [0u8; 16];
    verity.read_block(2, &mut out).unwrap();

    assert_eq!(&out, b"block-2222222222");
}

#[test]
fn verity_rejects_tampered_data_block() {
    let lower = MemoryBlockDevice::new(2, 16).unwrap();
    lower.write_block(0, b"trusted-block-00").unwrap();
    lower.write_block(1, b"trusted-block-11").unwrap();

    let verity = DmVerityDevice::build(lower.clone()).unwrap();
    lower.write_block(1, b"tampered-block!!").unwrap();

    let mut out = [0u8; 16];
    let err = verity.read_block(1, &mut out).unwrap_err();
    assert_eq!(err, DmError::IntegrityViolation { block_id: 1 });
}

#[test]
fn verity_rejects_wrong_root_hash() {
    let lower = MemoryBlockDevice::new(1, 16).unwrap();
    lower.write_block(0, b"trusted-block-00").unwrap();

    let base = DmVerityDevice::build(lower.clone()).unwrap();
    let mut wrong_root = base.root_hash();
    wrong_root[0] ^= 0xff;
    let verity = DmVerityDevice::open(lower, base.tree().clone(), wrong_root);

    let mut out = [0u8; 16];
    let err = verity.read_block(0, &mut out).unwrap_err();
    assert_eq!(err, DmError::IntegrityViolation { block_id: 0 });
}

#[test]
fn verity_is_read_only() {
    let lower = MemoryBlockDevice::new(1, 16).unwrap();
    lower.write_block(0, b"trusted-block-00").unwrap();
    let verity = DmVerityDevice::build(lower).unwrap();

    let err = verity.write_block(0, b"new-data-block!!").unwrap_err();
    assert_eq!(err, DmError::ReadOnlyDevice);
}
