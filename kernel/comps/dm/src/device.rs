// SPDX-License-Identifier: MPL-2.0

use alloc::{boxed::Box, string::String, sync::Arc, vec, vec::Vec};
use core::fmt::Debug;

use aster_block::{
    BlockDevice, BlockDeviceMeta,
    bio::{Bio, BioDirection, BioEnqueueError, BioSegment, BioStatus, BioType, SubmittedBio},
    id::{Bid, Sid},
};
use device_id::DeviceId;
use ostd::mm::VmIo;
use spin::Mutex;

use crate::{
    DmError,
    crypto::AesXtsCipher,
    statistics::{DmAtomicPerformanceStats, DmPerformanceStats, IoStage},
    verity::{VerityStats, VerityTarget},
};

/// A block-device mapper backed by another Asterinas block device.
pub struct DmDevice {
    id: DeviceId,
    name: String,
    lower: Arc<dyn BlockDevice>,
    target: DmTarget,
    performance_stats: Arc<DmAtomicPerformanceStats>,
}

impl DmDevice {
    /// Creates a passthrough mapper.
    pub fn new_passthrough(id: DeviceId, name: String, lower: Arc<dyn BlockDevice>) -> Self {
        Self {
            id,
            name,
            lower,
            target: DmTarget::Passthrough,
            performance_stats: Arc::new(DmAtomicPerformanceStats::default()),
        }
    }

    /// Creates an AES-256-XTS mapper.
    pub fn new_crypt(
        id: DeviceId,
        name: String,
        lower: Arc<dyn BlockDevice>,
        key: &[u8],
    ) -> Result<Self, DmError> {
        Self::new_crypt_with_pool(id, name, lower, key, 32)
    }

    /// Creates an AES-256-XTS mapper with a bounded scratch-buffer pool.
    pub fn new_crypt_with_pool(
        id: DeviceId,
        name: String,
        lower: Arc<dyn BlockDevice>,
        key: &[u8],
        scratch_pool_capacity: usize,
    ) -> Result<Self, DmError> {
        let logical_sectors = lower.metadata().nr_sectors;
        Self::new_crypt_table_with_pool(
            id,
            name,
            lower,
            key,
            0,
            0,
            logical_sectors,
            scratch_pool_capacity,
        )
    }

    /// Creates an AES-256-XTS mapper from Linux dm-crypt table geometry.
    pub fn new_crypt_table(
        id: DeviceId,
        name: String,
        lower: Arc<dyn BlockDevice>,
        key: &[u8],
        iv_offset: u64,
        data_offset_sectors: u64,
        logical_sectors: usize,
    ) -> Result<Self, DmError> {
        Self::new_crypt_table_with_pool(
            id,
            name,
            lower,
            key,
            iv_offset,
            data_offset_sectors,
            logical_sectors,
            32,
        )
    }

    /// Creates an AES-256-XTS mapper with Linux table geometry and scratch pooling.
    pub fn new_crypt_table_with_pool(
        id: DeviceId,
        name: String,
        lower: Arc<dyn BlockDevice>,
        key: &[u8],
        iv_offset: u64,
        data_offset_sectors: u64,
        logical_sectors: usize,
        scratch_pool_capacity: usize,
    ) -> Result<Self, DmError> {
        let lower_sectors = lower.metadata().nr_sectors as u64;
        let logical_end = data_offset_sectors
            .checked_add(logical_sectors as u64)
            .ok_or(DmError::OutOfRange)?;
        if logical_sectors == 0 || logical_end > lower_sectors {
            return Err(DmError::OutOfRange);
        }
        Ok(Self {
            id,
            name,
            lower,
            target: DmTarget::Crypt(CryptTarget {
                cipher: Arc::new(AesXtsCipher::new(key)?),
                iv_offset,
                data_offset_sectors,
                logical_sectors,
                scratch_pool: Arc::new(Mutex::new(CryptScratchPool::new(scratch_pool_capacity))),
            }),
            performance_stats: Arc::new(DmAtomicPerformanceStats::default()),
        })
    }

