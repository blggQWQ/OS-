// 引入项目自定义的错误类型，DmError是各种错误的枚举，Result是DmError的返回结果类型（成功or失败）
use crate::error::{DmError, Result};
// file是已打开文件，OpenOptions是用于配置文件打开选项的结构体，
// Read、Seek、SeekFrom和Write是用于文件读写和定位的trait，
// Path是用于表示文件路径的结构体，
// Arc是用于实现线程安全的引用计数智能指针，
// Mutex是用于实现线程安全的互斥锁。
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};


// 定义一个BlockDevice trait，
// 表示块设备的接口，要求实现Send和Sync trait，以便在多线程环境中安全使用。
pub trait BlockDevice: Send + Sync {
    // 返回每页的大小，单位为字节
    fn block_size(&self) -> usize;
    // 返回块设备的总页数
    fn num_blocks(&self) -> u64;
    // 读取指定页的数据到buf中，buf的长度必须等于block_size
    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()>;
    // 写入指定页的数据，buf的长度必须等于block_size
    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()>;
}

// MemoryBlockDevice —— 用内存模拟的块设备
// 数据全部存在内存中，适合测试和调试使用，不用每次都去读写真实硬盘
// derive(Clone) 让这个结构可以被 .clone() 复制
// derive(Debug) 让这个结构可以用 println!("{:?}", x)
#[derive(Clone, Debug)]
pub struct MemoryBlockDevice {
    block_size: usize,
    // 所有页的数据，从里到外拆解：
    //   u8             = 一个字节（0~255）
    //   Vec<u8>        = 一页的所有字节（一页纸）
    //   Vec<Vec<u8>>   = 所有页（整本笔记本）
    //   Mutex<...>     = 加锁，同一时刻只有一个线程能改（厕所门锁）
    //   Arc<...>       = 多线程共享（全班共用一本教材）
    // 所以 Arc<Mutex<Vec<Vec<u8>>>> = 一本可共享、但同一时刻只有一人能改的笔记本
    blocks: Arc<Mutex<Vec<Vec<u8>>>>,
}

// FileBlockDevice —— 用真实文件模拟的块设备
// 数据全部存在真实文件中，适合持久化存储，但是速度比内存模拟的慢
// 没有 derive(Clone)，文件不能简单地复制共享
#[derive(Debug)]
pub struct FileBlockDevice {
    block_size: usize,//每页大小
    num_blocks: u64,//总页数
    writable: bool,//是否可写
    file: Mutex<File>,//Mutex锁保护。防止多线程同时读写
}

impl MemoryBlockDevice {
    // 创建一个新的内存块设备，指定总页数和每页大小
    //   num_blocks: 要多少页
    //   block_size: 每页多大（字节）
    pub fn new(num_blocks: u64, block_size: usize) -> Result<Self> {
        // 检查总页数是否为0，如果是0则返回错误
        if num_blocks == 0 {
            return Err(DmError::EmptyDevice);
        }
        // 检查每页大小是否为0，如果是0则返回错误
        if block_size == 0 {
            return Err(DmError::InvalidBlockSize {
                expected: 1,
                actual: 0,
            });
        }
        // 检查总页数是否超过usize::MAX，如果超过则返回错误
        let block_count = usize::try_from(num_blocks).map_err(|_| DmError::CapacityOverflow {
            blocks: num_blocks,
            block_size,
        })?;
        // 计算总分配大小 = 总页数 * 每页大小，如果溢出则返回错误
        let allocation_size =
            block_count
                .checked_mul(block_size)
                .ok_or(DmError::CapacityOverflow {
                    blocks: num_blocks,
                    block_size,
                })?;
        // 检查总分配大小是否超过isize::MAX，如果超过则返回错误
        if allocation_size > isize::MAX as usize {
            return Err(DmError::CapacityOverflow {
                blocks: num_blocks,
                block_size,
            });
        }
        // 所有检查通过，创建设备！
        // vec![0; block_size] 创建一页全0的数据（空白页）
        // vec![...; block_count] 创建 block_count 页空白页
        // Mutex::new(...) 加锁
        // Arc::new(...) 可以被多线程共享
        Ok(Self {
            block_size,
            blocks: Arc::new(Mutex::new(vec![vec![0; block_size]; block_count])),
        })
    }
    // snapshot_block 方法用于获取指定页的数据快照，返回一个Vec<u8>，包含该页的所有字节
    pub fn snapshot_block(&self, block_id: u64) -> Result<Vec<u8>> {
        // 获取锁，确保线程安全，只读模式
        let blocks = self.blocks.lock().expect("memory block lock poisoned");
        // 获取指定页的数据，如果页号无效则返回错误
        let block = blocks
            .get(block_id as usize)
            .ok_or(DmError::InvalidBlockId {
                block_id,
                blocks: blocks.len() as u64,
            })?;
        // 返回该页的克隆，确保数据安全
        Ok(block.clone())
    }
}

