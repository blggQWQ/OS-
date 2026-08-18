use dm::{
    BlockDevice, DmCryptTable, DmError, DmStackBuilder, DmVerityDevice, InstrumentedDevice,
    MemoryBlockDevice,
};
use std::fmt::Write as _;
use std::str::FromStr;

fn crypt_table() -> DmCryptTable {
    let key = (0u8..64).fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    });
    DmCryptTable::from_str(&format!("aes-xts-plain64 {key} 0 memory 0")).unwrap()
}// 生成一个标准的AES-XTS 加密表配置，供两个测试复用

#[test]
fn composes_confidentiality_and_integrity_targets() {// 测试crypt+verity栈能同时提供加密和完整性，且verity使栈变为只读
    let raw = MemoryBlockDevice::new(4, 512).unwrap();
    let crypt_stack = DmStackBuilder::new(raw.clone())
        .crypt("crypt-root", &crypt_table())
        .unwrap()
        .build();
    let plain = [0x5au8; 512];
    crypt_stack.write_block(0, &plain).unwrap();
    // 用 builder 模式搭一个 单层 crypt 栈，写入数据。此时数据已经加密存到底层了

    let verity = DmVerityDevice::build(crypt_stack).unwrap();
    let tree = verity.tree().clone();
    let root_hash = verity.root_hash();// 在 crypt 栈之上建 verity 层，计算当前数据的哈希树和根哈希。这相当于"拍照存档"——之后任何篡改都会导致哈希不匹配
    let crypt_stack = DmStackBuilder::new(raw)
        .crypt("crypt-root", &crypt_table())
        .unwrap()
        .verity("verified-root", tree, root_hash)
        .build();
    //从底层重新搭：memory → crypt → verity，两层叠加，提供加密和完整性保护
    assert_eq!(crypt_stack.layers().len(), 2);
    assert!(crypt_stack.capabilities().confidentiality);
    assert!(crypt_stack.capabilities().integrity);
    assert!(crypt_stack.capabilities().read_only);
    // 测试读写能力
    let mut recovered = [0u8; 512];
    crypt_stack.read_block(0, &mut recovered).unwrap();
    assert_eq!(recovered, plain);// 能正确读回
    assert_eq!(
        crypt_stack.write_block(0, &plain),
        Err(DmError::ReadOnlyDevice)// 写被拒绝
    );
}

#[test]
fn instruments_io_at_a_mapper_boundary() {// 测试InstrumentedDevice 是否能正确统计 IO 操作次数、字节数和失败数
    let lower = InstrumentedDevice::new(MemoryBlockDevice::new(2, 64).unwrap()); // 在内存设备外面包一层“探针”，不改变功能，只记录统计
    let clone = lower.clone();
    lower.write_block(0, &[7u8; 64]).unwrap();
    let mut block = [0u8; 64];
    lower.read_block(0, &mut block).unwrap();
    assert!(lower.read_block(3, &mut block).is_err());// 读取超出范围的块会返回错误
    // 测试读写能力 
    let snapshot = clone.snapshot();
    assert_eq!(snapshot.read_operations, 2);  // 2 次读（1 成功 + 1 失败）
    assert_eq!(snapshot.write_operations, 1);  // 1 次写
    assert_eq!(snapshot.read_bytes, 128);  // 2 × 64 = 128 字节
    assert_eq!(snapshot.written_bytes, 64);  // 1 × 64 = 64 字节
    assert_eq!(snapshot.failed_operations, 1);  // 1 次读失败（超出范围）
    assert!(snapshot.max_read_nanoseconds <= snapshot.read_nanoseconds);  // 最大读取时间 <= 读取时间总和
}
