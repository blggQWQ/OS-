use dm::{BlockDevice, DmError, FileBlockDevice};
use std::fs;

#[test]
fn file_block_device_persists_blocks() {
    // 测试写入的数据在重新打开文件后还能读到（持久性）
    // 验证 FileBlockDevice 的数据真的写到了硬盘，关了重开还在
    fs::create_dir_all("target").unwrap();
    let path = "target/file-block-device-test.img";
    let dev = FileBlockDevice::create(path, 2, 16).unwrap(); // 创建2块、每块16字节的文件块设备
    dev.write_block(1, b"persistent-data!").unwrap(); // 往块1写入数据

    // 重新打开同一个文件（只读模式），验证数据持久化
    let reopened = FileBlockDevice::open(path, 16, false).unwrap();
    let mut out = [0u8; 16];
    reopened.read_block(1, &mut out).unwrap();
    assert_eq!(&out, b"persistent-data!"); // 读出来的 = 之前写的
}

#[test]
fn file_block_device_rejects_writes_when_read_only() {
    // 测试只读模式下写入会报错
    // 验证 FileBlockDevice 的 writable 标志真正生效
    fs::create_dir_all("target").unwrap();
    let path = "target/file-block-device-readonly-test.img";
    FileBlockDevice::create(path, 1, 16).unwrap(); // 先创建文件

    // 以只读模式打开（writable = false）
    let readonly = FileBlockDevice::open(path, 16, false).unwrap();
    let err = readonly.write_block(0, b"should-not-write!").unwrap_err();
    assert_eq!(err, DmError::ReadOnlyDevice); // 报 ReadOnlyDevice 错误
}

#[test]
fn file_block_device_rejects_opening_nonexistent_file() {
    // 测试打开不存在的文件应报错
    // 验证 FileBlockDevice::open 不会静默创建文件，而是返回 IO 错误
    let result = FileBlockDevice::open("target/no-such-file.img", 16, false);
    assert!(result.is_err()); // 文件不存在，打开失败
}

#[test]
fn file_block_device_rejects_unaligned_image_size() {
    // 测试 block_size 不整除文件大小时应报错
    // 验证 FileBlockDevice::open 会检查文件大小是否是 block_size 的整数倍
    fs::create_dir_all("target").unwrap();
    let path = "target/file-block-device-unaligned-test.img";
    // 创建一个2块、每块16字节的文件（总大小32字节）
    FileBlockDevice::create(path, 2, 16).unwrap();

    // 用 block_size=7 打开，32 % 7 ≠ 0，应该报错
    let result = FileBlockDevice::open(path, 7, false);
    assert!(result.is_err());
}