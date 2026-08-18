use dm::{BlockDevice, DmError, DmVerityDevice, MemoryBlockDevice};

#[test]
fn verity_accepts_untampered_blocks() {//用没有被篡改过的数据，正常读出来
    let lower = MemoryBlockDevice::new(3, 16).unwrap();
    lower.write_block(0, b"block-0000000000").unwrap();
    lower.write_block(1, b"block-1111111111").unwrap();
    lower.write_block(2, b"block-2222222222").unwrap();

    let verity = DmVerityDevice::build(lower).unwrap();
    let mut out = [0u8; 16];
    verity.read_block(2, &mut out).unwrap();

    assert_eq!(&out, b"block-2222222222");
}// 写入 3 块数据 → 建哈希树 → 读块 2 → 哈希校验通过 → 返回原始数据 ✓

#[test]
fn verity_rejects_tampered_data_block() {// 用被篡改过的数据，读取会报错
    let lower = MemoryBlockDevice::new(2, 16).unwrap();
    lower.write_block(0, b"trusted-block-00").unwrap();
    lower.write_block(1, b"trusted-block-11").unwrap();

    let verity = DmVerityDevice::build(lower.clone()).unwrap();
    //写入 2 块数据，建哈希树。此时树里记录的是 "trusted-block-11" 的哈希。
    lower.write_block(1, b"tampered-block!!").unwrap();
    //绕过 verity，直接往底层设备写篡改数据，哈希树不知道这件事
    let mut out = [0u8; 16];
    let err = verity.read_block(1, &mut out).unwrap_err();
    assert_eq!(err, DmError::IntegrityViolation { block_id: 1 });
    //通过 verity 读块 1 → 重新算哈希 → 和树里存的不一样 → IntegrityViolation 报错
    //所以即使攻击者直接改了硬盘上的数据，读取时也能发现
}

#[test]
fn verity_rejects_wrong_root_hash() {//根哈希被替换，读取报错
    let lower = MemoryBlockDevice::new(1, 16).unwrap();
    lower.write_block(0, b"trusted-block-00").unwrap();

    let base = DmVerityDevice::build(lower.clone()).unwrap();
    let mut wrong_root = base.root_hash();
    wrong_root[0] ^= 0xff;// 翻转第一个字节，让根哈希变错
    let verity = DmVerityDevice::open(lower, base.tree().clone(), wrong_root).unwrap();
    //正常建树，但故意用错误的根哈希打开设备。
    let mut out = [0u8; 16];
    let err = verity.read_block(0, &mut out).unwrap_err();
    assert_eq!(err, DmError::IntegrityViolation { block_id: 0 });
    //数据没被动，但根哈希不对 → 校验失败。
}

#[test]
fn verity_is_read_only() {//测试verity的只读功能，verity只读是因为verity的哈系树是在build时一次算好的
    //如果允许写入，新数据的哈希和树里存的不一样，后续读取就会报错。所以verity直接设为不可变也就是只读
    let lower = MemoryBlockDevice::new(1, 16).unwrap();
    lower.write_block(0, b"trusted-block-00").unwrap();
    let verity = DmVerityDevice::build(lower).unwrap();

    let err = verity.write_block(0, b"new-data-block!!").unwrap_err();
    assert_eq!(err, DmError::ReadOnlyDevice);
}