// 实现BlockDevice trait for MemoryBlockDevice
// 必须实现要求的四个方法：block_size、num_blocks、read_block、write_block
impl BlockDevice for MemoryBlockDevice {
    // 返回每页的大小，单位为字节
    fn block_size(&self) -> usize {
        self.block_size
    }
    // 返回总页数：拿锁->数页数->转成u64返回
    fn num_blocks(&self) -> u64 {
        self.blocks
            .lock()
            .expect("memory block lock poisoned")
            .len() as u64
    }

    // 读取指定页的数据到缓冲区 buf 中
    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        // 检查缓冲区大小是否与每页大小一致，如果不一致则返回错误
        if buf.len() != self.block_size {
            return Err(DmError::InvalidBlockSize {
                expected: self.block_size,
                actual: buf.len(),
            });
        }
        // 获取锁
        let blocks = self.blocks.lock().expect("memory block lock poisoned");
        // 获取指定页的数据，如果页号无效则返回错误
        let block = blocks
            .get(block_id as usize)
            .ok_or(DmError::InvalidBlockId {
                block_id,
                blocks: blocks.len() as u64,
            })?;
        // 将指定页的数据复制到缓冲区 buf 中
        buf.copy_from_slice(block);
        Ok(())
    }

    // 往某一页写内容
    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
        // 检查缓冲区大小是否与每页大小一致，如果不一致则返回错误
        if buf.len() != self.block_size {
            return Err(DmError::InvalidBlockSize {
                expected: self.block_size,
                actual: buf.len(),
            });
        }

        // 拿锁
        let mut blocks = self.blocks.lock().expect("memory block lock poisoned");
        // 记录总页数
        let len = blocks.len() as u64;
        //检查写入页码是否存在，如果不存在则返回错误
        let block = blocks
            .get_mut(block_id as usize)
            .ok_or(DmError::InvalidBlockId {
                block_id,
                blocks: len,
            })?;
        // 将缓冲区 buf 的内容复制到指定页中
        block.copy_from_slice(buf);
        Ok(())
    }
}

impl FileBlockDevice {
    // create():创建一个新的文件块设备
    // 和MemoryBlockDevice的new()不同,这里是在硬盘上创建一个真实文件
    // path：文件路径
    // num_blocks：总页数
    // block_size：每页大小，单位为字节
    pub fn create<P: AsRef<Path>>(path: P, num_blocks: u64, block_size: usize) -> Result<Self> {
        //检查页数不能为0
        if num_blocks == 0 {
            return Err(DmError::EmptyDevice);
        }
        //检查每页大小不能为0
        if block_size == 0 {
            return Err(DmError::InvalidBlockSize {
                expected: 1,
                actual: 0,
            });
        }
        // 打开文件，配置如下：
        // create(true)：如果文件不存在则创建
        // truncate(true)：如果文件已存在则清空内容
        // read(true)：允许读取
        // write(true)：允许写入
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(path)?;

        // 计算文件总大小 = 总页数 * 每页大小，如果溢出则返回错误
        // checked_mul() 方法用于检查乘法是否溢出，溢出则返回 None
        let image_size =
            num_blocks
                .checked_mul(block_size as u64)
                .ok_or(DmError::CapacityOverflow {
                    blocks: num_blocks,
                    block_size,
                })?;
        // 设置文件大小为 image_size，文件内容填充为0
        file.set_len(image_size)?;

        // 创建FileBlockDevice实例
        // 新创建的文件默认可写
        Ok(Self {
            block_size,
            num_blocks,
            writable: true,
            file: Mutex::new(file),
        })
    }

