//! Asterinas virtio-blk adapter skeleton.
//!
//! Copy this idea into the Asterinas block layer and replace the placeholder
//! calls with the real block-device API of the Asterinas version in use.

use dm::{BlockDevice, DmError, Result};

pub struct VirtioBlkAdapter<T> {
    inner: T,
    block_size: usize,
    num_blocks: u64,
}

impl<T> VirtioBlkAdapter<T> {
    pub fn new(inner: T, block_size: usize, num_blocks: u64) -> Self {
        Self {
            inner,
            block_size,
            num_blocks,
        }
    }
}

impl<T> BlockDevice for VirtioBlkAdapter<T>
where
    T: Send + Sync,
{
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

        // Replace this with the real Asterinas virtio-blk read call.
        // Example shape:
        // self.inner.read_blocks(block_id, buf).map_err(map_asterinas_error)?;
        let _ = &self.inner;
        unimplemented!("wire this to Asterinas virtio-blk read");
    }

    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
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

        // Replace this with the real Asterinas virtio-blk write call.
        // Example shape:
        // self.inner.write_blocks(block_id, buf).map_err(map_asterinas_error)?;
        let _ = &self.inner;
        unimplemented!("wire this to Asterinas virtio-blk write");
    }
}

