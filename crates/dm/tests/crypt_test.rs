use dm::{BlockDevice, DmCryptDevice, MemoryBlockDevice};

#[test]
fn crypt_round_trip_hides_plaintext_on_lower_device() {
    // 测试加密后底层设备无明文 + 加解密往返正确
    // 验证两个核心属性：加密确实隐藏了数据，且正确密钥能还原
    let lower = MemoryBlockDevice::new(4, 64).unwrap(); // 底层设备：4块，每块64字节
    let crypt = DmCryptDevice::new(lower.clone(), b"demo-key").unwrap(); // 加密设备，密钥是demo-key
    let plaintext = *b"hello from dm-crypt minimal demo block, padded for testing......";

    crypt.write_block(1, &plaintext).unwrap(); // 通过加密设备写入块1

    let raw = lower.snapshot_block(1).unwrap(); // 直接看底层设备的块1（密文）
    assert_ne!(&raw[..], &plaintext[..]); // 密文 ≠ 明文，加密生效
    // 用滑动窗口检查密文中是否包含 "hello" 子串
    // 光有 assert_ne 不够——密文可能只改了一个字节，明文模式还在
    assert!(!raw.windows(b"hello".len()).any(|part| part == b"hello"));

    let mut recovered = [0u8; 64];
    crypt.read_block(1, &mut recovered).unwrap(); // 通过加密设备读回
    assert_eq!(recovered, plaintext); // 解密后 = 原始明文，往返正确
}

#[test]
fn wrong_key_does_not_recover_plaintext() {
    // 测试用错误密钥解密时，无法还原出明文
    // 验证密钥隔离性：不同密钥无法解密对方加密的数据
    let lower = MemoryBlockDevice::new(2, 32).unwrap();
    let crypt = DmCryptDevice::new(lower.clone(), b"right-key").unwrap(); // 正确密钥
    let wrong = DmCryptDevice::new(lower, b"wrong-key").unwrap(); // 错误密钥
    // 两者共享同一份底层存储（Arc浅拷贝，指向同一块内存）
    let plaintext = *b"0123456789abcdef0123456789abcdef";

    crypt.write_block(0, &plaintext).unwrap(); // 用正确密钥写入

    let mut recovered = [0u8; 32];
    wrong.read_block(0, &mut recovered).unwrap(); // 用错误密钥读回
    assert_ne!(recovered, plaintext); // 解密结果 ≠ 明文
}

#[test]
fn same_plaintext_different_blocks_produces_different_ciphertext() {
    // 测试相同明文写入不同块，密文必须不同
    // 这是加密的基本安全属性：如果相同明文→相同密文，
    // 攻击者可以对比两个扇区的密文推断"这两个扇区内容一样"
    let lower = MemoryBlockDevice::new(4, 32).unwrap();
    let crypt = DmCryptDevice::new(lower.clone(), b"key").unwrap();
    let plaintext = *b"0123456789abcdef0123456789abcdef";

    crypt.write_block(0, &plaintext).unwrap();
    crypt.write_block(1, &plaintext).unwrap(); // 相同明文写入不同块

    let raw0 = lower.snapshot_block(0).unwrap();
    let raw1 = lower.snapshot_block(1).unwrap();
    assert_ne!(raw0, raw1); // 不同块的密文必须不同
}

#[test]
fn overwrite_block_recovers_new_data() {
    // 测试覆写同一块后，能正确读回新数据
    // 验证数据更新场景：旧数据被新数据替换，解密得到新数据
    let lower = MemoryBlockDevice::new(2, 32).unwrap();
    let crypt = DmCryptDevice::new(lower.clone(), b"key").unwrap();

    let old = *b"old_data_old_data_old_data_old_da";
    let new = *b"new_data_new_data_new_data_new_da";
    crypt.write_block(0, &old).unwrap(); // 先写旧数据
    crypt.write_block(0, &new).unwrap(); // 再写新数据（覆写）

    let mut buf = [0u8; 32];
    crypt.read_block(0, &mut buf).unwrap();
    assert_eq!(buf, new); // 读回的是新数据，不是旧数据
}

#[test]
fn out_of_range_block_returns_error() {
    // 测试越界读写返回错误，而不是 panic 或越界访问内存
    // 验证异常输入被安全拒绝
    let lower = MemoryBlockDevice::new(2, 32).unwrap();
    let crypt = DmCryptDevice::new(lower, b"key").unwrap();
    let data = *b"0123456789abcdef0123456789abcdef";

    assert!(crypt.write_block(5, &data).is_err()); // 块5不存在，写入报错
    assert!(crypt.read_block(5, &mut [0u8; 32]).is_err()); // 块5不存在，读取报错
}

#[test]
fn multi_block_independence() {
    // 测试多块读写互不干扰
    // 写入多个块后，每个块独立解密正确，不会串数据
    let lower = MemoryBlockDevice::new(4, 32).unwrap();
    let crypt = DmCryptDevice::new(lower.clone(), b"key").unwrap();

    let data0 = *b"block_0_data____block_0_data____";
    let data1 = *b"block_1_data____block_1_data____";
    let data2 = *b"block_2_data____block_2_data____";
    let data3 = *b"block_3_data____block_3_data____";

    crypt.write_block(0, &data0).unwrap();
    crypt.write_block(1, &data1).unwrap();
    crypt.write_block(2, &data2).unwrap();
    crypt.write_block(3, &data3).unwrap();

    // 逐块读回，验证数据没有串
    let mut buf = [0u8; 32];
    crypt.read_block(0, &mut buf).unwrap();
    assert_eq!(buf, data0);
    crypt.read_block(1, &mut buf).unwrap();
    assert_eq!(buf, data1);
    crypt.read_block(2, &mut buf).unwrap();
    assert_eq!(buf, data2);
    crypt.read_block(3, &mut buf).unwrap();
    assert_eq!(buf, data3);
}

#[test]
fn boundary_last_block_works() {
    // 测试写入最后一个块（边界值）
    // 验证块号从0开始，最后一个块号 = num_blocks - 1
    let lower = MemoryBlockDevice::new(4, 32).unwrap();
    let crypt = DmCryptDevice::new(lower.clone(), b"key").unwrap();
    let plaintext = *b"last_block_data__last_block_data__";

    crypt.write_block(3, &plaintext).unwrap(); // 写最后一个块（块3）

    let mut recovered = [0u8; 32];
    crypt.read_block(3, &mut recovered).unwrap();
    assert_eq!(recovered, plaintext); // 往返正确
}