    /// Creates a read-only dm-verity mapper with an on-disk hash tree.
    pub fn new_verity(
        id: DeviceId,
        name: String,
        lower: Arc<dyn BlockDevice>,
        data_blocks: usize,
        hash_start_block: usize,
        root_hash: [u8; 32],
    ) -> Result<Self, DmError> {
        Self::new_verity_with_cache(
            id,
            name,
            lower,
            data_blocks,
            hash_start_block,
            root_hash,
            256,
        )
    }

    /// Creates a dm-verity mapper with an explicitly sized authenticated cache.
    pub fn new_verity_with_cache(
        id: DeviceId,
        name: String,
        lower: Arc<dyn BlockDevice>,
        data_blocks: usize,
        hash_start_block: usize,
        root_hash: [u8; 32],
        hash_cache_blocks: usize,
    ) -> Result<Self, DmError> {
        Ok(Self {
            id,
            name,
            lower,
            target: DmTarget::Verity(VerityTarget::new_with_cache(
                data_blocks,
                hash_start_block,
                root_hash,
                hash_cache_blocks,
            )?),
            performance_stats: Arc::new(DmAtomicPerformanceStats::default()),
        })
    }

    /// Reads the first data block through this mapper.
    ///
    /// This is used as a boot-time smoke test by the dm-verity integration
    /// path. The read traverses the regular BIO path and therefore exercises
    /// both the lower device and the on-disk hash-tree verification.
    pub fn self_test_first_block(&self) -> Result<(), DmError> {
        let segment = BioSegment::alloc(1, BioDirection::FromDevice);
        let device: &dyn BlockDevice = self;
        let status = device
            .read_blocks(Bid::new(0), segment)
            .map_err(|_| DmError::Io)?;
        if status == BioStatus::Complete {
            Ok(())
        } else {
            Err(DmError::IntegrityViolation)
        }
    }

    /// Returns dm-verity hot-path counters when this mapper uses that target.
    pub fn verity_stats(&self) -> Option<VerityStats> {
        let DmTarget::Verity(target) = &self.target else {
            return None;
        };
        Some(target.stats())
    }

    /// Returns cumulative, opt-in stage timings for this mapper.
    pub fn performance_stats(&self) -> DmPerformanceStats {
        self.performance_stats.snapshot()
    }

    /// Reads and validates the ext2 superblock magic through this mapper.
    ///
    /// The magic is located 56 bytes into the superblock, which starts at
    /// byte 1024. Seeing it through an encrypted lower device proves that the
    /// regular BIO path performed AES-XTS decryption with the expected key and
    /// plain64 sector tweak.
    pub fn self_test_ext2_magic(&self) -> Result<(), DmError> {
        const EXT2_MAGIC_OFFSET: usize = 1024 + 56;
        const EXT2_MAGIC: [u8; 2] = [0x53, 0xef];

        let segment = BioSegment::alloc(1, BioDirection::FromDevice);
        let device: &dyn BlockDevice = self;
        let status = device
            .read_blocks(Bid::new(0), segment.clone())
            .map_err(|_| DmError::Io)?;
        if status != BioStatus::Complete {
            return Err(DmError::Io);
        }
        let mut magic = [0u8; 2];
        segment
            .read_bytes(EXT2_MAGIC_OFFSET, &mut magic)
            .map_err(|_| DmError::Io)?;
        if magic == EXT2_MAGIC {
            Ok(())
        } else {
            Err(DmError::InvalidData)
        }
    }

    fn process_passthrough(&self, bio: SubmittedBio) -> Result<(), BioEnqueueError> {
        self.lower.enqueue(bio)
    }

