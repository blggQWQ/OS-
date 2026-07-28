// SPDX-License-Identifier: MPL-2.0

//! Live hot-path counters for the boot-configured dm-verity mapper.

use aster_util::printer::VmPrinter;

use crate::{
    fs::{
        file::mkmod,
        procfs::template::{ProcFile, ProcFileOps},
        vfs::inode::Inode,
    },
    prelude::*,
};

/// Represents `/proc/dm-verity-stats`.
pub struct DmVerityStatsFileOps;

impl DmVerityStatsFileOps {
    pub fn new_inode(parent: Weak<dyn Inode>) -> Arc<dyn Inode> {
        ProcFile::new(Self, parent, mkmod!(a+r))
    }
}

impl ProcFileOps for DmVerityStatsFileOps {
    fn read_at(&self, offset: usize, writer: &mut VmWriter) -> Result<usize> {
        let mut printer = VmPrinter::new_skip(writer, offset);
        if let Some((cache_blocks, stats)) = aster_dm::boot_verity_stats() {
            writeln!(
                printer,
                "status=verified cache_blocks={} verified={} bio_blocks={} data_rereads={} hash_hits={} hash_misses={} auth_shortcuts={}",
                cache_blocks,
                stats.verified_data_blocks,
                stats.bio_data_blocks,
                stats.data_reread_blocks,
                stats.hash_cache_hits,
                stats.hash_cache_misses,
                stats.authenticated_cache_shortcuts
            )?;
        } else {
            writeln!(printer, "status={}", aster_dm::boot_verity_status())?;
        }
        Ok(printer.bytes_written())
    }
}