    // open():打开一个已存在的文件块设备
    // 和create()不同,这里是打开一个已存在的文件,而不是创建一个新文件，不会清空文件内容
    // path：文件路径
    // block_size：每页大小，单位为字节
    // writable：是否可写
    pub fn open<P: AsRef<Path>>(path: P, block_size: usize, writable: bool) -> Result<Self> {
        //检查每页大小不能为0
        if block_size == 0 {
            return Err(DmError::InvalidBlockSize {
                expected: 1,
                actual: 0,
            });
        }

        // 打开文件：read(true)始终可读，根据 writable 参数判断是否添加可写权限
        let file = OpenOptions::new().read(true).write(writable).open(path)?;
        // 获取文件大小
        let len = file.metadata()?.len();
        // 如果文件大小为0则返回错误
        if len == 0 {
            return Err(DmError::EmptyDevice);
        }
        // 检查文件大小是否是 block_size 的整数倍，如果不是则返回错误
        if len % block_size as u64 != 0 {
            return Err(DmError::InvalidImageSize {
                image_size: len,
                block_size,
            });
        }
        // 创建FileBlockDevice实例
        // 计算总页数 = 文件大小 / 每页大小
        Ok(Self {
            block_size,
            num_blocks: len / block_size as u64,
            writable,
            file: Mutex::new(file),
        })
    }

    // snapshot_block():获取指定页的数据快照，返回一个Vec<u8>，包含该页的所有字节
    // 和 MemoryBlockDevice 的 snapshot_block 类似，但这里是通过 read_block 来读取
    pub fn snapshot_block(&self, block_id: u64) -> Result<Vec<u8>> {
        // 准备一个空白数组
        let mut buf = vec![0; self.block_size];
        // 调用read_block方法读取指定页的数据到buf中
        self.read_block(block_id, &mut buf)?;
        // 返回读取到的数据
        Ok(buf)
    }
}

// 实现 BlockDevice trait for FileBlockDevice
// 需要满足所有方法，包括 read_block、write_block 等
impl BlockDevice for FileBlockDevice {
    //返回每页大小
    fn block_size(&self) -> usize {
        self.block_size
    }
    //返回总页数
    fn num_blocks(&self) -> u64 {
        self.num_blocks
    }

    // 读取指定页的数据到buf中
    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        // 数组大小必须等于每页大小
        if buf.len() != self.block_size {
            return Err(DmError::InvalidBlockSize {
                expected: self.block_size,
                actual: buf.len(),
            });
        }

        // 检查页号是否超出范围
        if block_id >= self.num_blocks {
            return Err(DmError::InvalidBlockId {
                block_id,
                blocks: self.num_blocks,
            });
        }
        // 拿锁，获得文件使用权
        let mut file = self.file.lock().expect("file block lock poisoned");
        // 定位到指定页的起始位置，block_id * block_size = 偏移量
        file.seek(SeekFrom::Start(block_id * self.block_size as u64))?;
        // 读取指定页的数据到buf中
        // read_exact() 方法会尝试读取 buf.len() 个字节，如果读取不到则返回错误
        file.read_exact(buf)?;
        Ok(())
    }

    // 往文件中写某一页的内容
    // 流程：检查是否可写 → 检查数据大小 → 检查页码 → 跳到指定位置 → 写入数据
    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
        // 检查是否可写
        if !self.writable {
            return Err(DmError::ReadOnlyDevice);
        }
        // 检查数据大小是否等于每页大小
        if buf.len() != self.block_size {
            return Err(DmError::InvalidBlockSize {
                expected: self.block_size,
                actual: buf.len(),
            });
        }
        // 检查页号是否超出范围
        if block_id >= self.num_blocks {
            return Err(DmError::InvalidBlockId {
                block_id,
                blocks: self.num_blocks,
            });
        }

        // 拿锁，获得文件使用权
        let mut file = self.file.lock().expect("file block lock poisoned");
        // 定位到指定页的起始位置，block_id * block_size = 偏移量
        file.seek(SeekFrom::Start(block_id * self.block_size as u64))?;
        // 写入指定页的数据,write_all() 方法会尝试写入 buf.len() 个字节，如果写入不到则返回错误
        file.write_all(buf)?;
        // 刷新文件缓冲区，确保数据写入磁盘
        file.flush()?;
        Ok(())
    }
}


impl<D: BlockDevice + ?Sized> BlockDevice for Arc<D> {
    fn block_size(&self) -> usize {
        (**self).block_size()
    }

    fn num_blocks(&self) -> u64 {
        (**self).num_blocks()
    }

    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        (**self).read_block(block_id, buf)
    }

    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
        (**self).write_block(block_id, buf)
    }
}