    fn process_crypt(
        &self,
        bio: SubmittedBio,
        target: &CryptTarget,
    ) -> Result<(), BioEnqueueError> {
        // Complete the upper BIO exactly once, after every transformation and
        // lower-device operation has produced one final status.
        let operation_started = self.performance_stats.start();
        let bytes = bio.segments().iter().fold(0usize, |total, segment| {
            total.saturating_add(segment.nbytes())
        });
        let bio_type = bio.type_();
        if bio_type == BioType::Flush {
            let lower_started = self.performance_stats.start();
            let status = self.lower.sync().unwrap_or(BioStatus::IoError);
            self.performance_stats
                .record_stage(IoStage::LowerWait, lower_started);
            self.performance_stats.record_operation(
                operation_started,
                bytes,
                status != BioStatus::Complete,
            );
            bio.complete(status);
            return Ok(());
        }

        let validation_started = self.performance_stats.start();
        // Keep logical addressing, lower-device addressing, and the plain64 IV
        // in separate domains. Checked arithmetic prevents a wrapped request
        // from reaching a valid lower sector with the wrong XTS tweak.
        let geometry = (|| {
            let logical_first_sector = bio
                .sid_range()
                .start
                .to_raw()
                .checked_add(bio.sid_offset())?;
            let num_sectors = bio.segments().iter().try_fold(0u64, |total, segment| {
                total.checked_add(segment.nsectors().to_raw())
            })?;
            let logical_end_sector = logical_first_sector.checked_add(num_sectors)?;
            if logical_end_sector > target.logical_sectors as u64 {
                return None;
            }
            let lower_first_sector = target
                .data_offset_sectors
                .checked_add(logical_first_sector)?;
            let iv_first_sector = target.iv_offset.checked_add(logical_first_sector)?;
            Some((lower_first_sector, iv_first_sector))
        })();
        self.performance_stats
            .record_stage(IoStage::Validation, validation_started);
        let Some((lower_first_sector, iv_first_sector)) = geometry else {
            self.performance_stats
                .record_operation(operation_started, bytes, true);
            bio.complete(BioStatus::IoError);
            return Ok(());
        };

        let lower_segments = if bio_type == BioType::Write {
            // Writes use bounce segments so plaintext is never submitted to the
            // backing device. Scratch buffers are recycled only after encrypted
            // bytes have been copied into DMA-backed lower segments.
            let mut sector_cursor = iv_first_sector;
            let mut bounce_segments = Vec::with_capacity(bio.segments().len());
            for segment in bio.segments() {
                let scratch_started = self.performance_stats.start();
                let mut ciphertext = target.take_scratch(segment.nbytes());
                self.performance_stats
                    .record_stage(IoStage::Scratch, scratch_started);
                let copy_started = self.performance_stats.start();
                let copied = segment
                    .inner_dma_slice()
                    .read_bytes(0, &mut ciphertext)
                    .is_ok();
                self.performance_stats
                    .record_stage(IoStage::Copy, copy_started);
                let crypto_started = self.performance_stats.start();
                let encrypted = copied
                    && target
                        .cipher
                        .encrypt(sector_cursor, &mut ciphertext)
                        .is_ok();
                self.performance_stats
                    .record_stage(IoStage::Crypto, crypto_started);
                if !encrypted {
                    target.recycle_scratch(ciphertext);
                    self.performance_stats
                        .record_operation(operation_started, bytes, true);
                    bio.complete(BioStatus::IoError);
                    return Ok(());
                }
                let copy_started = self.performance_stats.start();
                let bounce = BioSegment::alloc_bytes(segment.nbytes(), BioDirection::ToDevice);
                let copied = bounce.inner_dma_slice().write_bytes(0, &ciphertext).is_ok();
                self.performance_stats
                    .record_stage(IoStage::Copy, copy_started);
                if !copied {
                    target.recycle_scratch(ciphertext);
                    self.performance_stats
                        .record_operation(operation_started, bytes, true);
                    bio.complete(BioStatus::IoError);
                    return Ok(());
                }
                let scratch_started = self.performance_stats.start();
                target.recycle_scratch(ciphertext);
                self.performance_stats
                    .record_stage(IoStage::Scratch, scratch_started);
                sector_cursor += segment.nsectors().to_raw();
                bounce_segments.push(bounce);
            }
            bounce_segments
        } else {
            bio.segments().to_vec()
        };

        // Acquire read scratch buffers before submission. The lower completion
        // callback may run in interrupt context and must not allocate memory.
        let mut read_scratch = Vec::new();
        if bio_type == BioType::Read {
            read_scratch.reserve(bio.segments().len());
            for segment in bio.segments() {
                let scratch_started = self.performance_stats.start();
                read_scratch.push(target.take_scratch(segment.nbytes()));
                self.performance_stats
                    .record_stage(IoStage::Scratch, scratch_started);
            }
        }

        // Keep the upper BIO alive until the lower completion callback has
        // transformed the data and propagated one final status.
        let upper_slot = Arc::new(Mutex::new(Some(bio)));
        let callback_upper_slot = Arc::clone(&upper_slot);
        let cipher = Arc::clone(&target.cipher);
        let scratch_pool = Arc::clone(&target.scratch_pool);
        let performance_stats = Arc::clone(&self.performance_stats);
        let lower_started = self.performance_stats.start();
        let complete_fn = Box::new(move |lower_status| {
            performance_stats.record_stage(IoStage::LowerWait, lower_started);
            let upper_bio = callback_upper_slot.lock().take();
            let Some(upper_bio) = upper_bio else {
                return;
            };
            let status = if lower_status != BioStatus::Complete {
                lower_status
            } else if bio_type == BioType::Read {
                Self::decrypt_completed_read(
                    &upper_bio,
                    cipher.as_ref(),
                    scratch_pool.as_ref(),
                    performance_stats.as_ref(),
                    iv_first_sector,
                    read_scratch,
                )
            } else {
                BioStatus::Complete
            };
            performance_stats.record_operation(
                operation_started,
                bytes,
                status != BioStatus::Complete,
            );
            upper_bio.complete(status);
        });
        let lower_bio = Bio::new(
            bio_type,
            Sid::new(lower_first_sector),
            lower_segments,
            Some(complete_fn),
        );
        if let Err(error) = lower_bio.submit_without_wait(self.lower.as_ref()) {
            // Submission failed before ownership reached the lower device. Drop
            // our submitted handle and let the caller revert the upper BIO from
            // `Submit` to `Init` while returning the enqueue error.
            let _ = upper_slot.lock().take();
            return Err(error);
        }
        Ok(())
    }

