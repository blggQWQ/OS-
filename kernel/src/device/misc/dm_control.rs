// SPDX-License-Identifier: MPL-2.0

//! Linux-compatible Device Mapper control device.
//!
//! This implements the ioctl sequence used by libdevmapper/cryptsetup for a
//! single `crypt` target: create, load, activate, query, and remove.

use alloc::vec;

use aster_dm::{
    DmError, RuntimeMapperStatus, activate_runtime_mapper, create_runtime_mapper,
    load_runtime_crypt_table, remove_runtime_mapper, runtime_mapper_list, runtime_mapper_status,
};
use device_id::{DeviceId, MinorId};
use ostd::{mm::VmIo, task::Task};

use crate::{
    context::current_userspace,
    device::{Device, DeviceType, DevtmpfsInodeMeta, add_node, registry::char},
    events::IoEvents,
    fs::{
        file::{PerOpenFileOps, StatusFlags},
        vfs::{inode::FileOps, path::FsPath},
    },
    prelude::*,
    process::signal::{PollHandle, Pollable},
    util::ioctl::{RawIoctl, dispatch_ioctl},
};

const DM_CONTROL_MINOR: u32 = 236;
const DM_IOCTL_SIZE: usize = size_of::<DmIoctl>();
const DM_EXISTS_FLAG: u32 = 1 << 2;
const DM_STATUS_TABLE_FLAG: u32 = 1 << 4;
const DM_ACTIVE_PRESENT_FLAG: u32 = 1 << 5;
const DM_INACTIVE_PRESENT_FLAG: u32 = 1 << 6;
const DM_BUFFER_FULL_FLAG: u32 = 1 << 8;
const DM_UEVENT_GENERATED_FLAG: u32 = 1 << 13;
const DM_SUSPEND_FLAG: u32 = 1 << 1;
const MAX_IOCTL_DATA: usize = 1024 * 1024;

#[derive(Debug)]
struct DmControlDevice {
    id: DeviceId,
}

impl Device for DmControlDevice {
    fn type_(&self) -> DeviceType {
        DeviceType::Char
    }

    fn id(&self) -> DeviceId {
        self.id
    }

    fn devtmpfs_meta(&self) -> Option<DevtmpfsInodeMeta<'_>> {
        Some(DevtmpfsInodeMeta::new("mapper/control"))
    }

    fn open(&self) -> Result<Box<dyn PerOpenFileOps>> {
        Ok(Box::new(DmControlFile))
    }
}

struct DmControlFile;

impl Pollable for DmControlFile {
    fn poll(&self, mask: IoEvents, _poller: Option<&mut PollHandle>) -> IoEvents {
        mask & (IoEvents::IN | IoEvents::OUT)
    }
}

impl FileOps for DmControlFile {
    fn read_at(
        &self,
        _offset: usize,
        _writer: &mut VmWriter,
        _status_flags: StatusFlags,
    ) -> Result<usize> {
        return_errno_with_message!(Errno::EBADF, "device-mapper control is ioctl-only")
    }

    fn write_at(
        &self,
        _offset: usize,
        _reader: &mut VmReader,
        _status_flags: StatusFlags,
    ) -> Result<usize> {
        return_errno_with_message!(Errno::EBADF, "device-mapper control is ioctl-only")
    }
}

impl PerOpenFileOps for DmControlFile {
    fn check_seekable(&self) -> Result<()> {
        return_errno_with_message!(Errno::ESPIPE, "device-mapper control is not seekable")
    }

    fn is_offset_aware(&self) -> bool {
        false
    }

    fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        use ioctl_defs::*;

