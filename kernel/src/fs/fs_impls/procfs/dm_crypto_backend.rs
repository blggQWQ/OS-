// SPDX-License-Identifier: MPL-2.0

//! Selected dm-crypt data-plane backend.

use aster_util::printer::VmPrinter;

use crate::{
    fs::{
        file::mkmod,
        procfs::template::{ProcFile, ProcFileOps},
        vfs::inode::Inode,
    },
    prelude::*,
};

/// Represents `/proc/dm-crypto-backend`.
pub struct DmCryptoBackendFileOps;

impl DmCryptoBackendFileOps {
    pub fn new_inode(parent: Weak<dyn Inode>) -> Arc<dyn Inode> {
        ProcFile::new(Self, parent, mkmod!(a+r))
    }
}

impl ProcFileOps for DmCryptoBackendFileOps {
    fn read_at(&self, offset: usize, writer: &mut VmWriter) -> Result<usize> {
        let mut printer = VmPrinter::new_skip(writer, offset);
        writeln!(printer, "{}", aster_dm::crypto_backend_name())?;
        Ok(printer.bytes_written())
    }
}
