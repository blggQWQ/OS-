// SPDX-License-Identifier: MPL-2.0

//! Device Mapper targets for Asterinas block devices.

#![no_std]
#![deny(unsafe_code)]

extern crate alloc;

macro_rules! __log_prefix {
    () => {
        "dm: "
    };
}

mod crypto;
mod device;
mod runtime;
mod sha256;
mod verity;

use alloc::{string::String, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use aster_block::{BlockDevice, MajorIdOwner};
use component::{ComponentInitError, init_component};
pub use device::DmDevice;
use device_id::{DeviceId, MinorId};
use ostd::{info, warn};
pub use runtime::{
    RuntimeMapperStatus, activate_runtime_mapper, create_runtime_mapper, load_runtime_crypt_table,
    remove_runtime_mapper, runtime_mapper_list, runtime_mapper_status,
};
use spin::Once;
pub use verity::VerityStats;

static CRYPT_DEVICE: Once<String> = Once::new();
static CRYPT_NAME: Once<String> = Once::new();
static CRYPT_KEY: Once<String> = Once::new();
static PASSTHROUGH_DEVICE: Once<String> = Once::new();
static PASSTHROUGH_NAME: Once<String> = Once::new();
static VERITY_SCHEME: Once<String> = Once::new();
static VERITY_DEVICE: Once<String> = Once::new();
static VERITY_NAME: Once<String> = Once::new();
static VERITY_ROOT_HASH: Once<String> = Once::new();
static VERITY_DATA_BLOCKS: Once<usize> = Once::new();
static VERITY_HASH_START_BLOCK: Once<usize> = Once::new();
static VERITY_CACHE_BLOCKS: Once<usize> = Once::new();
static VERITY_MAPPER: Once<Arc<DmDevice>> = Once::new();
static VERITY_BOOT_STATUS: AtomicU8 = AtomicU8::new(0);
static DM_MAJOR: Once<MajorIdOwner> = Once::new();

/// Returns the Linux-visible block major allocated to device-mapper.
pub fn device_mapper_major() -> Option<u16> {
    DM_MAJOR.get().map(|owner| owner.get().get())
}

/// Returns the dm-crypt data backend selected by this kernel build and CPU.
pub fn crypto_backend_name() -> &'static str {
    if crypto::hardware_aes_available() {
        "AES-NI"
    } else {
        "portable-software"
    }
}

/// Returns a live snapshot of the boot-configured dm-verity mapper.
///
/// The procfs benchmark surface uses this instead of maintaining a second set
/// of counters, so the figures describe the exact mapper serving guest I/O.
pub fn boot_verity_stats() -> Option<(usize, VerityStats)> {
    let mapper = VERITY_MAPPER.get()?;
    let stats = mapper.verity_stats()?;
    Some((VERITY_CACHE_BLOCKS.get().copied().unwrap_or(256), stats))
}

/// Returns the result of the boot-time dm-verity authenticated read.
pub fn boot_verity_status() -> &'static str {
    match VERITY_BOOT_STATUS.load(Ordering::Relaxed) {
        1 => "verified",
        2 => "integrity-rejected",
        3 => "io-error",
        _ => "unavailable",
    }
}
static NEXT_DM_MINOR: AtomicU32 = AtomicU32::new(0);

aster_cmdline::define_kv_param!("dm_crypt.device", CRYPT_DEVICE);
aster_cmdline::define_kv_param!("dm_crypt.name", CRYPT_NAME);
aster_cmdline::define_kv_param!("dm_crypt.key", CRYPT_KEY);
aster_cmdline::define_kv_param!("dm_passthrough.device", PASSTHROUGH_DEVICE);
aster_cmdline::define_kv_param!("dm_passthrough.name", PASSTHROUGH_NAME);
aster_cmdline::define_kv_param!("rootfs_verity.scheme", VERITY_SCHEME);
aster_cmdline::define_kv_param!("rootfs_verity.device", VERITY_DEVICE);
aster_cmdline::define_kv_param!("rootfs_verity.name", VERITY_NAME);
aster_cmdline::define_kv_param!("rootfs_verity.hash", VERITY_ROOT_HASH);
aster_cmdline::define_kv_param!("rootfs_verity.data_blocks", VERITY_DATA_BLOCKS);
aster_cmdline::define_kv_param!("rootfs_verity.hash_start", VERITY_HASH_START_BLOCK);
aster_cmdline::define_kv_param!("rootfs_verity.cache_blocks", VERITY_CACHE_BLOCKS);

