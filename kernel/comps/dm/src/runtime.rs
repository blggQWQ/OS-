// SPDX-License-Identifier: MPL-2.0

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use aster_block::BlockDevice;
use device_id::{DeviceId, MajorId, MinorId};
use spin::Mutex;

use crate::{DmDevice, DmError, allocate_mapper_id, decode_hex};

static RUNTIME_MAPPERS: Mutex<BTreeMap<String, RuntimeMapper>> = Mutex::new(BTreeMap::new());

/// Runtime mapper state returned to the Linux-compatible control plane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeMapperStatus {
    pub id: DeviceId,
    pub name: String,
    pub uuid: String,
    pub table_loaded: bool,
    pub active: bool,
    pub sector_start: u64,
    pub logical_sectors: u64,
    pub target_type: Option<String>,
    pub table_parameters: Option<String>,
}

struct RuntimeMapper {
    id: DeviceId,
    uuid: String,
    state: RuntimeMapperState,
}

enum RuntimeMapperState {
    Empty,
    Loaded(CryptTable),
    Active(CryptPublicTable),
}

struct CryptTable {
    lower: Arc<dyn BlockDevice>,
    key: Vec<u8>,
    iv_offset: u64,
    data_offset_sectors: u64,
    logical_sectors: usize,
    public_parameters: String,
}

struct CryptPublicTable {
    logical_sectors: usize,
    public_parameters: String,
}

/// Reserves a runtime mapper name and device ID in suspended state.
pub fn create_runtime_mapper(name: &str, uuid: &str) -> Result<RuntimeMapperStatus, DmError> {
    validate_name(name)?;
    let mut mappers = RUNTIME_MAPPERS.lock();
    if mappers.contains_key(name) {
        return Err(DmError::AlreadyExists);
    }
    let id = allocate_mapper_id()?;
    let mapper = RuntimeMapper {
        id,
        uuid: uuid.to_string(),
        state: RuntimeMapperState::Empty,
    };
    let status = status_for(name, &mapper);
    mappers.insert(name.to_string(), mapper);
    Ok(status)
}

/// Loads one Linux dm-crypt target into the mapper's inactive table slot.
pub fn load_runtime_crypt_table(
    name: &str,
    sector_start: u64,
    logical_sectors: u64,
    parameters: &str,
) -> Result<RuntimeMapperStatus, DmError> {
    if sector_start != 0 {
        return Err(DmError::InvalidTable);
    }
    let logical_sectors = usize::try_from(logical_sectors).map_err(|_| DmError::OutOfRange)?;
    let table = parse_crypt_table(parameters, logical_sectors)?;
    let mut mappers = RUNTIME_MAPPERS.lock();
    let mapper = mappers.get_mut(name).ok_or(DmError::NotFound)?;
    if matches!(mapper.state, RuntimeMapperState::Active(_)) {
        return Err(DmError::Busy);
    }
    mapper.state = RuntimeMapperState::Loaded(table);
    Ok(status_for(name, mapper))
}

/// Atomically promotes the inactive table and registers the block device.
pub fn activate_runtime_mapper(name: &str) -> Result<RuntimeMapperStatus, DmError> {
    let mut mappers = RUNTIME_MAPPERS.lock();
    let mapper = mappers.get_mut(name).ok_or(DmError::NotFound)?;
    if matches!(mapper.state, RuntimeMapperState::Active(_)) {
        return Ok(status_for(name, mapper));
    }
    let RuntimeMapperState::Loaded(table) = &mut mapper.state else {
        return Err(DmError::InvalidTable);
    };
    let device = DmDevice::new_crypt_table(
        mapper.id,
        name.to_string(),
        Arc::clone(&table.lower),
        &table.key,
        table.iv_offset,
        table.data_offset_sectors,
        table.logical_sectors,
    )?;
    aster_block::register(Arc::new(device)).map_err(|_| DmError::AlreadyExists)?;
    table.key.fill(0);
    let public_table = CryptPublicTable {
        logical_sectors: table.logical_sectors,
        public_parameters: table.public_parameters.clone(),
    };
    mapper.state = RuntimeMapperState::Active(public_table);
    Ok(status_for(name, mapper))
}

/// Removes a runtime mapper and wipes any key still held by an inactive table.
pub fn remove_runtime_mapper(name: &str) -> Result<RuntimeMapperStatus, DmError> {
    let mut mapper = RUNTIME_MAPPERS
        .lock()
        .remove(name)
        .ok_or(DmError::NotFound)?;
    let status = status_for(name, &mapper);
    if matches!(mapper.state, RuntimeMapperState::Active(_)) {
        aster_block::unregister(mapper.id).map_err(|_| DmError::Busy)?;
    }
    if let RuntimeMapperState::Loaded(table) = &mut mapper.state {
        table.key.fill(0);
    }
    Ok(status)
}

