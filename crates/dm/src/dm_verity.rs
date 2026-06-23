use crate::block_device::BlockDevice;
use crate::error::{DmError, Result};
use crate::hash_tree::HashTree;

#[derive(Clone, Debug)]
pub struct DmVerityDevice<D> {
    lower: D,
    tree: HashTree,
    root_hash: [u8; 32],
}

impl<D: BlockDevice> DmVerityDevice<D> {
    pub fn build(lower: D) -> Result<Self> {
        let tree = build_tree_from_device(&lower)?;
        let root_hash = tree.root_hash();
        Ok(Self {
            lower,
            tree,
            root_hash,
        })
    }

    pub fn open(lower: D, tree: HashTree, root_hash: [u8; 32]) -> Self {
        Self {
            lower,
            tree,
            root_hash,
        }
    }

    pub fn root_hash(&self) -> [u8; 32] {
        self.root_hash
    }

    pub fn tree(&self) -> &HashTree {
        &self.tree
    }

    pub fn lower(&self) -> &D {
        &self.lower
    }
}

impl<D: BlockDevice> BlockDevice for DmVerityDevice<D> {
    fn block_size(&self) -> usize {
        self.lower.block_size()
    }

    fn num_blocks(&self) -> u64 {
        self.lower.num_blocks()
    }

    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        self.lower.read_block(block_id, buf)?;
        let verified_tree = self.tree.verify_block(block_id, buf);
        if verified_tree && self.tree.root_hash() == self.root_hash {
            Ok(())
        } else {
            Err(DmError::IntegrityViolation { block_id })
        }
    }

    fn write_block(&self, _block_id: u64, _buf: &[u8]) -> Result<()> {
        Err(DmError::ReadOnlyDevice)
    }
}

fn build_tree_from_device<D: BlockDevice>(device: &D) -> Result<HashTree> {
    let mut blocks = Vec::with_capacity(device.num_blocks() as usize);
    for block_id in 0..device.num_blocks() {
        let mut block = vec![0; device.block_size()];
        device.read_block(block_id, &mut block)?;
        blocks.push(block);
    }
    Ok(HashTree::build(&blocks))
}

