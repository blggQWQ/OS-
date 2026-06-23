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

use crate::{DmError, crypto::AesXtsCipher, verity::VerityTarget};

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
        Ok(Self {
            id,
            name,
            lower,
            target: DmTarget::Crypt(AesXtsCipher::new(key)?),
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
        Ok(Self {
            id,
            name,
            lower,
            target: DmTarget::Verity(VerityTarget::new(
                data_blocks,
                hash_start_block,
                root_hash,
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

    fn process_crypt(&self, bio: SubmittedBio, cipher: &AesXtsCipher) {
        let bio_type = bio.type_();
        if bio_type == BioType::Flush {
            let status = self.lower.sync().unwrap_or(BioStatus::IoError);
            bio.complete(status);
            return;
        }

        let first_sector = bio
            .sid_range()
            .start
            .to_raw()
            .checked_add(bio.sid_offset())
            .unwrap_or(u64::MAX);
        let mut sector_cursor = first_sector;
        let mut plaintext_segments = Vec::with_capacity(bio.segments().len());

        if bio_type == BioType::Write {
            for segment in bio.segments() {
                let mut plaintext = vec![0u8; segment.nbytes()];
                if segment
                    .inner_dma_slice()
                    .read_bytes(0, &mut plaintext)
                    .is_err()
                {
                    bio.complete(BioStatus::IoError);
                    return;
                }
                let mut ciphertext = plaintext.clone();
                if cipher.encrypt(sector_cursor, &mut ciphertext).is_err()
                    || segment.inner_dma_slice().write_bytes(0, &ciphertext).is_err()
                {
                    bio.complete(BioStatus::IoError);
                    return;
                }
                sector_cursor += segment.nsectors().to_raw();
                plaintext_segments.push(plaintext);
            }
        }

        let lower_bio = Bio::new(
            bio_type,
            Sid::from_offset(first_sector as usize * aster_block::SECTOR_SIZE),
            bio.segments().to_vec(),
            None,
        );
        let lower_status = lower_bio
            .submit_and_wait(self.lower.as_ref())
            .unwrap_or(BioStatus::IoError);
        if lower_status != BioStatus::Complete {
            restore_plaintext(&bio, &plaintext_segments);
            bio.complete(lower_status);
            return;
        }

        sector_cursor = first_sector;
        let status = match bio_type {
            BioType::Read => {
                let mut status = BioStatus::Complete;
                for segment in bio.segments() {
                    let mut plaintext = vec![0u8; segment.nbytes()];
                    if segment.inner_dma_slice().read_bytes(0, &mut plaintext).is_err()
                        || cipher.decrypt(sector_cursor, &mut plaintext).is_err()
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
            BioType::Write => {
                restore_plaintext(&bio, &plaintext_segments);
                BioStatus::Complete
            }
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

        let num_sectors: u64 = bio
            .segments()
            .iter()
            .map(|segment| segment.nsectors().to_raw())
            .sum();
        let status = if target
            .verify_range(self.lower.as_ref(), first_sector, num_sectors)
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
            DmTarget::Crypt(cipher) => {
                self.process_crypt(bio, cipher);
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
    Crypt(AesXtsCipher),
    Verity(VerityTarget),
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

fn restore_plaintext(bio: &SubmittedBio, plaintext_segments: &[Vec<u8>]) {
    for (segment, plaintext) in bio.segments().iter().zip(plaintext_segments) {
        let _ = segment.inner_dma_slice().write_bytes(0, plaintext);
    }
}
