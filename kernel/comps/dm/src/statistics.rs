// SPDX-License-Identifier: MPL-2.0

//! Low-overhead, opt-in attribution for Device Mapper I/O stages.

use core::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};

use aster_time::read_monotonic_time;

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Enables or disables stage attribution for subsequently executed I/O.
pub(crate) fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Identifies one measured section of a mapper I/O.
#[derive(Clone, Copy)]
pub(crate) enum IoStage {
    Validation,
    Scratch,
    Copy,
    Crypto,
    LowerWait,
    DataSha256,
    CacheLookup,
    HashWait,
    HashSha256,
    DigestCompare,
    CacheInsert,
}

/// Snapshot returned through procfs after a benchmark run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DmPerformanceStats {
    pub enabled: bool,
    pub operations: u64,
    pub bytes: u64,
    pub failures: u64,
    pub total_ns: u64,
    pub max_total_ns: u64,
    pub validation_ns: u64,
    pub scratch_ns: u64,
    pub copy_ns: u64,
    pub crypto_ns: u64,
    pub lower_wait_ns: u64,
    pub data_sha256_ns: u64,
    pub cache_lookup_ns: u64,
    pub hash_wait_ns: u64,
    pub hash_sha256_ns: u64,
    pub digest_compare_ns: u64,
    pub cache_insert_ns: u64,
    pub tree_levels: u64,
}

#[derive(Debug, Default)]
pub(crate) struct DmAtomicPerformanceStats {
    operations: AtomicU64,
    bytes: AtomicU64,
    failures: AtomicU64,
    total_ns: AtomicU64,
    max_total_ns: AtomicU64,
    validation_ns: AtomicU64,
    scratch_ns: AtomicU64,
    copy_ns: AtomicU64,
    crypto_ns: AtomicU64,
    lower_wait_ns: AtomicU64,
    data_sha256_ns: AtomicU64,
    cache_lookup_ns: AtomicU64,
    hash_wait_ns: AtomicU64,
    hash_sha256_ns: AtomicU64,
    digest_compare_ns: AtomicU64,
    cache_insert_ns: AtomicU64,
    tree_levels: AtomicU64,
}

impl DmAtomicPerformanceStats {
    pub(crate) fn start(&self) -> Option<Duration> {
        enabled().then(read_monotonic_time)
    }

    pub(crate) fn record_stage(&self, stage: IoStage, started: Option<Duration>) {
        let Some(started) = started else {
            return;
        };
        let elapsed = elapsed_ns(started);
        let counter = match stage {
            IoStage::Validation => &self.validation_ns,
            IoStage::Scratch => &self.scratch_ns,
            IoStage::Copy => &self.copy_ns,
            IoStage::Crypto => &self.crypto_ns,
            IoStage::LowerWait => &self.lower_wait_ns,
            IoStage::DataSha256 => &self.data_sha256_ns,
            IoStage::CacheLookup => &self.cache_lookup_ns,
            IoStage::HashWait => &self.hash_wait_ns,
            IoStage::HashSha256 => &self.hash_sha256_ns,
            IoStage::DigestCompare => &self.digest_compare_ns,
            IoStage::CacheInsert => &self.cache_insert_ns,
        };
        counter.fetch_add(elapsed, Ordering::Relaxed);
    }

    pub(crate) fn record_operation(&self, started: Option<Duration>, bytes: usize, failed: bool) {
        if !enabled() {
            return;
        }
        let elapsed = started.map(elapsed_ns).unwrap_or(0);
        self.operations.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(bytes as u64, Ordering::Relaxed);
        self.total_ns.fetch_add(elapsed, Ordering::Relaxed);
        self.max_total_ns.fetch_max(elapsed, Ordering::Relaxed);
        if failed {
            self.failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_tree_level(&self) {
        if enabled() {
            self.tree_levels.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn snapshot(&self) -> DmPerformanceStats {
        DmPerformanceStats {
            enabled: enabled(),
            operations: self.operations.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
            total_ns: self.total_ns.load(Ordering::Relaxed),
            max_total_ns: self.max_total_ns.load(Ordering::Relaxed),
            validation_ns: self.validation_ns.load(Ordering::Relaxed),
            scratch_ns: self.scratch_ns.load(Ordering::Relaxed),
            copy_ns: self.copy_ns.load(Ordering::Relaxed),
            crypto_ns: self.crypto_ns.load(Ordering::Relaxed),
            lower_wait_ns: self.lower_wait_ns.load(Ordering::Relaxed),
            data_sha256_ns: self.data_sha256_ns.load(Ordering::Relaxed),
            cache_lookup_ns: self.cache_lookup_ns.load(Ordering::Relaxed),
            hash_wait_ns: self.hash_wait_ns.load(Ordering::Relaxed),
            hash_sha256_ns: self.hash_sha256_ns.load(Ordering::Relaxed),
            digest_compare_ns: self.digest_compare_ns.load(Ordering::Relaxed),
            cache_insert_ns: self.cache_insert_ns.load(Ordering::Relaxed),
            tree_levels: self.tree_levels.load(Ordering::Relaxed),
        }
    }
}

fn elapsed_ns(started: Duration) -> u64 {
    let elapsed = read_monotonic_time().saturating_sub(started).as_nanos();
    u64::try_from(elapsed).unwrap_or(u64::MAX)
}
