use crate::block_device::BlockDevice;
use crate::error::Result;
use crate::{DmCryptDevice, DmCryptTable, DmVerityDevice, HashTree};
use std::sync::Arc;

pub type DynBlockDevice = Arc<dyn BlockDevice>;

/// Security and mutability properties contributed by one mapper target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetCapabilities {
    pub confidentiality: bool,
    pub integrity: bool,
    pub read_only: bool,
}

/// Describes one target in a dynamically composed mapper stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DmLayer {
    pub name: String,
    pub target: &'static str,
    pub capabilities: TargetCapabilities,
}

/// A type-erased Device Mapper stack assembled from safe Rust targets.
pub struct DmStack {
    top: DynBlockDevice,
    layers: Vec<DmLayer>,
}

/// Builds target stacks while preserving their order and security properties.
pub struct DmStackBuilder {
    top: DynBlockDevice,
    layers: Vec<DmLayer>,
}

impl DmStackBuilder {
    pub fn new<D: BlockDevice + 'static>(lower: D) -> Self {
        Self {
            top: Arc::new(lower),
            layers: Vec::new(),
        }
    }

    pub fn passthrough(mut self, name: impl Into<String>) -> Self {
        self.top = Arc::new(PassthroughDevice::new(self.top));
        self.layers.push(DmLayer {
            name: name.into(),
            target: "passthrough",
            capabilities: TargetCapabilities {
                confidentiality: false,
                integrity: false,
                read_only: false,
            },
        });
        self
    }

    pub fn crypt(mut self, name: impl Into<String>, table: &DmCryptTable) -> Result<Self> {
        self.top = Arc::new(DmCryptDevice::from_table(self.top, table)?);
        self.layers.push(DmLayer {
            name: name.into(),
            target: "crypt",
            capabilities: TargetCapabilities {
                confidentiality: true,
                integrity: false,
                read_only: false,
            },
        });
        Ok(self)
    }

    pub fn verity(mut self, name: impl Into<String>, tree: HashTree, root_hash: [u8; 32]) -> Self {
        self.top = Arc::new(DmVerityDevice::open(self.top, tree, root_hash));
        self.layers.push(DmLayer {
            name: name.into(),
            target: "verity",
            capabilities: TargetCapabilities {
                confidentiality: false,
                integrity: true,
                read_only: true,
            },
        });
        self
    }

    pub fn build(self) -> DmStack {
        DmStack {
            top: self.top,
            layers: self.layers,
        }
    }
}

impl DmStack {
    pub fn layers(&self) -> &[DmLayer] {
        &self.layers
    }

    pub fn capabilities(&self) -> TargetCapabilities {
        self.layers.iter().fold(
            TargetCapabilities {
                confidentiality: false,
                integrity: false,
                read_only: false,
            },
            |combined, layer| TargetCapabilities {
                confidentiality: combined.confidentiality || layer.capabilities.confidentiality,
                integrity: combined.integrity || layer.capabilities.integrity,
                read_only: combined.read_only || layer.capabilities.read_only,
            },
        )
    }
}

impl BlockDevice for DmStack {
    fn block_size(&self) -> usize {
        self.top.block_size()
    }

    fn num_blocks(&self) -> u64 {
        self.top.num_blocks()
    }

    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        self.top.read_block(block_id, buf)
    }

    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
        self.top.write_block(block_id, buf)
    }
}

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
