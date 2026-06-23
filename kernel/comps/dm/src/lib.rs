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
mod sha256;
mod verity;

use alloc::{string::String, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicU32, Ordering};

use aster_block::{BlockDevice, MajorIdOwner};
use component::{ComponentInitError, init_component};
use device_id::{DeviceId, MinorId};
pub use device::DmDevice;
use ostd::{info, warn};
use spin::Once;

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
static DM_MAJOR: Once<MajorIdOwner> = Once::new();
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

/// Errors returned while configuring Device Mapper targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DmError {
    IntegrityViolation,
    InvalidHex,
    InvalidKey,
    Io,
    OutOfRange,
    UnalignedIo,
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
        let id = allocate_mapper_id()?;
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
        let id = allocate_mapper_id()?;
        let mapper = DmDevice::new_crypt(id, mapper_name, lower, &key)
            .map_err(|_| ComponentInitError::Unknown)?;
        info!("registering AES-256-XTS device {}", mapper.name());
        aster_block::register(Arc::new(mapper)).map_err(|_| ComponentInitError::Unknown)?;
        ostd::early_println!("[dm] registered AES-256-XTS mapper");
    }

    if VERITY_SCHEME.get().is_some_and(|scheme| scheme == "dm-verity") {
        let device_name = VERITY_DEVICE
            .get()
            .ok_or(ComponentInitError::Unknown)?;
        let root_hash_hex = VERITY_ROOT_HASH
            .get()
            .ok_or(ComponentInitError::Unknown)?;
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
        let id = allocate_mapper_id()?;
        let mapper = DmDevice::new_verity(
            id,
            mapper_name,
            lower,
            data_blocks,
            hash_start_block,
            root_hash,
        )
        .map_err(|_| ComponentInitError::Unknown)?;
        mapper.self_test_first_block().map_err(|error| {
            ostd::early_println!("[dm-test] FAIL dm-verity rejected first data block: {:?}", error);
            ComponentInitError::Unknown
        })?;
        ostd::early_println!("[dm-test] PASS first data block verified through dm-verity");
        info!("registering dm-verity device {}", mapper.name());
        aster_block::register(Arc::new(mapper)).map_err(|_| ComponentInitError::Unknown)?;
        ostd::early_println!("[dm] registered dm-verity mapper");
    }

    ostd::early_println!("[dm] process-stage initialization complete");
    Ok(())
}

fn allocate_mapper_id() -> Result<DeviceId, ComponentInitError> {
    let major = DM_MAJOR.get().ok_or(ComponentInitError::Unknown)?.get();
    let minor = NEXT_DM_MINOR.fetch_add(1, Ordering::Relaxed);
    Ok(DeviceId::new(major, MinorId::new(minor)))
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
        let byte = u8::from_str_radix(&input[index..index + 2], 16)
            .map_err(|_| DmError::InvalidHex)?;
        output.push(byte);
    }
    Ok(output)
}