        dispatch_ioctl!(match raw_ioctl {
            _cmd @ DmVersion => {
                handle_ioctl(raw_ioctl, Command::Version)
            }
            _cmd @ DmListVersions => {
                handle_ioctl(raw_ioctl, Command::ListVersions)
            }
            _cmd @ DmTableStatus => {
                handle_ioctl(raw_ioctl, Command::TableStatus)
            }
            _cmd @ DmDevCreate => {
                handle_ioctl(raw_ioctl, Command::DevCreate)
            }
            _cmd @ DmTableLoad => {
                handle_ioctl(raw_ioctl, Command::TableLoad)
            }
            _cmd @ DmDevSuspend => {
                handle_ioctl(raw_ioctl, Command::DevSuspend)
            }
            _cmd @ DmDevRemove => {
                handle_ioctl(raw_ioctl, Command::DevRemove)
            }
            _cmd @ DmDevStatus => {
                handle_ioctl(raw_ioctl, Command::DevStatus)
            }
            _ => return_errno_with_message!(Errno::ENOTTY, "unknown device-mapper ioctl"),
        })
    }
}

#[derive(Clone, Copy)]
enum Command {
    Version,
    ListVersions,
    TableStatus,
    DevCreate,
    TableLoad,
    DevSuspend,
    DevRemove,
    DevStatus,
}

fn handle_ioctl(raw: RawIoctl, command: Command) -> Result<i32> {
    let mut header = current_userspace!().read_val::<DmIoctl>(raw.arg())?;
    if header.data_size as usize > MAX_IOCTL_DATA {
        return_errno_with_message!(Errno::E2BIG, "device-mapper ioctl buffer is too large");
    }
    let input_flags = header.flags;
    set_version(&mut header);

    match command {
        Command::Version => {}
        Command::ListVersions => write_target_versions(raw.arg(), &mut header)?,
        Command::DevCreate => {
            let name = c_string(&header.name)?;
            let uuid = c_string(&header.uuid)?;
            let status = create_runtime_mapper(&name, &uuid).map_err(dm_error)?;
            fill_status(&mut header, &status, input_flags);
        }
        Command::TableLoad => {
            let name = mapper_name(&header)?;
            let spec_addr = payload_address(raw.arg(), &header, size_of::<DmTargetSpec>())?;
            if header.target_count != 1 {
                return_errno_with_message!(Errno::EINVAL, "exactly one dm target is supported");
            }
            let spec = current_userspace!().read_val::<DmTargetSpec>(spec_addr)?;
            if c_string(&spec.target_type)? != "crypt" {
                return_errno_with_message!(Errno::EOPNOTSUPP, "only the crypt target is supported");
            }
            let parameters = read_target_parameters(spec_addr, raw.arg(), &header, &spec)?;
            let parameters = normalize_lower_device(&parameters)?;
            let lower_spec = parameters.split_whitespace().nth(3).unwrap_or("<missing>");
            let status =
                load_runtime_crypt_table(&name, spec.sector_start, spec.length, &parameters)
                    .map_err(|error| {
                        ostd::early_println!(
                            "[dm-control] table load failed: name={} dev={:#x} lower={} error={:?}",
                            name,
                            header.dev,
                            lower_spec,
                            error
                        );
                        dm_error(error)
                    })?;
            fill_status(&mut header, &status, input_flags | DM_INACTIVE_PRESENT_FLAG);
        }
        Command::DevSuspend => {
            if input_flags & DM_SUSPEND_FLAG != 0 {
                return_errno_with_message!(
                    Errno::EOPNOTSUPP,
                    "suspending an active mapper is unsupported"
                );
            }
            let name = mapper_name(&header)?;
            let status = activate_runtime_mapper(&name).map_err(dm_error)?;
            create_mapper_node(&status);
            fill_status(
                &mut header,
                &status,
                input_flags | DM_ACTIVE_PRESENT_FLAG | DM_UEVENT_GENERATED_FLAG,
            );
            header.target_count = 1;
        }
        Command::TableStatus => {
            let name = mapper_name(&header)?;
            let status = runtime_mapper_status(&name).map_err(dm_error)?;
            fill_status(&mut header, &status, input_flags);
            write_table_status(raw.arg(), &mut header, &status, input_flags)?;
        }
        Command::DevStatus => {
            let name = mapper_name(&header)?;
            let status = runtime_mapper_status(&name).map_err(dm_error)?;
            fill_status(&mut header, &status, input_flags);
        }
        Command::DevRemove => {
            let name = mapper_name(&header)?;
            let status = remove_runtime_mapper(&name).map_err(dm_error)?;
            remove_mapper_node(&name);
            fill_status(&mut header, &status, input_flags | DM_UEVENT_GENERATED_FLAG);
            header.flags &= !(DM_ACTIVE_PRESENT_FLAG | DM_INACTIVE_PRESENT_FLAG);
        }
    }

    current_userspace!().write_val(raw.arg(), &header)?;
    Ok(0)
}

