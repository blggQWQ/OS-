use dm::{BlockDevice, DmError, FileBlockDevice};
use std::fs;

#[test]
fn file_block_device_persists_blocks() {// 测试写入的数据在重新打开文件后还能读到（持久性）
    fs::create_dir_all("target").unwrap();// 创建target目录
    let path = "target/file-block-device-test.img";// 文件路径为target/file-block-device-test.img
    let dev = FileBlockDevice::create(path, 2, 16).unwrap();// 创建一个2个扇区的文件块设备，每块16字节
    dev.write_block(1, b"persistent-data!").unwrap();// 写入数据到第二个扇区，写的是b"persistent-data!"

    let reopened = FileBlockDevice::open(path, 16, false).unwrap();// 打开文件块设备，每块16字节
    let mut out = [0u8; 16];
    reopened.read_block(1, &mut out).unwrap();// 从第二个扇区读取数据

    assert_eq!(&out, b"persistent-data!");// 断言读取到的数据与写入的数据相同
}

#[test]
fn file_block_device_rejects_writes_when_read_only() {// 测试只读设备是否拒绝写入
    fs::create_dir_all("target").unwrap();// 创建target目录
    let path = "target/file-block-device-readonly-test.img";// 文件路径为target/file-block-device-readonly-test.img
    FileBlockDevice::create(path, 1, 16).unwrap();// 创建一个1个扇区的文件块设备，每块16字节

    let readonly = FileBlockDevice::open(path, 16, true).unwrap();// 打开文件块设备，每块16字节，只读模式
    let err = readonly.write_block(0, b"should-not-write!").unwrap_err();// 尝试写入数据到第一个扇区，写的是b"should-not-write!"

    assert_eq!(err, DmError::ReadOnlyDevice);// 断言写入失败，错误码为ReadOnlyDevice
}
