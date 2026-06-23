use crate::block_device::BlockDevice;
use crate::crypto::DemoStreamCipher;
use crate::error::{DmError, Result};

#[derive(Clone, Debug)]
pub struct DmCryptDevice<D> {
    lower: D,
    cipher: DemoStreamCipher,
}

impl<D> DmCryptDevice<D> {
    pub fn new(lower: D, key: &[u8]) -> Result<Self> {
        Ok(Self {
            lower,
            cipher: DemoStreamCipher::new(key)?,
        })
    }

    pub fn lower(&self) -> &D {
        &self.lower
    }
}

impl<D: BlockDevice> BlockDevice for DmCryptDevice<D> {
    fn block_size(&self) -> usize {
        self.lower.block_size()
    }

    fn num_blocks(&self) -> u64 {
        self.lower.num_blocks()
    }

    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        if buf.len() != self.block_size() {
            return Err(DmError::InvalidBlockSize {
                expected: self.block_size(),
                actual: buf.len(),
            });
        }

        let mut encrypted = vec![0; buf.len()];
        self.lower.read_block(block_id, &mut encrypted)?;
        self.cipher.apply(block_id, &encrypted, buf)
    }

    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
        if buf.len() != self.block_size() {
            return Err(DmError::InvalidBlockSize {
                expected: self.block_size(),
                actual: buf.len(),
            });
        }

        let mut encrypted = vec![0; buf.len()];
        self.cipher.apply(block_id, buf, &mut encrypted)?;
        self.lower.write_block(block_id, &encrypted)
    }
}