fn write_target_versions(base: usize, header: &mut DmIoctl) -> Result<()> {
    let mut payload = Vec::new();
    // Runtime table loading currently supports only the crypt target.
    append_target_version(&mut payload, [1, 24, 0], b"crypt", true);
    write_payload(base, header, &payload)
}

fn append_target_version(output: &mut Vec<u8>, version: [u32; 3], name: &[u8], is_last: bool) {
    let start = output.len();
    let unaligned = 16 + name.len() + 1;
    let record_len = align_to_8(unaligned);
    let next = if is_last { 0 } else { record_len as u32 };
    output.extend_from_slice(&next.to_ne_bytes());
    for part in version {
        output.extend_from_slice(&part.to_ne_bytes());
    }
    output.extend_from_slice(name);
    output.resize(start + record_len, 0);
}

fn write_table_status(
    base: usize,
    header: &mut DmIoctl,
    status: &RuntimeMapperStatus,
    input_flags: u32,
) -> Result<()> {
    let Some(target_type) = &status.target_type else {
        header.target_count = 0;
        header.data_size = header.data_start.max(DM_IOCTL_SIZE as u32);
        return Ok(());
    };
    let parameters = if input_flags & DM_STATUS_TABLE_FLAG != 0 {
        status.table_parameters.as_deref().unwrap_or("")
    } else {
        ""
    };
    let spec = DmTargetSpec {
        sector_start: status.sector_start,
        length: status.logical_sectors,
        status: 0,
        next: 0,
        target_type: c_array::<16>(target_type)?,
    };
    let mut payload = Vec::with_capacity(size_of::<DmTargetSpec>() + parameters.len() + 1);
    payload.extend_from_slice(spec.as_bytes());
    payload.extend_from_slice(parameters.as_bytes());
    payload.push(0);
    header.target_count = 1;
    write_payload(base, header, &payload)
}

fn write_payload(base: usize, header: &mut DmIoctl, payload: &[u8]) -> Result<()> {
    let start = header.data_start.max(DM_IOCTL_SIZE as u32) as usize;
    let required = start.checked_add(payload.len()).ok_or_else(|| {
        Error::with_message(Errno::EOVERFLOW, "device-mapper payload size overflow")
    })?;
    if required > header.data_size as usize {
        header.flags |= DM_BUFFER_FULL_FLAG;
        header.data_size = required as u32;
        return Ok(());
    }
    current_userspace!().write_bytes(base + start, payload)?;
    header.data_start = start as u32;
    header.data_size = required as u32;
    Ok(())
}

fn read_target_parameters(
    spec_addr: usize,
    base: usize,
    header: &DmIoctl,
    spec: &DmTargetSpec,
) -> Result<String> {
    let start = spec_addr + size_of::<DmTargetSpec>();
    let data_end = base
        .checked_add(header.data_size as usize)
        .ok_or_else(|| Error::with_message(Errno::EOVERFLOW, "dm table end overflow"))?;
    let end = if spec.next == 0 {
        data_end
    } else {
        spec_addr
            .checked_add(spec.next as usize)
            .ok_or_else(|| Error::with_message(Errno::EOVERFLOW, "dm target end overflow"))?
            .min(data_end)
    };
    if end <= start || end - start > MAX_IOCTL_DATA {
        return_errno_with_message!(Errno::EINVAL, "invalid dm target parameter range");
    }
    let mut bytes = vec![0u8; end - start];
    current_userspace!().read_bytes(start, &mut bytes)?;
    c_string(&bytes)
}

