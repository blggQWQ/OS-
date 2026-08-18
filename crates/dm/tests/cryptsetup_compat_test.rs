use dm::{BlockDevice, DmCryptDevice, DmCryptTable, MemoryBlockDevice};
use std::fmt::Write as _;
use std::str::FromStr;

fn key_hex() -> String {
    // 生成一个64字节的十六进制密钥字符串："00010203...3f"
    // AES-256-XTS需要64字节密钥：32字节data key + 32字节tweak key
    (0u8..64).fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    })
}

#[test]
fn parses_linux_dm_crypt_table() {
    // 测试能正确解析Linux dm-crypt的表格式字符串
    // 对应Linux dm-crypt表格式的各字段：
    //   cipher | key | iv_offset | device | offset | 选项数量 | 选项内容
    let text = format!(
        "aes-xts-plain64 {} 17 /dev/vdb 8 1 allow_discards",
        key_hex()
    );
    let table = DmCryptTable::from_str(&text).unwrap();
    assert_eq!(table.cipher, "aes-xts-plain64"); // 加密算法+IV模式
    assert_eq!(table.key.len(), 64); // 密钥长度64字节（32+32）
    assert_eq!(table.iv_offset, 17); // IV偏移量：影响加密时扇区号的计算
    assert_eq!(table.device, "/dev/vdb"); // 底层设备路径
    assert_eq!(table.offset, 8); // 数据偏移：跳过前8个扇区
    assert_eq!(table.options, ["allow_discards"]); // 选项：允许TRIM操作
}

#[test]
fn validates_linux_crypt_option_count_and_semantics() {
    // 测试对Linux cryptsetup的选项做校验
    // 分两部分：选项数量不匹配 + 不支持的选项

    // ① 选项数量不匹配：说有2个选项，但只给了1个（allow_discards）
    let malformed = DmCryptTable::from_str(&format!(
        "aes-xts-plain64 {} 0 /dev/vdb 0 2 allow_discards",
        key_hex()
    ));
    assert!(malformed.is_err()); // 数量对不上，解析失败

    // ② 不支持的选项：sector_size:4096 不在白名单里
    let unsupported = DmCryptTable::from_str(&format!(
        "aes-xts-plain64 {} 0 /dev/vdb 0 1 sector_size:4096",
        key_hex()
    ))
    .unwrap(); // 解析成功（from_str只做格式解析）
    let lower = MemoryBlockDevice::new(2, 4096).unwrap();
    assert!(DmCryptDevice::from_table(lower, &unsupported).is_err()); // from_table做语义校验，报错
}

