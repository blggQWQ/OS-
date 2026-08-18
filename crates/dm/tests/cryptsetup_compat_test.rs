use dm::{BlockDevice, DmCryptDevice, DmCryptTable, MemoryBlockDevice};
use std::fmt::Write as _;
use std::str::FromStr;

fn key_hex() -> String {// 生成一个64字节的随机密钥
    (0u8..64).fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    })
}

#[test]
fn parses_linux_dm_crypt_table() {// 测试解析Linux dm-crypt表
    let text = format!(// 生成一个Linux dm-crypt表
        "aes-xts-plain64 {} 17 /dev/vdb 8 1 allow_discards",
        key_hex()
    );
    let table = DmCryptTable::from_str(&text).unwrap();// 解析Linux dm-crypt表
    assert_eq!(table.cipher, "aes-xts-plain64");// 加密算法是aes-xts-plain64
    assert_eq!(table.key.len(), 64);// 密钥长度是64字节
    assert_eq!(table.iv_offset, 17);// IV偏移量是17
    assert_eq!(table.device, "/dev/vdb");// 设备是/dev/vdb
    assert_eq!(table.offset, 8);// 偏移量是8
    assert_eq!(table.options, ["allow_discards"]);// 允许丢弃操作
}

#[test]
fn validates_linux_crypt_option_count_and_semantics() {// 测试Linux dm-crypt表选项计数和语义
    let malformed = DmCryptTable::from_str(&format!(// 生成一个Linux dm-crypt表，选项计数错误
        "aes-xts-plain64 {} 0 /dev/vdb 0 2 allow_discards",// 选项计数是3个，而不是4个
        key_hex()
    ));
    assert!(malformed.is_err());// 选项计数错误

    let unsupported = DmCryptTable::from_str(&format!(
        "aes-xts-plain64 {} 0 /dev/vdb 0 1 sector_size:4096",
        key_hex()
    ));// 生成一个Linux dm-crypt表，选项计数错误
    .unwrap();// 解析Linux dm-crypt表
    let lower = MemoryBlockDevice::new(2, 4096).unwrap();// 创建一个2个扇区的内存块设备
    assert!(DmCryptDevice::from_table(lower, &unsupported).is_err());// 选项计数错误
}

#[test]
fn aes_xts_round_trip_uses_offset_and_plain64_sector_number() {// 测试AES-XTS加密和解密是否使用偏移量和64字节扇区号
    let lower = MemoryBlockDevice::new(4, 4096).unwrap();// 创建一个4个扇区的内存块设备
    let table =
        DmCryptTable::from_str(&format!("aes-xts-plain64 {} 9 /dev/vdb 8", key_hex())).unwrap();// 生成一个Linux dm-crypt表，使用AES-XTS加密算法，偏移量为9，设备为/dev/vdb，偏移量为8
    let crypt = DmCryptDevice::from_table(lower.clone(), &table).unwrap();// 创建一个DmCrypt设备
    assert_eq!(crypt.num_blocks(), 3);// DmCrypt设备有3个扇区

    let plain: Vec<u8> = (0..4096).map(|index| (index % 251) as u8).collect();// 生成一个4096字节的随机数据
    crypt.write_block(0, &plain).unwrap();// 写入数据到DmCrypt设备的第0个扇区
    assert_eq!(lower.snapshot_block(0).unwrap(), vec![0; 4096]);// 第0个扇区的内容是0
    assert_ne!(lower.snapshot_block(1).unwrap(), plain);// 第1个扇区的内容不是随机数据

    let mut recovered = vec![0; 4096];
    crypt.read_block(0, &mut recovered).unwrap();// 从DmCrypt设备的第0个扇区读取数据
    assert_eq!(recovered, plain);// 读取到的数据是随机数据
}

#[test]
fn rejects_non_xts_cipher_and_wrong_key_size() {// 测试非AES-XTS加密算法和密钥长度错误是否被拒绝
    let lower = MemoryBlockDevice::new(2, 512).unwrap();// 创建一个2个扇区的内存块设备
    let unsupported = DmCryptTable::from_str("aes-cbc-essiv:sha256 00 0 /dev/vdb 0").unwrap();// 生成一个Linux dm-crypt表，使用AES-CBC加密算法，密钥为00，偏移量为0，设备为/dev/vdb
    assert!(DmCryptDevice::from_table(lower.clone(), &unsupported).is_err());// 选项计数错误

    let short_key = DmCryptTable::from_str("aes-xts-plain64 0011 0 /dev/vdb 0").unwrap();// 生成一个Linux dm-crypt表，使用AES-XTS加密算法，密钥为0011，偏移量为0，设备为/dev/vdb
    assert!(DmCryptDevice::from_table(lower, &short_key).is_err());// 选项计数错误
}
