use dm::{
    BlockDevice, DmCryptTable, DmError, DmStackBuilder, DmVerityDevice, InstrumentedDevice,
    MemoryBlockDevice,
};
use std::str::FromStr;

fn crypt_table() -> DmCryptTable {
    let key: String = (0u8..64).map(|byte| format!("{byte:02x}")).collect();
    DmCryptTable::from_str(&format!("aes-xts-plain64 {key} 0 memory 0")).unwrap()
}

#[test]
fn composes_confidentiality_and_integrity_targets() {
    let raw = MemoryBlockDevice::new(4, 512).unwrap();
    let crypt_stack = DmStackBuilder::new(raw.clone())
        .crypt("crypt-root", &crypt_table())
        .unwrap()
        .build();
    let plain = [0x5au8; 512];
    crypt_stack.write_block(0, &plain).unwrap();

    let verity = DmVerityDevice::build(crypt_stack).unwrap();
    let tree = verity.tree().clone();
    let root_hash = verity.root_hash();
    let crypt_stack = DmStackBuilder::new(raw)
        .crypt("crypt-root", &crypt_table())
        .unwrap()
        .verity("verified-root", tree, root_hash)
        .build();

    assert_eq!(crypt_stack.layers().len(), 2);
    assert!(crypt_stack.capabilities().confidentiality);
    assert!(crypt_stack.capabilities().integrity);
    assert!(crypt_stack.capabilities().read_only);

    let mut recovered = [0u8; 512];
    crypt_stack.read_block(0, &mut recovered).unwrap();
    assert_eq!(recovered, plain);
    assert_eq!(
        crypt_stack.write_block(0, &plain),
        Err(DmError::ReadOnlyDevice)
    );
}

#[test]
fn instruments_io_at_a_mapper_boundary() {
    let lower = InstrumentedDevice::new(MemoryBlockDevice::new(2, 64).unwrap());
    let clone = lower.clone();
    lower.write_block(0, &[7u8; 64]).unwrap();
    let mut block = [0u8; 64];
    lower.read_block(0, &mut block).unwrap();
    assert!(lower.read_block(3, &mut block).is_err());

    let snapshot = clone.snapshot();
    assert_eq!(snapshot.read_operations, 2);
    assert_eq!(snapshot.write_operations, 1);
    assert_eq!(snapshot.read_bytes, 128);
    assert_eq!(snapshot.written_bytes, 64);
    assert_eq!(snapshot.failed_operations, 1);
    assert!(snapshot.max_read_nanoseconds <= snapshot.read_nanoseconds);
}
