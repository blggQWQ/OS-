use dm::{BlockDevice, DmCryptDevice, DmCryptTable, DmError, MemoryBlockDevice, Result};
use std::fmt::Write as _;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;

fn key_hex() -> String {
    (0u8..64).fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    })
}

#[derive(Clone)]
struct HugeLogicalDevice {
    blocks: u64,
    block_size: usize,
}

impl BlockDevice for HugeLogicalDevice {
    fn block_size(&self) -> usize {
        self.block_size
    }

    fn num_blocks(&self) -> u64 {
        self.blocks
    }

    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        if block_id >= self.blocks {
            return Err(DmError::InvalidBlockId {
                block_id,
                blocks: self.blocks,
            });
        }
        buf.fill(0);
        Ok(())
    }

    fn write_block(&self, _block_id: u64, _buf: &[u8]) -> Result<()> {
        Ok(())
    }
}

#[test]
fn rejects_capacity_and_sector_number_overflow_without_panicking() {
    assert!(matches!(
        MemoryBlockDevice::new(u64::MAX, 4096),
        Err(DmError::CapacityOverflow { .. })
    ));

    let lower = HugeLogicalDevice {
        blocks: u64::MAX,
        block_size: 4096,
    };
    let table = DmCryptTable::from_str(&format!("aes-xts-plain64 {} 0 huge 0", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower, &table).unwrap();
    let mut block = [0u8; 4096];
    let overflowing_block = u64::MAX / 8 + 1;
    assert!(matches!(
        crypt.read_block(overflowing_block, &mut block),
        Err(DmError::InvalidTable(message)) if message.contains("overflow")
    ));
}

#[test]
fn same_plaintext_uses_different_ciphertext_in_different_sectors() {
    let lower = MemoryBlockDevice::new(2, 512).unwrap();
    let table =
        DmCryptTable::from_str(&format!("aes-xts-plain64 {} 0 memory 0", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower.clone(), &table).unwrap();
    let plaintext = [0x5a; 512];

    crypt.write_block(0, &plaintext).unwrap();
    crypt.write_block(1, &plaintext).unwrap();

    assert_ne!(
        lower.snapshot_block(0).unwrap(),
        lower.snapshot_block(1).unwrap()
    );
}

#[test]
fn rejects_unaligned_io_and_out_of_range_block() {
    let lower = MemoryBlockDevice::new(1, 512).unwrap();
    let table =
        DmCryptTable::from_str(&format!("aes-xts-plain64 {} 0 memory 0", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower, &table).unwrap();

    let mut short = [0u8; 511];
    assert!(matches!(
        crypt.read_block(0, &mut short),
        Err(DmError::InvalidBlockSize { .. })
    ));
    let mut block = [0u8; 512];
    assert!(matches!(
        crypt.read_block(1, &mut block),
        Err(DmError::InvalidBlockId { .. })
    ));
}

#[test]
fn memory_backend_supports_concurrent_disjoint_io() {
    let device = Arc::new(MemoryBlockDevice::new(8, 64).unwrap());
    let workers: Vec<_> = (0u64..8)
        .map(|block_id| {
            let device = Arc::clone(&device);
            thread::spawn(move || {
                device.write_block(block_id, &[block_id as u8; 64]).unwrap();
            })
        })
        .collect();

    for worker in workers {
        worker.join().unwrap();
    }
    for block_id in 0u64..8 {
        assert_eq!(
            device.snapshot_block(block_id).unwrap(),
            vec![block_id as u8; 64]
        );
    }
}