#[test]
fn aes_xts_round_trip_uses_offset_and_plain64_sector_number() {
    // 测试AES-XTS加解密往返正确，且offset和iv_offset真正生效
    // offset=8扇区=1块（4096/512=8扇区/块），所以逻辑块数=4-1=3
    // iv_offset=9，影响IV计算：实际IV输入=iv_offset+扇区号
    let lower = MemoryBlockDevice::new(4, 4096).unwrap();
    let table =
        DmCryptTable::from_str(&format!("aes-xts-plain64 {} 9 /dev/vdb 8", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower.clone(), &table).unwrap();
    assert_eq!(crypt.num_blocks(), 3); // 4块 - 1块(offset) = 3个逻辑块

    let plain: Vec<u8> = (0..4096).map(|index| (index % 251) as u8).collect();
    crypt.write_block(0, &plain).unwrap(); // 逻辑块0 → 物理块1（因为offset跳过了块0）
    assert_eq!(lower.snapshot_block(0).unwrap(), vec![0; 4096]); // 物理块0没被写过，全零
    assert_ne!(lower.snapshot_block(1).unwrap(), plain); // 物理块1存的是密文

    let mut recovered = vec![0; 4096];
    crypt.read_block(0, &mut recovered).unwrap();
    assert_eq!(recovered, plain); // 往返正确
}

#[test]
fn rejects_non_xts_cipher_and_wrong_key_size() {
    // 测试拒绝不支持的算法和错误密钥长度
    let lower = MemoryBlockDevice::new(2, 512).unwrap();

    // ① 不支持的算法：aes-cbc-essiv:sha256
    let unsupported = DmCryptTable::from_str("aes-cbc-essiv:sha256 00 0 /dev/vdb 0").unwrap();
    assert!(DmCryptDevice::from_table(lower.clone(), &unsupported).is_err());

    // ② 密钥太短：只有2字节（需要64字节）
    // from_str只做格式解析（十六进制合法），from_table才做语义校验（长度必须=64）
    let short_key = DmCryptTable::from_str("aes-xts-plain64 0011 0 /dev/vdb 0").unwrap();
    assert!(DmCryptDevice::from_table(lower, &short_key).is_err());
}

#[test]
fn iv_offset_affects_ciphertext() {
    // 测试iv_offset不同时，相同明文+相同密钥产生不同密文
    // iv_offset影响IV的计算：IV输入 = iv_offset + 扇区号
    // 不同的iv_offset → 不同的IV → 不同的tweak → 不同的密文
    let key = key_hex();
    let lower_a = MemoryBlockDevice::new(2, 512).unwrap();
    let lower_b = MemoryBlockDevice::new(2, 512).unwrap();

    let table_a = DmCryptTable::from_str(&format!("aes-xts-plain64 {key} 0 memory 0")).unwrap();
    let table_b = DmCryptTable::from_str(&format!("aes-xts-plain64 {key} 7 memory 0")).unwrap();

    let crypt_a = DmCryptDevice::from_table(lower_a.clone(), &table_a).unwrap();
    let crypt_b = DmCryptDevice::from_table(lower_b.clone(), &table_b).unwrap();

    let plain = [0x5au8; 512];
    crypt_a.write_block(0, &plain).unwrap();
    crypt_b.write_block(0, &plain).unwrap();

    // 相同明文+相同密钥，但iv_offset不同 → 密文不同
    assert_ne!(
        lower_a.snapshot_block(0).unwrap(),
        lower_b.snapshot_block(0).unwrap()
    );
}

#[test]
fn aes_xts_multi_block_round_trip() {
    // 测试AES-XTS模式下多块连续读写的往返正确性
    // 验证多个块独立加密/解密，互不干扰
    let lower = MemoryBlockDevice::new(4, 512).unwrap();
    let table =
        DmCryptTable::from_str(&format!("aes-xts-plain64 {} 0 memory 0", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower.clone(), &table).unwrap();

    let plain0: Vec<u8> = (0..512).map(|i| (i % 97) as u8).collect();
    let plain1: Vec<u8> = (0..512).map(|i| (i % 131) as u8).collect();

    crypt.write_block(0, &plain0).unwrap();
    crypt.write_block(1, &plain1).unwrap();

    // 密文互不相同
    assert_ne!(lower.snapshot_block(0).unwrap(), lower.snapshot_block(1).unwrap());

    // 各自往返正确
    let mut recovered = vec![0u8; 512];
    crypt.read_block(0, &mut recovered).unwrap();
    assert_eq!(recovered, plain0);
    crypt.read_block(1, &mut recovered).unwrap();
    assert_eq!(recovered, plain1);
}

#[test]
fn aes_xts_boundary_last_logical_block_with_offset() {
    // 测试有offset时，写入最后一个逻辑块（边界值）
    // offset=8扇区=1块，4块设备→3个逻辑块，最后一个逻辑块号=2
    let lower = MemoryBlockDevice::new(4, 4096).unwrap();
    let table =
        DmCryptTable::from_str(&format!("aes-xts-plain64 {} 0 /dev/vdb 8", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower.clone(), &table).unwrap();

    let plain: Vec<u8> = (0..4096).map(|i| (i % 199) as u8).collect();
    crypt.write_block(2, &plain).unwrap(); // 写最后一个逻辑块

    let mut recovered = vec![0; 4096];
    crypt.read_block(2, &mut recovered).unwrap();
    assert_eq!(recovered, plain); // 往返正确
}