fn payload_address(base: usize, header: &DmIoctl, minimum: usize) -> Result<usize> {
    let start = header.data_start as usize;
    let end = start.checked_add(minimum).ok_or_else(|| {
        Error::with_message(Errno::EOVERFLOW, "device-mapper data offset overflow")
    })?;
    if start < DM_IOCTL_SIZE || end > header.data_size as usize {
        return_errno_with_message!(Errno::EINVAL, "invalid device-mapper data range");
    }
    base.checked_add(start)
        .ok_or_else(|| Error::with_message(Errno::EOVERFLOW, "userspace address overflow"))
}

fn fill_status(header: &mut DmIoctl, status: &RuntimeMapperStatus, input_flags: u32) {
    header.dev = status.id.as_encoded_u64();
    header.name = c_array_lossy::<128>(&status.name);
    header.uuid = c_array_lossy::<129>(&status.uuid);
    header.open_count = 0;
    header.event_nr = 0;
    header.flags = input_flags | DM_EXISTS_FLAG;
    if status.active {
        header.flags |= DM_ACTIVE_PRESENT_FLAG;
    } else if status.table_loaded {
        header.flags |= DM_INACTIVE_PRESENT_FLAG;
    }
}

fn set_version(header: &mut DmIoctl) {
    header.version = [4, 48, 0];
    header.flags |= DM_EXISTS_FLAG;
}

fn c_string(bytes: &[u8]) -> Result<String> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec())
        .map_err(|_| Error::with_message(Errno::EINVAL, "device-mapper string is not UTF-8"))
}

fn mapper_name(header: &DmIoctl) -> Result<String> {
    let name = c_string(&header.name)?;
    if !name.is_empty() {
        return Ok(name);
    }
    runtime_mapper_list()
        .into_iter()
        .find(|status| status.id.as_encoded_u64() == header.dev)
        .map(|status| status.name)
        .ok_or_else(|| Error::with_message(Errno::ENXIO, "device-mapper device does not exist"))
}

fn normalize_lower_device(parameters: &str) -> Result<String> {
    let mut fields: Vec<String> = parameters
        .split_whitespace()
        .map(String::from)
        .collect();
    let Some(lower) = fields.get(3) else {
        return_errno_with_message!(Errno::EINVAL, "dm-crypt table is missing a lower device");
    };
    if !lower.starts_with('/') {
        return Ok(parameters.to_string());
    }

    let task = Task::current()
        .ok_or_else(|| Error::with_message(Errno::ESRCH, "no current task"))?;
    let thread_local = task
        .as_thread_local()
        .ok_or_else(|| Error::with_message(Errno::ESRCH, "no current thread state"))?;
    let fs = thread_local.borrow_fs();
    let resolver = fs.resolver().read();
    let path = resolver.lookup(&FsPath::try_from(lower.as_str())?)?;
    let device_id = path.metadata().self_dev_id.ok_or_else(|| {
        Error::with_message(Errno::ENODEV, "dm-crypt lower path is not a device")
    })?;
    fields[3] = alloc::format!(
        "{}:{}",
        device_id.major().get(),
        device_id.minor().get()
    );
    Ok(fields.join(" "))
}

fn c_array<const N: usize>(value: &str) -> Result<[u8; N]> {
    if value.len() >= N {
        return_errno_with_message!(Errno::ENAMETOOLONG, "device-mapper string is too long");
    }
    Ok(c_array_lossy(value))
}

fn c_array_lossy<const N: usize>(value: &str) -> [u8; N] {
    let mut output = [0u8; N];
    let length = value.len().min(N.saturating_sub(1));
    output[..length].copy_from_slice(&value.as_bytes()[..length]);
    output
}