    fn decrypt_completed_read(
        bio: &SubmittedBio,
        cipher: &AesXtsCipher,
        scratch_pool: &Mutex<CryptScratchPool>,
        performance_stats: &DmAtomicPerformanceStats,
        iv_first_sector: u64,
        scratch_buffers: Vec<Vec<u8>>,
    ) -> BioStatus {
        let mut sector_cursor = iv_first_sector;
        let mut status = BioStatus::Complete;
        for (segment, mut plaintext) in bio.segments().iter().zip(scratch_buffers) {
            if status == BioStatus::Complete {
                let copy_started = performance_stats.start();
                let copied_in = segment
                    .inner_dma_slice()
                    .read_bytes(0, &mut plaintext)
                    .is_ok();
                performance_stats.record_stage(IoStage::Copy, copy_started);
                let crypto_started = performance_stats.start();
                let decrypted = copied_in && cipher.decrypt(sector_cursor, &mut plaintext).is_ok();
                performance_stats.record_stage(IoStage::Crypto, crypto_started);
                let copy_started = performance_stats.start();
                let copied_out =
                    decrypted && segment.inner_dma_slice().write_bytes(0, &plaintext).is_ok();
                performance_stats.record_stage(IoStage::Copy, copy_started);
                if !copied_out {
                    status = BioStatus::IoError;
                }
                sector_cursor += segment.nsectors().to_raw();
            }
            let scratch_started = performance_stats.start();
            scratch_pool.lock().recycle(plaintext);
            performance_stats.record_stage(IoStage::Scratch, scratch_started);
        }
        status
    }

