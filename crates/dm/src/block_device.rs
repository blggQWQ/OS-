use crate::error::{DmError, Result};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub trait BlockDevice: Send + Sync {
    fn block_size(&self) -> usize;
    fn num_blocks(&self) -> u64;
    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()>;
    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()>;
}

#[derive(Clone, Debug)]
pub struct MemoryBlockDevice {
    block_size: usize,
    blocks: Arc<Mutex<Vec<Vec<u8>>>>,
}

#[derive(Debug)]
pub struct FileBlockDevice {
    block_size: usize,
    num_blocks: u64,
    writable: bool,
    file: Mutex<File>,
}

impl MemoryBlockDevice {
    pub fn new(num_blocks: u64, block_size: usize) -> Result<Self> {
        if num_blocks == 0 {
            return Err(DmError::EmptyDevice);
        }
        if block_size == 0 {
            return Err(DmError::InvalidBlockSize {
                expected: 1,
                actual: 0,
            });
        }

        Ok(Self {
            block_size,
            blocks: Arc::new(Mutex::new(vec![vec![0; block_size]; num_blocks as usize])),
        })
    }

    pub fn snapshot_block(&self, block_id: u64) -> Result<Vec<u8>> {
        let blocks = self.blocks.lock().expect("memory block lock poisoned");
        let block = blocks
            .get(block_id as usize)
            .ok_or(DmError::InvalidBlockId {
                block_id,
                blocks: blocks.len() as u64,
            })?;
        Ok(block.clone())
    }
}

impl BlockDevice for MemoryBlockDevice {
    fn block_size(&self) -> usize {
        self.block_size
    }

    fn num_blocks(&self) -> u64 {
        self.blocks.lock().expect("memory block lock poisoned").len() as u64
    }

    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        if buf.len() != self.block_size {
            return Err(DmError::InvalidBlockSize {
                expected: self.block_size,
                actual: buf.len(),
            });
        }

        let blocks = self.blocks.lock().expect("memory block lock poisoned");
        let block = blocks
            .get(block_id as usize)
            .ok_or(DmError::InvalidBlockId {
                block_id,
                blocks: blocks.len() as u64,
            })?;
        buf.copy_from_slice(block);
        Ok(())
    }

    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
        if buf.len() != self.block_size {
            return Err(DmError::InvalidBlockSize {
                expected: self.block_size,
                actual: buf.len(),
            });
        }

        let mut blocks = self.blocks.lock().expect("memory block lock poisoned");
        let len = blocks.len() as u64;
        let block = blocks
            .get_mut(block_id as usize)
            .ok_or(DmError::InvalidBlockId {
                block_id,
                blocks: len,
            })?;
        block.copy_from_slice(buf);
        Ok(())
    }
}

impl FileBlockDevice {
    pub fn create<P: AsRef<Path>>(path: P, num_blocks: u64, block_size: usize) -> Result<Self> {
        if num_blocks == 0 {
            return Err(DmError::EmptyDevice);
        }
        if block_size == 0 {
            return Err(DmError::InvalidBlockSize {
                expected: 1,
                actual: 0,
            });
        }

        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(path)?;
        file.set_len(num_blocks * block_size as u64)?;

        Ok(Self {
            block_size,
            num_blocks,
            writable: true,
            file: Mutex::new(file),
        })
    }

    pub fn open<P: AsRef<Path>>(path: P, block_size: usize, writable: bool) -> Result<Self> {
        if block_size == 0 {
            return Err(DmError::InvalidBlockSize {
                expected: 1,
                actual: 0,
            });
        }

        let file = OpenOptions::new().read(true).write(writable).open(path)?;
        let len = file.metadata()?.len();
        if len == 0 {
            return Err(DmError::EmptyDevice);
        }
        if len % block_size as u64 != 0 {
            return Err(DmError::InvalidImageSize {
                image_size: len,
                block_size,
            });
        }

        Ok(Self {
            block_size,
            num_blocks: len / block_size as u64,
            writable,
            file: Mutex::new(file),
        })
    }

    pub fn snapshot_block(&self, block_id: u64) -> Result<Vec<u8>> {
        let mut buf = vec![0; self.block_size];
        self.read_block(block_id, &mut buf)?;
        Ok(buf)
    }
}

impl BlockDevice for FileBlockDevice {
    fn block_size(&self) -> usize {
        self.block_size
    }

    fn num_blocks(&self) -> u64 {
        self.num_blocks
    }

    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        if buf.len() != self.block_size {
            return Err(DmError::InvalidBlockSize {
                expected: self.block_size,
                actual: buf.len(),
            });
        }
        if block_id >= self.num_blocks {
            return Err(DmError::InvalidBlockId {
                block_id,
                blocks: self.num_blocks,
            });
        }

        let mut file = self.file.lock().expect("file block lock poisoned");
        file.seek(SeekFrom::Start(block_id * self.block_size as u64))?;
        file.read_exact(buf)?;
        Ok(())
    }

    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
        if !self.writable {
            return Err(DmError::ReadOnlyDevice);
        }
        if buf.len() != self.block_size {
            return Err(DmError::InvalidBlockSize {
                expected: self.block_size,
                actual: buf.len(),
            });
        }
        if block_id >= self.num_blocks {
            return Err(DmError::InvalidBlockId {
                block_id,
                blocks: self.num_blocks,
            });
        }

        let mut file = self.file.lock().expect("file block lock poisoned");
        file.seek(SeekFrom::Start(block_id * self.block_size as u64))?;
        file.write_all(buf)?;
        file.flush()?;
        Ok(())
    }
}
