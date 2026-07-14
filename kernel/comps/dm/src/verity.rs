// SPDX-License-Identifier: MPL-2.0

use alloc::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    vec::Vec,
};
use core::sync::atomic::{AtomicU64, Ordering};

use aster_block::{
    BLOCK_SIZE, BlockDevice, SECTOR_SIZE,
    bio::{BioDirection, BioSegment, BioStatus},
    id::Bid,
};
use ostd::mm::VmIo;
use spin::Mutex;

use crate::{DmError, sha256::sha256};

const HASH_SIZE: usize = 32;
const HASHES_PER_BLOCK: usize = BLOCK_SIZE / HASH_SIZE;

/// A point-in-time snapshot of dm-verity's hot-path counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VerityStats {
    /// Number of data blocks authenticated successfully.
    pub verified_data_blocks: u64,
    /// Number of data blocks authenticated directly from the completed BIO.
    pub bio_data_blocks: u64,
    /// Number of data blocks reread for partial or unaligned BIOs.
    pub data_reread_blocks: u64,
    /// Number of authenticated hash blocks served from the cache.
    pub hash_cache_hits: u64,
    /// Number of hash blocks fetched from the lower device.
    pub hash_cache_misses: u64,
}

/// Verifies data blocks against an on-disk SHA-256 Merkle tree.
pub(crate) struct VerityTarget {
    data_blocks: usize,
    root_hash: [u8; HASH_SIZE],
    levels: Vec<HashLevel>,
    hash_cache: Mutex<VerifiedHashBlockCache>,
    stats: VerityAtomicStats,
}

