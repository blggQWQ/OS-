use crate::block_device::BlockDevice;
use crate::error::Result;

#[derive(Clone, Debug)]
pub struct PassthroughDevice<D> {
    lower: D,
}

impl<D> PassthroughDevice<D> {
    pub fn new(lower: D) -> Self {
        Self { lower }
    }

    pub fn lower(&self) -> &D {
        &self.lower
    }
}

impl<D: BlockDevice> BlockDevice for PassthroughDevice<D> {
    fn block_size(&self) -> usize {
        self.lower.block_size()
    }

    fn num_blocks(&self) -> u64 {
        self.lower.num_blocks()
    }

    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        self.lower.read_block(block_id, buf)
    }

    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
        self.lower.write_block(block_id, buf)
    }
}

