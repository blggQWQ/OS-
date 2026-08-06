// SPDX-License-Identifier: MPL-2.0

//! Cumulative Device Mapper stage-attribution counters.

use aster_util::printer::VmPrinter;

use crate::{
    fs::{
        file::mkmod,
        procfs::template::{ProcFile, ProcFileOps},
        vfs::inode::Inode,
    },
    prelude::*,
};

/// Represents `/proc/dm-performance-stats`.
pub struct DmPerformanceStatsFileOps;

impl DmPerformanceStatsFileOps {
    pub fn new_inode(parent: Weak<dyn Inode>) -> Arc<dyn Inode> {
        ProcFile::new(Self, parent, mkmod!(a+r))
    }
}

impl ProcFileOps for DmPerformanceStatsFileOps {
    fn read_at(&self, offset: usize, writer: &mut VmWriter) -> Result<usize> {
        let mut printer = VmPrinter::new_skip(writer, offset);
        if let Some((target, stats)) = aster_dm::boot_performance_stats() {
            writeln!(
                printer,
                "target={} enabled={} operations={} bytes={} failures={} total_ns={} max_total_ns={} current_in_flight={} max_in_flight={} validation_ns={} scratch_ns={} copy_ns={} crypto_ns={} lower_wait_ns={} completion_queue_ns={} data_sha256_ns={} cache_lookup_ns={} hash_wait_ns={} hash_sha256_ns={} digest_compare_ns={} cache_insert_ns={} tree_levels={}",
                target,
                u8::from(stats.enabled),
                stats.operations,
                stats.bytes,
                stats.failures,
                stats.total_ns,
                stats.max_total_ns,
                stats.current_in_flight,
                stats.max_in_flight,
                stats.validation_ns,
                stats.scratch_ns,
                stats.copy_ns,
                stats.crypto_ns,
                stats.lower_wait_ns,
                stats.completion_queue_ns,
                stats.data_sha256_ns,
                stats.cache_lookup_ns,
                stats.hash_wait_ns,
                stats.hash_sha256_ns,
                stats.digest_compare_ns,
                stats.cache_insert_ns,
                stats.tree_levels
            )?;
        } else {
            writeln!(printer, "target=none enabled=0")?;
        }
        Ok(printer.bytes_written())
    }
}
