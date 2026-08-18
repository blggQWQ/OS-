// 安全边界测试——不测正常功能，专测危险边界
use dm::{BlockDevice, DmCryptDevice, DmCryptTable, DmError, MemoryBlockDevice, Result};
use std::fmt::Write as _;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;

fn key_hex() -> String {
    // 生成一个64字节的十六进制密钥，用于AES-XTS加密测试
    (0u8..64).fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    })
}

// 假装有u64::MAX个块的设备——实际上不存任何数据
// 专门用来测试"块数极大时，计算会不会溢出"
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
        buf.fill(0); // 不存数据，读出来全零
        Ok(())
    }

    fn write_block(&self, _block_id: u64, _buf: &[u8]) -> Result<()> {
        Ok(()) // 啥都不做
    }
}

#[test]
fn rejects_capacity_and_sector_number_overflow_without_panicking() {
    // 测试容量溢出和扇区号溢出时，报错而不是panic崩溃
    // 如果溢出时panic，攻击者可以故意传入超大块号让程序崩溃（DoS攻击）
    // 返回错误 = 安全地拒绝了恶意输入

    // ① 容量溢出：u64::MAX × 4096 远超内存容量
    assert!(matches!(
        MemoryBlockDevice::new(u64::MAX, 4096),
        Err(DmError::CapacityOverflow { .. })
    ));

    // ② 扇区号溢出：块号 × 每块扇区数 超过 u64 范围
    let lower = HugeLogicalDevice {
        blocks: u64::MAX,
        block_size: 4096,
    };
    let table = DmCryptTable::from_str(&format!("aes-xts-plain64 {} 0 huge 0", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower, &table).unwrap();
    let mut block = [0u8; 4096];
    let overflowing_block = u64::MAX / 8 + 1; // 这个块号 × 8 会溢出
    assert!(matches!(
        crypt.read_block(overflowing_block, &mut block),
        Err(DmError::InvalidTable(message)) if message.contains("overflow")
    ));
}

#[test]
fn same_plaintext_uses_different_ciphertext_in_different_sectors() {
    // 测试相同明文写入不同扇区，密文必须不同
    // 这是AES-XTS的tweak机制在起作用——不同扇区号产生不同tweak
    // 可以减小被攻击破译密码的风险，因为相同明文存入不同的磁盘
    // 所加密出来的密文会根据扇区号的不同而不同
    // 这也是为什么选择AES-XTS模式的原因
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
    // 测试缓冲区大小不对和越界访问都会报错
    // 缓冲区大小不匹配 → 可能导致缓冲区溢出（经典安全漏洞）
    // 块号越界 → 可能读到别的数据或写入不该写的位置

    let lower = MemoryBlockDevice::new(1, 512).unwrap();
    let table =
        DmCryptTable::from_str(&format!("aes-xts-plain64 {} 0 memory 0", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower, &table).unwrap();

    // ① 缓冲区太小（511 ≠ 512）
    let mut short = [0u8; 511];
    assert!(matches!(
        crypt.read_block(0, &mut short),
        Err(DmError::InvalidBlockSize { .. })
    ));

    // ② 块号越界（设备只有1块，读块1）
    let mut block = [0u8; 512];
    assert!(matches!(
        crypt.read_block(1, &mut block),
        Err(DmError::InvalidBlockId { .. })
    ));
}

#[test]
fn memory_backend_supports_concurrent_disjoint_io() {
    // 测试多线程并发读写不同块，不会数据错乱
    // MemoryBlockDevice内部用Arc<Mutex<...>>保护数据
    // 如果锁有问题，多线程并发时可能：
    //   - 数据写串（线程A的数据跑到线程B的块里）
    //   - 数据竞争（读到半写半不写的状态）
    let device = Arc::new(MemoryBlockDevice::new(8, 64).unwrap());
    let workers: Vec<_> = (0u64..8)
        .map(|block_id| {
            let device = Arc::clone(&device);
            thread::spawn(move || {
                // 8个线程同时写8个不同的块
                // 线程0写块0，线程1写块1，...
                device.write_block(block_id, &[block_id as u8; 64]).unwrap();
            })
        })
        .collect();

    for worker in workers {
        worker.join().unwrap();
    }

    // 写完后逐块验证数据是否正确
    for block_id in 0u64..8 {
        assert_eq!(
            device.snapshot_block(block_id).unwrap(),
            vec![block_id as u8; 64]
        );
    }
}

#[test]
fn write_overflow_returns_error_without_panicking() {
    // 测试写入时扇区号溢出也报错而不是panic
    // 和read_overflow测试配对，确保读写两个方向都安全
    let lower = HugeLogicalDevice {
        blocks: u64::MAX,
        block_size: 4096,
    };
    let table = DmCryptTable::from_str(&format!("aes-xts-plain64 {} 0 huge 0", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower, &table).unwrap();
    let block = [0u8; 4096];
    let overflowing_block = u64::MAX / 8 + 1;
    assert!(matches!(
        crypt.write_block(overflowing_block, &block),
        Err(DmError::InvalidTable(message)) if message.contains("overflow")
    ));
}