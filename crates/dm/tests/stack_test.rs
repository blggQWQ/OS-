use dm::{
    BlockDevice, DmCryptTable, DmError, DmStackBuilder, DmVerityDevice, InstrumentedDevice,
    MemoryBlockDevice,
};
use std::fmt::Write as _;
use std::str::FromStr;

fn crypt_table() -> DmCryptTable {
    // 生成一个标准的AES-XTS加密表配置，供测试复用
    let key = (0u8..64).fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    });
    DmCryptTable::from_str(&format!("aes-xts-plain64 {key} 0 memory 0")).unwrap()
}

#[test]
fn composes_confidentiality_and_integrity_targets() {
    // 测试crypt+verity栈能同时提供加密和完整性，且verity使栈变为只读
    // 这是Linux device-mapper的精髓——栈式组合：
    //   memory → crypt(加密) → verity(完整性校验)
    let raw = MemoryBlockDevice::new(4, 512).unwrap();

    // 第一步：先用单层crypt栈写入数据
    let crypt_stack = DmStackBuilder::new(raw.clone())
        .crypt("crypt-root", &crypt_table())
        .unwrap()
        .build();
    let plain = [0x5au8; 512];
    crypt_stack.write_block(0, &plain).unwrap();

    // 第二步：在crypt栈之上建verity层，"拍照存档"当前数据的哈希树和根哈希
    let verity = DmVerityDevice::build(crypt_stack).unwrap();
    let tree = verity.tree().clone();
    let root_hash = verity.root_hash();

    // 第三步：从底层重新搭完整栈：memory → crypt → verity
    let crypt_stack = DmStackBuilder::new(raw)
        .crypt("crypt-root", &crypt_table())
        .unwrap()
        .verity("verified-root", tree, root_hash)
        .build();

    // 验证栈的属性
    assert_eq!(crypt_stack.layers().len(), 2); // 2层：crypt + verity
    assert!(crypt_stack.capabilities().confidentiality); // 有加密（crypt贡献）
    assert!(crypt_stack.capabilities().integrity); // 有完整性（verity贡献）
    assert!(crypt_stack.capabilities().read_only); // 只读（verity贡献）

    // 验证读写行为
    let mut recovered = [0u8; 512];
    crypt_stack.read_block(0, &mut recovered).unwrap();
    assert_eq!(recovered, plain); // 能正确读回
    assert_eq!(
        crypt_stack.write_block(0, &plain),
        Err(DmError::ReadOnlyDevice) // 写被拒绝（verity只读）
    );
}

#[test]
fn instruments_io_at_a_mapper_boundary() {
    // 测试InstrumentedDevice能正确统计IO操作次数、字节数和失败数
    // InstrumentedDevice是"探针"——不改变功能，只记录统计
    let lower = InstrumentedDevice::new(MemoryBlockDevice::new(2, 64).unwrap());
    let clone = lower.clone(); // clone共享同一份统计数据（Arc浅拷贝）

    lower.write_block(0, &[7u8; 64]).unwrap(); // 写1次，64字节
    let mut block = [0u8; 64];
    lower.read_block(0, &mut block).unwrap(); // 读1次成功，64字节
    assert!(lower.read_block(3, &mut block).is_err()); // 读1次失败（块3不存在）

    let snapshot = clone.snapshot();
    assert_eq!(snapshot.read_operations, 2); // 2次读（1成功 + 1失败）
    assert_eq!(snapshot.write_operations, 1); // 1次写
    assert_eq!(snapshot.read_bytes, 128); // 2 × 64 = 128字节
    assert_eq!(snapshot.written_bytes, 64); // 1 × 64 = 64字节
    assert_eq!(snapshot.failed_operations, 1); // 1次读失败
    assert!(snapshot.max_read_nanoseconds <= snapshot.read_nanoseconds); // 单次最长 ≤ 总时间
}

#[test]
fn crypt_verity_stack_detects_bottom_layer_tampering() {
    // 测试crypt+verity栈能检测到底层数据被篡改
    // 这是两个模块组合的核心安全场景：
    //   1. 攻击者绕过栈，直接修改底层设备的数据
    //   2. crypt层会解密出乱码（因为密文被改了）
    //   3. verity层校验乱码的哈希，发现和树里存的不一致 → IntegrityViolation
    let raw = MemoryBlockDevice::new(4, 512).unwrap();

    // 先用单层crypt栈写入数据，建哈希树
    let crypt_stack = DmStackBuilder::new(raw.clone())
        .crypt("crypt-root", &crypt_table())
        .unwrap()
        .build();
    let plain = [0x5au8; 512];
    crypt_stack.write_block(0, &plain).unwrap();

    let verity = DmVerityDevice::build(crypt_stack).unwrap();
    let tree = verity.tree().clone();
    let root_hash = verity.root_hash();

    // 搭完整栈（保留raw的引用用于篡改）
    let crypt_stack = DmStackBuilder::new(raw.clone())
        .crypt("crypt-root", &crypt_table())
        .unwrap()
        .verity("verified-root", tree, root_hash)
        .build();

    // 篡改底层设备的块0（绕过crypt和verity）
    raw.write_block(0, &[0xffu8; 512]).unwrap();

    // 通过栈读取 → verity应该检测到篡改
    let mut recovered = [0u8; 512];
    let result = crypt_stack.read_block(0, &mut recovered);
    assert!(result.is_err()); // 篡改被检测到
}