impl VerityTarget {
    pub(crate) fn new_with_cache(
        data_blocks: usize,
        hash_start_block: usize,
        root_hash: [u8; HASH_SIZE],
        hash_cache_blocks: usize,
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
            hash_cache: Mutex::new(VerifiedHashBlockCache::new(hash_cache_blocks)),
            stats: VerityAtomicStats::default(),
        })
    }

    pub(crate) fn data_sectors(&self) -> usize {
        self.data_blocks * (BLOCK_SIZE / SECTOR_SIZE)
    }

    /// Authenticates a completed BIO without rereading its full data blocks.
    ///
    /// Partial-block BIOs use the conservative lower-device path because a
    /// SHA-256 digest must cover the complete 4-KiB block.
    pub(crate) fn verify_segments(
        &self,
        lower: &dyn BlockDevice,
        first_sector: u64,
        segments: &[BioSegment],
    ) -> Result<(), DmError> {
        let start_byte = first_sector
            .checked_mul(SECTOR_SIZE as u64)
            .ok_or(DmError::OutOfRange)?;
        let total_bytes = segments.iter().try_fold(0usize, |total, segment| {
            total
                .checked_add(segment.nbytes())
                .ok_or(DmError::OutOfRange)
        })?;
        if !start_byte.is_multiple_of(BLOCK_SIZE as u64) || !total_bytes.is_multiple_of(BLOCK_SIZE)
        {
            return self.verify_range(lower, first_sector, total_bytes);
        }

        let first_block =
            usize::try_from(start_byte / BLOCK_SIZE as u64).map_err(|_| DmError::OutOfRange)?;
        let requested_blocks = total_bytes / BLOCK_SIZE;
        let end_block = first_block
            .checked_add(requested_blocks)
            .ok_or(DmError::OutOfRange)?;
        if end_block > self.data_blocks {
            return Err(DmError::OutOfRange);
        }

        let mut data_block = [0u8; BLOCK_SIZE];
        let mut bytes_in_block = 0usize;
        let mut block_id = first_block;
        for segment in segments {
            let mut segment_offset = 0usize;
            while segment_offset < segment.nbytes() {
                let copy_len = (BLOCK_SIZE - bytes_in_block).min(segment.nbytes() - segment_offset);
                segment
                    .inner_dma_slice()
                    .read_bytes(
                        segment_offset,
                        &mut data_block[bytes_in_block..bytes_in_block + copy_len],
                    )
                    .map_err(|_| DmError::Io)?;
                segment_offset += copy_len;
                bytes_in_block += copy_len;

                if bytes_in_block == BLOCK_SIZE {
                    self.verify_data_block(lower, block_id, &data_block)?;
                    self.stats.bio_data_blocks.fetch_add(1, Ordering::Relaxed);
                    block_id += 1;
                    bytes_in_block = 0;
                }
            }
        }
        debug_assert_eq!(bytes_in_block, 0);
        debug_assert_eq!(block_id, end_block);
        Ok(())
    }

    pub(crate) fn stats(&self) -> VerityStats {
        self.stats.snapshot()
    }

    fn verify_range(
        &self,
        lower: &dyn BlockDevice,
        first_sector: u64,
        num_bytes: usize,
    ) -> Result<(), DmError> {
        let start_byte = first_sector
            .checked_mul(SECTOR_SIZE as u64)
            .ok_or(DmError::OutOfRange)?;
        let end_byte = start_byte
            .checked_add(num_bytes as u64)
            .ok_or(DmError::OutOfRange)?;
        let first_block = start_byte / BLOCK_SIZE as u64;
        let end_block = end_byte.div_ceil(BLOCK_SIZE as u64);
        if end_block > self.data_blocks as u64 {
            return Err(DmError::OutOfRange);
        }

        self.stats
            .data_reread_blocks
            .fetch_add(end_block - first_block, Ordering::Relaxed);
        for block_id in first_block..end_block {
            let data = read_block(lower, block_id as usize)?;
            self.verify_data_block(lower, block_id as usize, &data)?;
        }
        Ok(())
    }

    fn verify_data_block(
        &self,
        lower: &dyn BlockDevice,
        block_id: usize,
        data: &[u8; BLOCK_SIZE],
    ) -> Result<(), DmError> {
        let mut digest = sha256(data);
        let mut digest_index = block_id;
        let mut newly_authenticated = Vec::new();

        for level in &self.levels {
            if digest_index >= level.digest_count {
                return Err(DmError::OutOfRange);
            }
            let hash_block_index = digest_index / HASHES_PER_BLOCK;
            let digest_slot = digest_index % HASHES_PER_BLOCK;
            let physical_block = level.block_offset + hash_block_index;
            let hash_block = if let Some(cached) = self.hash_cache.lock().get(physical_block) {
                self.stats.hash_cache_hits.fetch_add(1, Ordering::Relaxed);
                cached
            } else {
                self.stats.hash_cache_misses.fetch_add(1, Ordering::Relaxed);
                let fetched = Arc::new(read_block(lower, physical_block)?);
                newly_authenticated.push((physical_block, Arc::clone(&fetched)));
                fetched
            };
            let digest_offset = digest_slot * HASH_SIZE;
            if hash_block[digest_offset..digest_offset + HASH_SIZE] != digest {
                return Err(DmError::IntegrityViolation);
            }
            digest = sha256(hash_block.as_ref());
            digest_index = hash_block_index;
        }

        if digest != self.root_hash {
            return Err(DmError::IntegrityViolation);
        }
        // Cache insertion happens only after the complete path reaches the
        // trusted root, so an unauthenticated hash block is never reused.
        let mut cache = self.hash_cache.lock();
        for (block_id, block) in newly_authenticated {
            cache.insert_authenticated(block_id, block);
        }
        self.stats
            .verified_data_blocks
            .fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

struct HashLevel {
    block_offset: usize,
    digest_count: usize,
}

struct VerifiedHashBlockCache {
    blocks: BTreeMap<usize, Arc<[u8; BLOCK_SIZE]>>,
    insertion_order: VecDeque<usize>,
    capacity: usize,
}

impl VerifiedHashBlockCache {
    fn new(capacity: usize) -> Self {
        Self {
            blocks: BTreeMap::new(),
            insertion_order: VecDeque::new(),
            capacity,
        }
    }

    fn get(&self, block_id: usize) -> Option<Arc<[u8; BLOCK_SIZE]>> {
        self.blocks.get(&block_id).map(Arc::clone)
    }

    fn insert_authenticated(&mut self, block_id: usize, block: Arc<[u8; BLOCK_SIZE]>) {
        if self.capacity == 0 || self.blocks.contains_key(&block_id) {
            return;
        }
        if self.blocks.len() == self.capacity {
            if let Some(evicted) = self.insertion_order.pop_front() {
                self.blocks.remove(&evicted);
            }
        }
        self.blocks.insert(block_id, block);
        self.insertion_order.push_back(block_id);
    }
}

#[derive(Default)]
struct VerityAtomicStats {
    verified_data_blocks: AtomicU64,
    bio_data_blocks: AtomicU64,
    data_reread_blocks: AtomicU64,
    hash_cache_hits: AtomicU64,
    hash_cache_misses: AtomicU64,
}

impl VerityAtomicStats {
    fn snapshot(&self) -> VerityStats {
        VerityStats {
            verified_data_blocks: self.verified_data_blocks.load(Ordering::Relaxed),
            bio_data_blocks: self.bio_data_blocks.load(Ordering::Relaxed),
            data_reread_blocks: self.data_reread_blocks.load(Ordering::Relaxed),
            hash_cache_hits: self.hash_cache_hits.load(Ordering::Relaxed),
            hash_cache_misses: self.hash_cache_misses.load(Ordering::Relaxed),
        }
    }
}

fn read_block(lower: &dyn BlockDevice, block_id: usize) -> Result<[u8; BLOCK_SIZE], DmError> {
    let segment = BioSegment::alloc(1, BioDirection::FromDevice);
    let status = lower
        .read_blocks(Bid::new(block_id as u64), segment.clone())
        .map_err(|_| DmError::Io)?;
    if status != BioStatus::Complete {
        return Err(DmError::Io);
    }
    let mut block = [0u8; BLOCK_SIZE];
    segment.read_bytes(0, &mut block).map_err(|_| DmError::Io)?;
    Ok(block)
}
