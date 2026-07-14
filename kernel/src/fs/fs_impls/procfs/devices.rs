// SPDX-License-Identifier: MPL-2.0

//! Linux-compatible `/proc/devices` support.

use aster_util::printer::VmPrinter;

use crate::{
    fs::{
        file::mkmod,
        procfs::template::{ProcFile, ProcFileOps},
        vfs::inode::Inode,
    },
    prelude::*,
};

/// Represents the inode at `/proc/devices`.
pub struct DevicesFileOps;

impl DevicesFileOps {
    pub fn new_inode(parent: Weak<dyn Inode>) -> Arc<dyn Inode> {
        ProcFile::new(Self, parent, mkmod!(a+r))
    }
}

impl ProcFileOps for DevicesFileOps {
    fn read_at(&self, offset: usize, writer: &mut VmWriter) -> Result<usize> {
        let mut printer = VmPrinter::new_skip(writer, offset);

        // Keep the Linux section layout because libdevmapper parses this file
        // to discover the dynamically allocated device-mapper block major.
        writeln!(printer, "Character devices:")?;
        writeln!(printer, "  5 tty")?;
        writeln!(printer, " 10 misc")?;
        writeln!(printer)?;
        writeln!(printer, "Block devices:")?;
        if let Some(major) = aster_dm::device_mapper_major() {
            writeln!(printer, "{:>3} device-mapper", major)?;
        }

        Ok(printer.bytes_written())
    }
}