fn align_to_8(value: usize) -> usize {
    (value + 7) & !7
}

fn dm_error(error: DmError) -> Error {
    let errno = match error {
        DmError::AlreadyExists => Errno::EEXIST,
        DmError::Busy => Errno::EBUSY,
        DmError::NotFound => Errno::ENXIO,
        DmError::UnsupportedTarget => Errno::EOPNOTSUPP,
        DmError::OutOfRange => Errno::ERANGE,
        DmError::IntegrityViolation | DmError::Io => Errno::EIO,
        DmError::InvalidHex
        | DmError::InvalidKey
        | DmError::InvalidTable
        | DmError::InvalidData
        | DmError::UnalignedIo => Errno::EINVAL,
    };
    Error::with_message(errno, "device-mapper operation failed")
}

fn create_mapper_node(status: &RuntimeMapperStatus) {
    let Some(task) = Task::current() else {
        return;
    };
    let Some(thread_local) = task.as_thread_local() else {
        return;
    };
    let fs = thread_local.borrow_fs();
    let resolver = fs.resolver().read();
    let path = alloc::format!("mapper/{}", status.name);
    let _ = add_node(
        DeviceType::Block,
        status.id.as_encoded_u64(),
        &DevtmpfsInodeMeta::new(path),
        &resolver,
    );
}

fn remove_mapper_node(name: &str) {
    let Some(task) = Task::current() else {
        return;
    };
    let Some(thread_local) = task.as_thread_local() else {
        return;
    };
    let fs = thread_local.borrow_fs();
    let resolver = fs.resolver().read();
    if let Ok(mapper_dir) = resolver.lookup(&FsPath::try_from("/dev/mapper").unwrap()) {
        let _ = mapper_dir.unlink(name);
    }
}

pub(super) fn init_in_first_kthread() {
    let major = super::MISC_MAJOR.get().unwrap().get();
    let id = DeviceId::new(major, MinorId::new(DM_CONTROL_MINOR));
    char::register(Arc::new(DmControlDevice { id })).unwrap();
}

#[repr(C)]
#[derive(Clone, Copy, Pod)]
struct DmIoctl {
    version: [u32; 3],
    data_size: u32,
    data_start: u32,
    target_count: u32,
    open_count: i32,
    flags: u32,
    event_nr: u32,
    padding: u32,
    dev: u64,
    name: [u8; 128],
    uuid: [u8; 129],
    data: [u8; 7],
}

#[repr(C)]
#[derive(Clone, Copy, Pod)]
struct DmTargetSpec {
    sector_start: u64,
    length: u64,
    status: i32,
    next: u32,
    target_type: [u8; 16],
}

mod ioctl_defs {
    use super::DmIoctl;
    use crate::util::ioctl::{InOutData, ioc};

    pub(super) type DmVersion = ioc!(DM_VERSION, 0xfd, 0, InOutData<DmIoctl>);
    pub(super) type DmDevCreate = ioc!(DM_DEV_CREATE, 0xfd, 3, InOutData<DmIoctl>);
    pub(super) type DmDevRemove = ioc!(DM_DEV_REMOVE, 0xfd, 4, InOutData<DmIoctl>);
    pub(super) type DmDevSuspend = ioc!(DM_DEV_SUSPEND, 0xfd, 6, InOutData<DmIoctl>);
    pub(super) type DmDevStatus = ioc!(DM_DEV_STATUS, 0xfd, 7, InOutData<DmIoctl>);
    pub(super) type DmTableLoad = ioc!(DM_TABLE_LOAD, 0xfd, 9, InOutData<DmIoctl>);
    pub(super) type DmTableStatus = ioc!(DM_TABLE_STATUS, 0xfd, 12, InOutData<DmIoctl>);
    pub(super) type DmListVersions = ioc!(DM_LIST_VERSIONS, 0xfd, 13, InOutData<DmIoctl>);
}