    fn process_verity(&self, bio: SubmittedBio, target: &VerityTarget) {
        if bio.type_() == BioType::Write {
            bio.complete(BioStatus::NotSupported);
            return;
        }
        if bio.type_() == BioType::Flush {
            bio.complete(BioStatus::Complete);
            return;
        }

        let operation_started = self.performance_stats.start();
        let bytes = bio.segments().iter().fold(0usize, |total, segment| {
            total.saturating_add(segment.nbytes())
        });
        let validation_started = self.performance_stats.start();
        let first_sector = bio
            .sid_range()
            .start
            .to_raw()
            .checked_add(bio.sid_offset())
            .unwrap_or(u64::MAX);
        self.performance_stats
            .record_stage(IoStage::Validation, validation_started);
        let lower_bio = Bio::new(
            BioType::Read,
            Sid::new(first_sector),
            bio.segments().to_vec(),
            None,
        );
        // Read data once into the caller's segments. `verify_segments` hashes
        // these completed bytes directly for aligned requests, avoiding a
        // second lower-device read of every data block.
        let lower_started = self.performance_stats.start();
        let lower_status = lower_bio
            .submit_and_wait(self.lower.as_ref())
            .unwrap_or(BioStatus::IoError);
        self.performance_stats
            .record_stage(IoStage::LowerWait, lower_started);
        if lower_status != BioStatus::Complete {
            self.performance_stats
                .record_operation(operation_started, bytes, true);
            bio.complete(lower_status);
            return;
        }

        // Authentication must finish before the upper BIO is completed; callers
        // must never observe data whose Merkle path failed verification.
        let status = if target
            .verify_segments(
                self.lower.as_ref(),
                first_sector,
                bio.segments(),
                &self.performance_stats,
            )
            .is_ok()
        {
            BioStatus::Complete
        } else {
            BioStatus::IoError
        };
        self.performance_stats.record_operation(
            operation_started,
            bytes,
            status != BioStatus::Complete,
        );
        bio.complete(status);
    }
}

impl BlockDevice for DmDevice {
    fn enqueue(&self, bio: SubmittedBio) -> Result<(), BioEnqueueError> {
        match &self.target {
            DmTarget::Passthrough => self.process_passthrough(bio),
            DmTarget::Crypt(target) => self.process_crypt(bio, target),
            DmTarget::Verity(target) => {
                self.process_verity(bio, target);
                Ok(())
            }
        }
    }

    fn metadata(&self) -> BlockDeviceMeta {
        match &self.target {
            DmTarget::Verity(target) => BlockDeviceMeta {
                max_nr_segments_per_bio: self.lower.metadata().max_nr_segments_per_bio,
                nr_sectors: target.data_sectors(),
            },
            DmTarget::Crypt(target) => BlockDeviceMeta {
                max_nr_segments_per_bio: self.lower.metadata().max_nr_segments_per_bio,
                nr_sectors: target.logical_sectors,
            },
            _ => self.lower.metadata(),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> DeviceId {
        self.id
    }

    fn supports_partition_scan(&self) -> bool {
        false
    }
}

impl Debug for DmDevice {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DmDevice")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("lower", &self.lower.name())
            .field("target", &self.target)
            .finish()
    }
}

enum DmTarget {
    Passthrough,
    Crypt(CryptTarget),
    Verity(VerityTarget),
}

struct CryptTarget {
    cipher: Arc<AesXtsCipher>,
    iv_offset: u64,
    data_offset_sectors: u64,
    logical_sectors: usize,
    scratch_pool: Arc<Mutex<CryptScratchPool>>,
}

impl CryptTarget {
    fn take_scratch(&self, len: usize) -> Vec<u8> {
        self.scratch_pool.lock().take(len)
    }

    fn recycle_scratch(&self, buffer: Vec<u8>) {
        self.scratch_pool.lock().recycle(buffer);
    }
}

struct CryptScratchPool {
    buffers: Vec<Vec<u8>>,
    capacity: usize,
}

impl CryptScratchPool {
    fn new(capacity: usize) -> Self {
        Self {
            buffers: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn take(&mut self, len: usize) -> Vec<u8> {
        let Some(index) = self
            .buffers
            .iter()
            .position(|buffer| buffer.capacity() >= len)
        else {
            return vec![0u8; len];
        };
        let mut buffer = self.buffers.swap_remove(index);
        buffer.resize(len, 0);
        buffer
    }

    fn recycle(&mut self, mut buffer: Vec<u8>) {
        if self.capacity == 0 || self.buffers.len() == self.capacity {
            return;
        }
        buffer.clear();
        self.buffers.push(buffer);
    }
}

impl Debug for DmTarget {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Passthrough => formatter.write_str("Passthrough"),
            Self::Crypt(_) => formatter.write_str("Crypt(AES-256-XTS)"),
            Self::Verity(_) => formatter.write_str("Verity(SHA-256)"),
        }
    }
}
