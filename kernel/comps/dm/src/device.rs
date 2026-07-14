// SPDX-License-Identifier: MPL-2.0

use alloc::{string::String, sync::Arc, vec, vec::Vec};
use core::fmt::Debug;

use aster_block::{
    BlockDevice, BlockDeviceMeta,
    bio::{Bio, BioDirection, BioEnqueueError, BioSegment, BioStatus, BioType, SubmittedBio},
    id::{Bid, Sid},
};
use device_id::DeviceId;
use ostd::mm::VmIo;

use crate::{
    DmError,
    crypto::AesXtsCipher,
    verity::{VerityStats, VerityTarget},
};

/// A block-device mapper backed by another Asterinas block device.
pub struct DmDevice {
    id: DeviceId,
    name: String,
    lower: Arc<dyn BlockDevice>,
    target: DmTarget,
}

impl DmDevice {
    /// Creates a passthrough mapper.
    pub fn new_passthrough(id: DeviceId, name: String, lower: Arc<dyn BlockDevice>) -> Self {
        Self {
            id,
            name,
            lower,
            target: DmTarget::Passthrough,
        }
    }

    /// Creates an AES-256-XTS mapper.
    pub fn new_crypt(
        id: DeviceId,
        name: String,
        lower: Arc<dyn BlockDevice>,
        key: &[u8],
    ) -> Result<Self, DmError> {
        let logical_sectors = lower.metadata().nr_sectors;
        Self::new_crypt_table(id, name, lower, key, 0, 0, logical_sectors)
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
                cipher: AesXtsCipher::new(key)?,
                iv_offset,
                data_offset_sectors,
                logical_sectors,
            }),
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

    fn process_crypt(&self, bio: SubmittedBio, target: &CryptTarget) {
        let bio_type = bio.type_();
        if bio_type == BioType::Flush {
            let status = self.lower.sync().unwrap_or(BioStatus::IoError);
            bio.complete(status);
            return;
        }

        let logical_first_sector = bio
            .sid_range()
            .start
            .to_raw()
            .checked_add(bio.sid_offset())
            .unwrap_or(u64::MAX);
        let num_sectors = bio.segments().iter().try_fold(0u64, |total, segment| {
            total.checked_add(segment.nsectors().to_raw())
        });
        let Some(logical_end_sector) =
            num_sectors.and_then(|count| logical_first_sector.checked_add(count))
        else {
            bio.complete(BioStatus::IoError);
            return;
        };
        if logical_end_sector > target.logical_sectors as u64 {
            bio.complete(BioStatus::IoError);
            return;
        }
        let Some(lower_first_sector) = target.data_offset_sectors.checked_add(logical_first_sector)
        else {
            bio.complete(BioStatus::IoError);
            return;
        };
        let Some(iv_first_sector) = target.iv_offset.checked_add(logical_first_sector) else {
            bio.complete(BioStatus::IoError);
            return;
        };

        let lower_segments = if bio_type == BioType::Write {
            let mut sector_cursor = iv_first_sector;
            let mut bounce_segments = Vec::with_capacity(bio.segments().len());
            for segment in bio.segments() {
                let mut ciphertext = vec![0u8; segment.nbytes()];
                if segment
                    .inner_dma_slice()
                    .read_bytes(0, &mut ciphertext)
                    .is_err()
                    || target
                        .cipher
                        .encrypt(sector_cursor, &mut ciphertext)
                        .is_err()
                {
                    bio.complete(BioStatus::IoError);
                    return;
                }
                let bounce = BioSegment::alloc_bytes(segment.nbytes(), BioDirection::ToDevice);
                if bounce
                    .inner_dma_slice()
                    .write_bytes(0, &ciphertext)
                    .is_err()
                {
                    bio.complete(BioStatus::IoError);
                    return;
                }
                sector_cursor += segment.nsectors().to_raw();
                bounce_segments.push(bounce);
            }
            bounce_segments
        } else {
            bio.segments().to_vec()
        };

        let lower_bio = Bio::new(bio_type, Sid::new(lower_first_sector), lower_segments, None);
        let lower_status = lower_bio
            .submit_and_wait(self.lower.as_ref())
            .unwrap_or(BioStatus::IoError);
        if lower_status != BioStatus::Complete {
            bio.complete(lower_status);
            return;
        }

        let status = match bio_type {
            BioType::Read => {
                let mut sector_cursor = iv_first_sector;
                let mut status = BioStatus::Complete;
                for segment in bio.segments() {
                    let mut plaintext = vec![0u8; segment.nbytes()];
                    if segment
                        .inner_dma_slice()
                        .read_bytes(0, &mut plaintext)
                        .is_err()
                        || target
                            .cipher
                            .decrypt(sector_cursor, &mut plaintext)
                            .is_err()
                        || segment
                            .inner_dma_slice()
                            .write_bytes(0, &plaintext)
                            .is_err()
                    {
                        status = BioStatus::IoError;
                        break;
                    }
                    sector_cursor += segment.nsectors().to_raw();
                }
                status
            }
            BioType::Write => BioStatus::Complete,
            BioType::Flush => unreachable!(),
        };
        bio.complete(status);
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

        let first_sector = bio
            .sid_range()
            .start
            .to_raw()
            .checked_add(bio.sid_offset())
            .unwrap_or(u64::MAX);
        let lower_bio = Bio::new(
            BioType::Read,
            Sid::new(first_sector),
            bio.segments().to_vec(),
            None,
        );
        let lower_status = lower_bio
            .submit_and_wait(self.lower.as_ref())
            .unwrap_or(BioStatus::IoError);
        if lower_status != BioStatus::Complete {
            bio.complete(lower_status);
            return;
        }

        let status = if target
            .verify_segments(self.lower.as_ref(), first_sector, bio.segments())
            .is_ok()
        {
            BioStatus::Complete
        } else {
            BioStatus::IoError
        };
        bio.complete(status);
    }
}

impl BlockDevice for DmDevice {
    fn enqueue(&self, bio: SubmittedBio) -> Result<(), BioEnqueueError> {
        match &self.target {
            DmTarget::Passthrough => self.process_passthrough(bio),
            DmTarget::Crypt(target) => {
                self.process_crypt(bio, target);
                Ok(())
            }
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
    cipher: AesXtsCipher,
    iv_offset: u64,
    data_offset_sectors: u64,
    logical_sectors: usize,
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