/// Errors returned while configuring Device Mapper targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DmError {
    AlreadyExists,
    Busy,
    IntegrityViolation,
    InvalidHex,
    InvalidKey,
    InvalidTable,
    InvalidData,
    Io,
    NotFound,
    OutOfRange,
    UnalignedIo,
    UnsupportedTarget,
}

#[init_component(process)]
fn init() -> Result<(), ComponentInitError> {
    ostd::early_println!("[dm] process-stage initialization started");
    if DM_MAJOR.get().is_none() {
        let major = aster_block::allocate_major().map_err(|_| ComponentInitError::Unknown)?;
        DM_MAJOR.call_once(|| major);
    }

    if let Some(device_name) = PASSTHROUGH_DEVICE.get() {
        let mapper_name = PASSTHROUGH_NAME
            .get()
            .cloned()
            .unwrap_or_else(|| String::from("dm-pass0"));
        let Some(lower) = find_device(device_name) else {
            warn!("cannot find passthrough lower device {device_name}");
            return Err(ComponentInitError::Unknown);
        };
        let id = allocate_mapper_id().map_err(|_| ComponentInitError::Unknown)?;
        let mapper = Arc::new(DmDevice::new_passthrough(id, mapper_name, lower));
        info!("registering passthrough device {}", mapper.name());
        aster_block::register(mapper).map_err(|_| ComponentInitError::Unknown)?;
        ostd::early_println!("[dm] registered passthrough mapper");
    }

    if let Some(device_name) = CRYPT_DEVICE.get() {
        let Some(key_hex) = CRYPT_KEY.get() else {
            warn!("dm_crypt.key is required when dm_crypt.device is set");
            return Err(ComponentInitError::Unknown);
        };
        let key = decode_hex(key_hex).map_err(|_| ComponentInitError::Unknown)?;
        let mapper_name = CRYPT_NAME
            .get()
            .cloned()
            .unwrap_or_else(|| String::from("dm-crypt0"));
        let Some(lower) = find_device(device_name) else {
            warn!("cannot find dm-crypt lower device {device_name}");
            return Err(ComponentInitError::Unknown);
        };
        let id = allocate_mapper_id().map_err(|_| ComponentInitError::Unknown)?;
        ostd::early_println!("[dm] initializing AES-256-XTS mapper");
        let mapper = DmDevice::new_crypt(id, mapper_name, lower, &key)
            .map_err(|_| ComponentInitError::Unknown)?;
        ostd::early_println!(
            "[dm-perf] AES backend={}",
            if crypto::hardware_aes_available() {
                "AES-NI"
            } else {
                "portable-software"
            }
        );
        mapper.self_test_ext2_magic().map_err(|error| {
            ostd::early_println!(
                "[dm-test] FAIL AES-256-XTS did not reveal a valid ext2 superblock: {:?}",
                error
            );
            ComponentInitError::Unknown
        })?;
        ostd::early_println!(
            "[dm-test] PASS AES-256-XTS decrypted a valid ext2 superblock through dm-crypt"
        );
        info!("registering AES-256-XTS device {}", mapper.name());
        aster_block::register(Arc::new(mapper)).map_err(|_| ComponentInitError::Unknown)?;
        ostd::early_println!("[dm] registered AES-256-XTS mapper");
    }

    if VERITY_SCHEME
        .get()
        .is_some_and(|scheme| scheme == "dm-verity")
    {
        let device_name = VERITY_DEVICE.get().ok_or(ComponentInitError::Unknown)?;
        let root_hash_hex = VERITY_ROOT_HASH.get().ok_or(ComponentInitError::Unknown)?;
        let root_hash = decode_hash(root_hash_hex).map_err(|_| ComponentInitError::Unknown)?;
        let data_blocks = *VERITY_DATA_BLOCKS
            .get()
            .ok_or(ComponentInitError::Unknown)?;
        let hash_start_block = *VERITY_HASH_START_BLOCK
            .get()
            .ok_or(ComponentInitError::Unknown)?;
        let mapper_name = VERITY_NAME
            .get()
            .cloned()
            .unwrap_or_else(|| String::from("dm-verity0"));
        let Some(lower) = find_device(device_name) else {
            warn!("cannot find dm-verity lower device {device_name}");
            return Err(ComponentInitError::Unknown);
        };
        let id = allocate_mapper_id().map_err(|_| ComponentInitError::Unknown)?;
        let cache_blocks = VERITY_CACHE_BLOCKS.get().copied().unwrap_or(256);
        let mapper = Arc::new(
            DmDevice::new_verity_with_cache(
                id,
                mapper_name,
                lower,
                data_blocks,
                hash_start_block,
                root_hash,
                cache_blocks,
            )
            .map_err(|_| ComponentInitError::Unknown)?,
        );
        mapper.self_test_first_block().map_err(|error| {
            VERITY_BOOT_STATUS.store(
                if error == DmError::IntegrityViolation {
                    2
                } else {
                    3
                },
                Ordering::Relaxed,
            );
            ostd::early_println!(
                "[dm-test] FAIL dm-verity rejected first data block: {:?}",
                error
            );
            ComponentInitError::Unknown
        })?;
        mapper.self_test_first_block().map_err(|error| {
            VERITY_BOOT_STATUS.store(
                if error == DmError::IntegrityViolation {
                    2
                } else {
                    3
                },
                Ordering::Relaxed,
            );
            ostd::early_println!(
                "[dm-test] FAIL dm-verity rejected repeated data block: {:?}",
                error
            );
            ComponentInitError::Unknown
        })?;
        VERITY_BOOT_STATUS.store(1, Ordering::Relaxed);
        ostd::early_println!("[dm-test] PASS first data block verified through dm-verity");
        if let Some(stats) = mapper.verity_stats() {
            ostd::early_println!(
                "[dm-perf] verity cache_blocks={} verified={} bio_blocks={} data_rereads={} hash_hits={} hash_misses={}",
                cache_blocks,
                stats.verified_data_blocks,
                stats.bio_data_blocks,
                stats.data_reread_blocks,
                stats.hash_cache_hits,
                stats.hash_cache_misses
            );
        }
        info!("registering dm-verity device {}", mapper.name());
        VERITY_MAPPER.call_once(|| Arc::clone(&mapper));
        aster_block::register(mapper).map_err(|_| ComponentInitError::Unknown)?;
        ostd::early_println!("[dm] registered dm-verity mapper");
    }

    ostd::early_println!("[dm] process-stage initialization complete");
    Ok(())
}

