use dm::{BlockDevice, DmError, DmVerityDevice, MemoryBlockDevice};

#[test]
fn verity_accepts_untampered_blocks() {
    // 测试没被篡改的数据，正常读出来
    // 写入3块数据 → 建哈希树 → 读块2 → 哈希校验通过 → 返回原始数据
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
    // 测试数据被篡改后，读取报 IntegrityViolation
    // 绕过 verity 直接往底层设备写篡改数据，哈希树不知道这件事
    // 通过 verity 读取时 → 重新算哈希 → 和树里存的不一样 → 报错
    // 验证：即使攻击者直接改了硬盘上的数据，读取时也能发现
    let lower = MemoryBlockDevice::new(2, 16).unwrap();
    lower.write_block(0, b"trusted-block-00").unwrap();
    lower.write_block(1, b"trusted-block-11").unwrap();

    let verity = DmVerityDevice::build(lower.clone()).unwrap();
    lower.write_block(1, b"tampered-block!!").unwrap(); // 绕过verity篡改数据

    let mut out = [0u8; 16];
    let err = verity.read_block(1, &mut out).unwrap_err();
    assert_eq!(err, DmError::IntegrityViolation { block_id: 1 });
}

#[test]
fn verity_rejects_wrong_root_hash() {
    // 测试根哈希被替换后，读取报 IntegrityViolation
    // 根哈希是信任的锚点，实际部署中存在可信位置（如内核命令行、TPM芯片）
    // 如果攻击者替换了根哈希，就能用篡改的数据建新树让校验"通过"
    // 所以根哈希本身也必须被保护
    let lower = MemoryBlockDevice::new(1, 16).unwrap();
    lower.write_block(0, b"trusted-block-00").unwrap();

    let base = DmVerityDevice::build(lower.clone()).unwrap();
    let mut wrong_root = base.root_hash();
    wrong_root[0] ^= 0xff; // 翻转第一个字节，让根哈希变错
    // DmVerityDevice::open 返回 Self（不是 Result），不需要 unwrap
    let verity = DmVerityDevice::open(lower, base.tree().clone(), wrong_root);

    let mut out = [0u8; 16];
    let err = verity.read_block(0, &mut out).unwrap_err();
    assert_eq!(err, DmError::IntegrityViolation { block_id: 0 });
}

#[test]
fn verity_is_read_only() {
    // 测试 verity 设备不允许写入
    // verity 只读是因为哈希树在 build 时一次性算好
    // 如果允许写入，新数据的哈希和树里存的不一致，后续读取就会报错
    // 所以干脆禁止写入——verity 设备是不可变的
    let lower = MemoryBlockDevice::new(1, 16).unwrap();
    lower.write_block(0, b"trusted-block-00").unwrap();
    let verity = DmVerityDevice::build(lower).unwrap();

    let err = verity.write_block(0, b"new-data-block!!").unwrap_err();
    assert_eq!(err, DmError::ReadOnlyDevice);
}

#[test]
fn verity_rejects_tampered_block_in_multi_block_device() {
    // 测试多块设备中篡改其中一块，只有被篡改的块报错，其他块正常
    // 验证 verity 的校验是逐块进行的，不会因为一块被篡改而影响其他块
    let lower = MemoryBlockDevice::new(3, 16).unwrap();
    lower.write_block(0, b"block-0000000000").unwrap();
    lower.write_block(1, b"block-1111111111").unwrap();
    lower.write_block(2, b"block-2222222222").unwrap();

    let verity = DmVerityDevice::build(lower.clone()).unwrap();
    lower.write_block(1, b"tampered-block!!").unwrap(); // 只篡改块1

    // 块0和块2没被动过，应该正常读出
    let mut out = [0u8; 16];
    verity.read_block(0, &mut out).unwrap();
    assert_eq!(&out, b"block-0000000000");
    verity.read_block(2, &mut out).unwrap();
    assert_eq!(&out, b"block-2222222222");

    // 块1被篡改，应该报错
    let err = verity.read_block(1, &mut out).unwrap_err();
    assert_eq!(err, DmError::IntegrityViolation { block_id: 1 });
}