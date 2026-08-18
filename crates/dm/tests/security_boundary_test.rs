// 安全边界测试
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
}// 生成一个64字节的随机密钥，用于加密测试

#[derive(Clone)]
struct HugeLogicalDevice {
    blocks: u64,
    block_size: usize,
}// 用来测试块数极大时，计算会不会溢出

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
fn rejects_capacity_and_sector_number_overflow_without_panicking() {// 测试容量和扇区号溢出是否为报错而不是崩溃
    assert!(matches!(
        MemoryBlockDevice::new(u64::MAX, 4096),
        Err(DmError::CapacityOverflow { .. })
    ));//此为容量溢出

    let lower = HugeLogicalDevice {
        blocks: u64::MAX,
        block_size: 4096,
    };
    let table = DmCryptTable::from_str(&format!("aes-xts-plain64 {} 0 huge 0", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower, &table).unwrap();
    let mut block = [0u8; 4096];
    let overflowing_block = u64::MAX / 8 + 1;//此为扇区号溢出
    assert!(matches!(
        crypt.read_block(overflowing_block, &mut block),
        Err(DmError::InvalidTable(message)) if message.contains("overflow")
    ));
}//如果溢出时 panic，攻击者可以故意传入超大块号让程序崩溃（DoS 攻击）。返回错误 = 安全地拒绝了恶意输入

#[test]
fn same_plaintext_uses_different_ciphertext_in_different_sectors() {// 测试相同的明文是否在不同的扇区使用不同的密文
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
}// 可以减小被攻击破译密码的风险，因为相同明文存入不同的磁盘所加密出来的密文会根据扇区号的不同而不同。这个也是
//为什么选择AES-XTS模式的原因。而AES-CBC和AES-ECB模式不能提供这种保护，因为他们的加密始终是伴随着其他扇区的明文或者密文

#[test]
fn rejects_unaligned_io_and_out_of_range_block() {// 测试缓冲区大小不对和越界访问是否会报错
    let lower = MemoryBlockDevice::new(1, 512).unwrap();
    let table =
        DmCryptTable::from_str(&format!("aes-xts-plain64 {} 0 memory 0", key_hex())).unwrap();
    let crypt = DmCryptDevice::from_table(lower, &table).unwrap();

    let mut short = [0u8; 511];
    assert!(matches!(
        crypt.read_block(0, &mut short),
        Err(DmError::InvalidBlockSize { .. })
    ));//此为缓冲区大小不对
    let mut block = [0u8; 512];
    assert!(matches!(
        crypt.read_block(1, &mut block),
        Err(DmError::InvalidBlockId { .. })
    ));//此为越界访问
}

#[test]
fn memory_backend_supports_concurrent_disjoint_io() {// 测试多线程并发读写不同块，不会数据错乱
    let device = Arc::new(MemoryBlockDevice::new(8, 64).unwrap());
    let workers: Vec<_> = (0u64..8)
        .map(|block_id| {
            let device = Arc::clone(&device);
            thread::spawn(move || {
                device.write_block(block_id, &[block_id as u8; 64]).unwrap();
            })
        })
        .collect();
        // 8个线程同时写8个不同的块
        //线程0写块0，线程1写块1，线程2写块2，线程3写块3，线程4写块4，线程5写块5，线程6写块6，线程7写块7
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
// 写完后逐块验证数据是否正确
// 测试这个是因为MemoryBlockDevice 内部用 Arc<Mutex<...>> 保护数据。如果锁有问题，多线程并发时可能：

// 数据写串（线程 A 的数据跑到线程 B 的块里）
// 数据竞争（读到半写半不写的状态）