fn allocate_mapper_id() -> Result<DeviceId, DmError> {
    let major = DM_MAJOR.get().ok_or(DmError::NotFound)?.get();
    let minor = NEXT_DM_MINOR.fetch_add(1, Ordering::Relaxed);
    let minor = MinorId::try_from(minor).map_err(|_| DmError::OutOfRange)?;
    Ok(DeviceId::new(major, minor))
}

fn find_device(name: &str) -> Option<Arc<dyn BlockDevice>> {
    aster_block::collect_all()
        .into_iter()
        .find(|device| device.name() == name)
}

fn decode_hash(input: &str) -> Result<[u8; 32], DmError> {
    let bytes = decode_hex(input)?;
    if bytes.len() != 32 {
        return Err(DmError::InvalidHex);
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes);
    Ok(hash)
}

fn decode_hex(input: &str) -> Result<Vec<u8>, DmError> {
    if !input.len().is_multiple_of(2) {
        return Err(DmError::InvalidHex);
    }

    let mut output = Vec::with_capacity(input.len() / 2);
    for index in (0..input.len()).step_by(2) {
        let byte =
            u8::from_str_radix(&input[index..index + 2], 16).map_err(|_| DmError::InvalidHex)?;
        output.push(byte);
    }
    Ok(output)
}
