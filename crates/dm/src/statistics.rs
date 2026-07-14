use crate::block_device::BlockDevice;
use crate::error::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// A stable snapshot of one mapper boundary's I/O behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IoStatsSnapshot {
    pub read_operations: u64,
    pub write_operations: u64,
    pub read_bytes: u64,
    pub written_bytes: u64,
    pub failed_operations: u64,
    pub read_nanoseconds: u64,
    pub write_nanoseconds: u64,
    pub max_read_nanoseconds: u64,
    pub max_write_nanoseconds: u64,
}

impl IoStatsSnapshot {
    pub fn average_read_nanoseconds(&self) -> u64 {
        self.read_nanoseconds
            .checked_div(self.read_operations)
            .unwrap_or(0)
    }

    pub fn average_write_nanoseconds(&self) -> u64 {
        self.write_nanoseconds
            .checked_div(self.write_operations)
            .unwrap_or(0)
    }
}

/// Collects mapper-local I/O counts and latency without changing the target.
///
/// Place instances above and below a target stack to attribute transformation
/// overhead separately from the backing device's latency.
#[derive(Clone, Debug)]
pub struct InstrumentedDevice<D> {
    lower: D,
    stats: Arc<IoStats>,
}

impl<D> InstrumentedDevice<D> {
    pub fn new(lower: D) -> Self {
        Self {
            lower,
            stats: Arc::new(IoStats::default()),
        }
    }

    pub fn lower(&self) -> &D {
        &self.lower
    }

    pub fn snapshot(&self) -> IoStatsSnapshot {
        self.stats.snapshot()
    }

    pub fn reset(&self) {
        self.stats.reset();
    }
}

impl<D: BlockDevice> BlockDevice for InstrumentedDevice<D> {
    fn block_size(&self) -> usize {
        self.lower.block_size()
    }

    fn num_blocks(&self) -> u64 {
        self.lower.num_blocks()
    }

    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()> {
        let start = Instant::now();
        let result = self.lower.read_block(block_id, buf);
        self.stats.record_read(
            buf.len() as u64,
            elapsed_nanoseconds(start),
            result.is_err(),
        );
        result
    }

    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()> {
        let start = Instant::now();
        let result = self.lower.write_block(block_id, buf);
        self.stats.record_write(
            buf.len() as u64,
            elapsed_nanoseconds(start),
            result.is_err(),
        );
        result
    }
}

#[derive(Debug, Default)]
struct IoStats {
    read_operations: AtomicU64,
    write_operations: AtomicU64,
    read_bytes: AtomicU64,
    written_bytes: AtomicU64,
    failed_operations: AtomicU64,
    read_nanoseconds: AtomicU64,
    write_nanoseconds: AtomicU64,
    max_read_nanoseconds: AtomicU64,
    max_write_nanoseconds: AtomicU64,
}

impl IoStats {
    fn record_read(&self, bytes: u64, nanoseconds: u64, failed: bool) {
        self.read_operations.fetch_add(1, Ordering::Relaxed);
        self.read_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.read_nanoseconds
            .fetch_add(nanoseconds, Ordering::Relaxed);
        self.max_read_nanoseconds
            .fetch_max(nanoseconds, Ordering::Relaxed);
        if failed {
            self.failed_operations.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn record_write(&self, bytes: u64, nanoseconds: u64, failed: bool) {
        self.write_operations.fetch_add(1, Ordering::Relaxed);
        self.written_bytes.fetch_add(bytes, Ordering::Relaxed);
        self.write_nanoseconds
            .fetch_add(nanoseconds, Ordering::Relaxed);
        self.max_write_nanoseconds
            .fetch_max(nanoseconds, Ordering::Relaxed);
        if failed {
            self.failed_operations.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn snapshot(&self) -> IoStatsSnapshot {
        IoStatsSnapshot {
            read_operations: self.read_operations.load(Ordering::Relaxed),
            write_operations: self.write_operations.load(Ordering::Relaxed),
            read_bytes: self.read_bytes.load(Ordering::Relaxed),
            written_bytes: self.written_bytes.load(Ordering::Relaxed),
            failed_operations: self.failed_operations.load(Ordering::Relaxed),
            read_nanoseconds: self.read_nanoseconds.load(Ordering::Relaxed),
            write_nanoseconds: self.write_nanoseconds.load(Ordering::Relaxed),
            max_read_nanoseconds: self.max_read_nanoseconds.load(Ordering::Relaxed),
            max_write_nanoseconds: self.max_write_nanoseconds.load(Ordering::Relaxed),
        }
    }

    fn reset(&self) {
        self.read_operations.store(0, Ordering::Relaxed);
        self.write_operations.store(0, Ordering::Relaxed);
        self.read_bytes.store(0, Ordering::Relaxed);
        self.written_bytes.store(0, Ordering::Relaxed);
        self.failed_operations.store(0, Ordering::Relaxed);
        self.read_nanoseconds.store(0, Ordering::Relaxed);
        self.write_nanoseconds.store(0, Ordering::Relaxed);
        self.max_read_nanoseconds.store(0, Ordering::Relaxed);
        self.max_write_nanoseconds.store(0, Ordering::Relaxed);
    }
}

fn elapsed_nanoseconds(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}
