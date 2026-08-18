use dm::{BlockDevice, DmCryptDevice, MemoryBlockDevice};

#[test]
fn crypt_round_trip_hides_plaintext_on_lower_device() {
    let lower = MemoryBlockDevice::new(4, 64).unwrap();// 底层设备，4个块，每个块64字节
    let crypt = DmCryptDevice::new(lower.clone(), b"demo-key").unwrap();// 加密设备，底层设备，密钥是demo-key
    let plaintext = *b"hello from dm-crypt minimal demo block, padded for testing......";// 明文

    crypt.write_block(1, &plaintext).unwrap();

    let raw = lower.snapshot_block(1).unwrap();// 底层设备的第1个块，即加密后的明文
    assert_ne!(&raw[..], &plaintext[..]); // 加密后的明文和明文不相等
    assert!(!raw.windows(b"hello".len()).any(|part| part == b"hello"));// 加密后的明文中不包含明文中的"hello"

    let mut recovered = [0u8; 64];// 解密后的明文
    crypt.read_block(1, &mut recovered).unwrap();// 从加密设备读取第1个块，即解密后的明文
    assert_eq!(recovered, plaintext);// 解密后的明文和明文相等
}

#[test]
fn wrong_key_does_not_recover_plaintext() {// 测试错误密钥是否能解密明文
    let lower = MemoryBlockDevice::new(2, 32).unwrap();// 底层设备，2个块，每个块32字节
    let crypt = DmCryptDevice::new(lower.clone(), b"right-key").unwrap();// 加密设备，底层设备，密钥是right-key
    let wrong = DmCryptDevice::new(lower, b"wrong-key").unwrap();// 解密设备，底层设备，密钥是wrong-key
    let plaintext = *b"0123456789abcdef0123456789abcdef";

    crypt.write_block(0, &plaintext).unwrap();// 写入明文到加密设备的第0个块

    let mut recovered = [0u8; 32];// 解密后的明文
    wrong.read_block(0, &mut recovered).unwrap();// 从解密设备读取第0个块，即解密后的明文
    assert_ne!(recovered, plaintext);// 解密后的明文和明文不相等
}

#[test]
fn same_plaintext_different_blocks_produces_different_ciphertext() {// 测试相同明文不同块加密后是否不同
    let lower = MemoryBlockDevice::new(4, 32).unwrap();// 底层设备，4个块，每个块32字节
    let crypt = DmCryptDevice::new(lower.clone(), b"key").unwrap();// 加密设备，底层设备，密钥是key
    let plaintext = *b"0123456789abcdef0123456789abcdef";

    crypt.write_block(0, &plaintext).unwrap();// 写入明文到加密设备的第0个块
    crypt.write_block(1, &plaintext).unwrap();// 写入明文到加密设备的第1个块

    let raw0 = lower.snapshot_block(0).unwrap();// 底层设备的第0个块，即加密后的明文
    let raw1 = lower.snapshot_block(1).unwrap();// 底层设备的第1个块，即加密后的明文
    assert_ne!(raw0, raw1);// 加密后的明文和明文不相等
}

#[test]
fn overwrite_block_recovers_new_data() {// 测试覆盖块是否能恢复新数据
    let lower = MemoryBlockDevice::new(2, 32).unwrap();// 底层设备，2个块，每个块32字节
    let crypt = DmCryptDevice::new(lower.clone(), b"key").unwrap();// 加密设备，底层设备，密钥是key

    let old = *b"old_data_old_data_old_data_old_da";// 旧数据
    let new = *b"new_data_new_data_new_data_new_da";// 新数据
    crypt.write_block(0, &old).unwrap();// 写入旧数据到加密设备的第0个块
    crypt.write_block(0, &new).unwrap();// 写入新数据到加密设备的第0个块

    let mut buf = [0u8; 32];// 解密后的数据
    crypt.read_block(0, &mut buf).unwrap();// 从加密设备读取第0个块，即解密后的新数据
    assert_eq!(buf, new);// 解密后的数据和新数据相等
}

#[test]
fn out_of_range_block_returns_error() {// 测试超出范围块是否能返回错误
    let lower = MemoryBlockDevice::new(2, 32).unwrap();// 底层设备，2个块，每个块32字节
    let crypt = DmCryptDevice::new(lower, b"key").unwrap();// 解密设备，底层设备，密钥是key
    let data = *b"0123456789abcdef0123456789abcdef"; // 测试数据

    assert!(crypt.write_block(5, &data).is_err());// 写入超出范围块返回错误
    assert!(crypt.read_block(5, &mut [0u8; 32]).is_err());// 读取超出范围块返回错误
}