pub fn runtime_mapper_status(name: &str) -> Result<RuntimeMapperStatus, DmError> {
    let mappers = RUNTIME_MAPPERS.lock();
    let mapper = mappers.get(name).ok_or(DmError::NotFound)?;
    Ok(status_for(name, mapper))
}

pub fn runtime_mapper_list() -> Vec<RuntimeMapperStatus> {
    RUNTIME_MAPPERS
        .lock()
        .iter()
        .map(|(name, mapper)| status_for(name, mapper))
        .collect()
}

fn status_for(name: &str, mapper: &RuntimeMapper) -> RuntimeMapperStatus {
    let (logical_sectors, table_parameters) = match &mapper.state {
        RuntimeMapperState::Loaded(table) => (
            table.logical_sectors as u64,
            Some(table.public_parameters.clone()),
        ),
        RuntimeMapperState::Active(table) => (
            table.logical_sectors as u64,
            Some(table.public_parameters.clone()),
        ),
        RuntimeMapperState::Empty => (0, None),
    };
    RuntimeMapperStatus {
        id: mapper.id,
        name: name.to_string(),
        uuid: mapper.uuid.clone(),
        table_loaded: !matches!(mapper.state, RuntimeMapperState::Empty),
        active: matches!(mapper.state, RuntimeMapperState::Active(_)),
        sector_start: 0,
        logical_sectors,
        target_type: table_parameters.as_ref().map(|_| "crypt".to_string()),
        table_parameters,
    }
}

fn parse_crypt_table(parameters: &str, logical_sectors: usize) -> Result<CryptTable, DmError> {
    let fields: Vec<&str> = parameters.split_whitespace().collect();
    if fields.len() < 5 || fields[0] != "aes-xts-plain64" {
        return Err(DmError::UnsupportedTarget);
    }
    let iv_offset = parse_u64(fields[2])?;
    let lower = find_lower_device(fields[3])?;
    let data_offset_sectors = parse_u64(fields[4])?;
    validate_options(&fields[5..])?;
    let mut key = decode_hex(fields[1])?;
    if key.len() != 64 {
        key.fill(0);
        return Err(DmError::InvalidKey);
    }
    Ok(CryptTable {
        public_parameters: alloc::format!(
            "{} - {} {}:{} {}",
            fields[0],
            iv_offset,
            lower.id().major().get(),
            lower.id().minor().get(),
            data_offset_sectors
        ),
        lower,
        key,
        iv_offset,
        data_offset_sectors,
        logical_sectors,
    })
}

fn find_lower_device(specifier: &str) -> Result<Arc<dyn BlockDevice>, DmError> {
    if let Some((major, minor)) = specifier.split_once(':') {
        let major = major.parse::<u16>().map_err(|_| DmError::InvalidTable)?;
        let minor = minor.parse::<u32>().map_err(|_| DmError::InvalidTable)?;
        let major = MajorId::try_from(major).map_err(|_| DmError::OutOfRange)?;
        let minor = MinorId::try_from(minor).map_err(|_| DmError::OutOfRange)?;
        return aster_block::lookup(DeviceId::new(major, minor)).ok_or(DmError::NotFound);
    }
    aster_block::collect_all()
        .into_iter()
        .find(|device| device.name() == specifier)
        .ok_or(DmError::NotFound)
}

fn validate_options(fields: &[&str]) -> Result<(), DmError> {
    if fields.is_empty() {
        return Ok(());
    }
    let count = fields[0]
        .parse::<usize>()
        .map_err(|_| DmError::InvalidTable)?;
    if fields.len() != count + 1 {
        return Err(DmError::InvalidTable);
    }
    for option in &fields[1..] {
        // Accept only options whose semantics are implemented by this target.
        // Silently accepting workqueue, discard, or large-sector options would
        // misrepresent the behavior exposed to cryptsetup.
        if *option != "sector_size:512" {
            return Err(DmError::InvalidTable);
        }
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), DmError> {
    if name.is_empty() || name.len() >= 128 || name.contains('/') {
        return Err(DmError::InvalidTable);
    }
    Ok(())
}

fn parse_u64(value: &str) -> Result<u64, DmError> {
    value.parse().map_err(|_| DmError::InvalidTable)
}
