use dm::{BlockDevice, DmError, FileBlockDevice};
use std::fs;

#[test]
fn file_block_device_persists_blocks() {
    fs::create_dir_all("target").unwrap();
    let path = "target/file-block-device-test.img";
    let dev = FileBlockDevice::create(path, 2, 16).unwrap();
    dev.write_block(1, b"persistent-data!").unwrap();

    let reopened = FileBlockDevice::open(path, 16, false).unwrap();
    let mut out = [0u8; 16];
    reopened.read_block(1, &mut out).unwrap();

    assert_eq!(&out, b"persistent-data!");
}

#[test]
fn file_block_device_rejects_writes_when_read_only() {
    fs::create_dir_all("target").unwrap();
    let path = "target/file-block-device-readonly-test.img";
    FileBlockDevice::create(path, 1, 16).unwrap();

    let readonly = FileBlockDevice::open(path, 16, false).unwrap();
    let err = readonly.write_block(0, b"should-not-write!").unwrap_err();

    assert_eq!(err, DmError::ReadOnlyDevice);
}
