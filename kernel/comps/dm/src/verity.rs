// SPDX-License-Identifier: MPL-2.0

use alloc::{vec, vec::Vec};

use aster_block::{
    BLOCK_SIZE, BlockDevice, SECTOR_SIZE,
    bio::{BioDirection, BioSegment, BioStatus},
    id::Bid,
};
use ostd::mm::VmIo;

use crate::{DmError, sha256::sha256};

const HASH_SIZE: usize = 32;
const HASHES_PER_BLOCK: usize = BLOCK_SIZE / HASH_SIZE;

/// Verifies data blocks against an on-disk SHA-256 Merkle tree.
pub(crate) struct VerityTarget {
    data_blocks: usize,
    root_hash: [u8; HASH_SIZE],
    levels: Vec<HashLevel>,
}

impl VerityTarget {
    /// Creates the level layout for a packed on-disk hash tree.
    pub(crate) fn new(
        data_blocks: usize,
        hash_start_block: usize,
        root_hash: [u8; HASH_SIZE],
    ) -> Result<Self, DmError> {
        if data_blocks == 0 {
            return Err(DmError::OutOfRange);
        }

        let mut levels = Vec::new();
        let mut digest_count = data_blocks;
        let mut block_offset = hash_start_block;
        loop {
            let block_count = digest_count.div_ceil(HASHES_PER_BLOCK);
            levels.push(HashLevel {
                block_offset,
                digest_count,
            });
            block_offset = block_offset
                .checked_add(block_count)
                .ok_or(DmError::OutOfRange)?;
            if block_count == 1 {
                break;
            }
            digest_count = block_count;
        }

        Ok(Self {
            data_blocks,
            root_hash,
            levels,
        })
    }

    pub(crate) fn data_sectors(&self) -> usize {
        self.data_blocks * (BLOCK_SIZE / SECTOR_SIZE)
    }

    pub(crate) fn verify_range(
        &self,
        lower: &dyn BlockDevice,
        first_sector: u64,
        num_sectors: u64,
    ) -> Result<(), DmError> {
        let start_byte = first_sector
            .checked_mul(SECTOR_SIZE as u64)
            .ok_or(DmError::OutOfRange)?;
        let end_byte = first_sector
            .checked_add(num_sectors)
            .and_then(|end| end.checked_mul(SECTOR_SIZE as u64))
            .ok_or(DmError::OutOfRange)?;
        let first_block = start_byte / BLOCK_SIZE as u64;
        let end_block = end_byte.div_ceil(BLOCK_SIZE as u64);
        if end_block > self.data_blocks as u64 {
            return Err(DmError::OutOfRange);
        }

        for block_id in first_block..end_block {
            self.verify_block(lower, block_id as usize)?;
        }
        Ok(())
    }

    fn verify_block(&self, lower: &dyn BlockDevice, block_id: usize) -> Result<(), DmError> {
        let data = read_block(lower, block_id)?;
        let mut digest = sha256(&data);
        let mut digest_index = block_id;

        for level in &self.levels {
            if digest_index >= level.digest_count {
                return Err(DmError::OutOfRange);
            }
            let hash_block_index = digest_index / HASHES_PER_BLOCK;
            let digest_slot = digest_index % HASHES_PER_BLOCK;
            let hash_block = read_block(lower, level.block_offset + hash_block_index)?;
            let digest_offset = digest_slot * HASH_SIZE;
            if hash_block[digest_offset..digest_offset + HASH_SIZE] != digest {
                return Err(DmError::IntegrityViolation);
            }
            digest = sha256(&hash_block);
            digest_index = hash_block_index;
        }

        if digest != self.root_hash {
            return Err(DmError::IntegrityViolation);
        }
        Ok(())
    }
}

struct HashLevel {
    block_offset: usize,
    digest_count: usize,
}

fn read_block(lower: &dyn BlockDevice, block_id: usize) -> Result<Vec<u8>, DmError> {
    let segment = BioSegment::alloc(1, BioDirection::FromDevice);
    let status = lower
        .read_blocks(Bid::new(block_id as u64), segment.clone())
        .map_err(|_| DmError::Io)?;
    if status != BioStatus::Complete {
        return Err(DmError::Io);
    }
    let mut block = vec![0u8; BLOCK_SIZE];
    segment
        .read_bytes(0, &mut block)
        .map_err(|_| DmError::Io)?;
    Ok(block)
}
