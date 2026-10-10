//! In-process control-plane model. This does NOT mount an OS device or attest
//! kernel state. Callers supplying configuration and policy are trusted.
//! Every mutation must pass through this registry for its guarantees to hold.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Configuration {
    pub encrypted: bool,
    pub integrity: bool,
    pub read_only: bool,
    pub root_hash: Option<[u8; 32]>,
    pub dependencies: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Policy {
    pub confidentiality: bool,
    pub integrity: bool,
    pub read_only: bool,
    /// Supplied by a trusted policy authority, not learned from the device.
    pub expected_root: Option<[u8; 32]>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtectionError { Missing, Busy, Stale, PolicyMismatch, InvalidGraph, Poisoned, Exhausted }
type Result<T> = std::result::Result<T, ProtectionError>;
#[derive(Clone)]
pub struct Registry { state: Arc<Mutex<State>> }
#[derive(Default)]
struct State { next_id: u64, entries: BTreeMap<String, Entry> }
struct Entry { id: u64, generation: u64, pins: u64, config: Configuration }
#[derive(Clone)]
struct Binding { name: String, id: u64, generation: u64 }
/// Private fields prevent callers constructing a checked object directly.
pub struct Configured { registry: Registry, name: String, id: u64 }
/// Non-Clone, consumed by activation; a receipt is not a reusable path string.
pub struct Checked { registry: Registry, bindings: Vec<Binding> }
/// Logical lease only. Not an operating-system mount.
pub struct Mounted { registry: Registry, bindings: Vec<Binding>, active: bool }

fn collect(state: &State, name: &str, seen: &mut BTreeSet<String>, out: &mut Vec<Binding>) -> Result<()> {
    if !seen.insert(name.to_owned()) { return Err(ProtectionError::InvalidGraph); }
    let entry = state.entries.get(name).ok_or(ProtectionError::Missing)?;
    out.push(Binding { name: name.to_owned(), id: entry.id, generation: entry.generation });
    for dep in &entry.config.dependencies {
        collect(state, dep, seen, out)?;
    }
    seen.remove(name);
    Ok(())
}
fn valid_graph(state: &State, name: &str, config: &Configuration) -> Result<()> {
    let mut seen = BTreeSet::from([name.to_owned()]);
    for dep in &config.dependencies { collect(state, dep, &mut seen, &mut Vec::new())?; }
    Ok(())
}
impl Default for Registry { fn default() -> Self { Self::new() } }
impl Registry {
    pub fn new() -> Self { Self { state: Arc::new(Mutex::new(State::default())) } }
    pub fn configure(&self, name: &str, config: Configuration) -> Result<Configured> {
        let mut state = self.state.lock().map_err(|_| ProtectionError::Poisoned)?;
        valid_graph(&state, name, &config)?;
        let id = if let Some(entry) = state.entries.get_mut(name) {
            if entry.pins != 0 { return Err(ProtectionError::Busy); }
            entry.generation = entry.generation.checked_add(1).ok_or(ProtectionError::Exhausted)?;
            entry.config = config; entry.id
        } else {
            state.next_id = state.next_id.checked_add(1).ok_or(ProtectionError::Exhausted)?;
            let id = state.next_id;
            state.entries.insert(name.to_owned(), Entry { id, generation: 1, pins: 0, config }); id
        };
        Ok(Configured { registry: self.clone(), name: name.to_owned(), id })
    }
    pub fn remove(&self, name: &str) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| ProtectionError::Poisoned)?;
        let entry = state.entries.get(name).ok_or(ProtectionError::Missing)?;
        if entry.pins != 0 { return Err(ProtectionError::Busy); }
        state.entries.remove(name); Ok(())
    }
    pub fn pin_count(&self, name: &str) -> Result<u64> {
        Ok(self.state.lock().map_err(|_| ProtectionError::Poisoned)?.entries.get(name).ok_or(ProtectionError::Missing)?.pins)
    }
}
impl Configured {
    /// Consume the configured handle and bind the entire dependency closure.
    pub fn verify(self, policy: &Policy) -> Result<Checked> {
        let state = self.registry.state.lock().map_err(|_| ProtectionError::Poisoned)?;
        let e = state.entries.get(&self.name).ok_or(ProtectionError::Missing)?;
        if e.id != self.id { return Err(ProtectionError::Stale); }
        let c = &e.config;
        if (policy.confidentiality && !c.encrypted) || (policy.integrity && !c.integrity)
            || (policy.read_only && !c.read_only)
            || (c.integrity && (!c.read_only || policy.expected_root.is_none() || c.root_hash != policy.expected_root)) {
            return Err(ProtectionError::PolicyMismatch);
        }
        let mut bindings = Vec::new(); collect(&state, &self.name, &mut BTreeSet::new(), &mut bindings)?;
        // Shared lower devices in a DAG are pinned exactly once per lease.
        bindings.sort_by_key(|b| b.id); bindings.dedup_by_key(|b| b.id);
        drop(state); Ok(Checked { registry: self.registry, bindings })
    }
}
impl Checked {
    /// Check identities/versions and pin the graph within the SAME lock.
    /// A standalone recheck followed by a separate activation would still race.
    pub fn activate(self) -> Result<Mounted> {
        let mut state = self.registry.state.lock().map_err(|_| ProtectionError::Poisoned)?;
        for b in &self.bindings {
            let e = state.entries.get(&b.name).ok_or(ProtectionError::Stale)?;
            if e.id != b.id || e.generation != b.generation { return Err(ProtectionError::Stale); }
            if e.pins == u64::MAX { return Err(ProtectionError::Exhausted); }
        }
        for b in &self.bindings { state.entries.get_mut(&b.name).unwrap().pins += 1; }
        drop(state); Ok(Mounted { registry: self.registry, bindings: self.bindings, active: true })
    }
}
impl Mounted {
    pub fn close(mut self) { self.release(); }
    fn release(&mut self) {
        if !self.active { return; }
        // Poisoning never grants access. Drop can recover the protected state
        // solely to release references; normal operations remain fail-closed.
        let mut state = self.registry.state.lock().unwrap_or_else(|e| e.into_inner());
        for b in &self.bindings {
            if let Some(e) = state.entries.get_mut(&b.name) { if e.id == b.id { e.pins -= 1; } }
        }
        self.active = false;
    }
}
impl Drop for Mounted { fn drop(&mut self) { self.release(); } }

#[cfg(test)]
mod tests {
    use super::*;
    fn raw() -> Configuration { Configuration { encrypted: false, integrity: false, read_only: false, root_hash: None, dependencies: vec![] } }
    fn protected(dep: &str) -> Configuration { Configuration { encrypted: true, integrity: true, read_only: true, root_hash: Some([7;32]), dependencies: vec![dep.into()] } }
    fn policy() -> Policy { Policy { confidentiality:true, integrity:true, read_only:true, expected_root:Some([7;32]) } }
    fn prepared() -> (Registry, Checked) { let r=Registry::new();r.configure("lower",raw()).unwrap();let c=r.configure("top",protected("lower")).unwrap().verify(&policy()).unwrap();(r,c) }
    #[test] fn lower_change_invalidates_receipt() { let (r,c)=prepared();r.configure("lower",raw()).unwrap();assert!(matches!(c.activate(),Err(ProtectionError::Stale)));assert_eq!(r.pin_count("top"),Ok(0)); }
    #[test] fn same_name_recreation_invalidates_receipt() { let (r,c)=prepared();r.remove("lower").unwrap();r.configure("lower",raw()).unwrap();assert!(matches!(c.activate(),Err(ProtectionError::Stale))); }
    #[test] fn live_lease_blocks_mutation_and_removal() { let (r,c)=prepared();let m=c.activate().unwrap();assert!(matches!(r.configure("lower",raw()),Err(ProtectionError::Busy)));assert_eq!(r.remove("top"),Err(ProtectionError::Busy));m.close();assert_eq!(r.pin_count("lower"),Ok(0));assert!(r.configure("lower",raw()).is_ok()); }
    #[test] fn unrelated_device_is_not_frozen() { let (r,c)=prepared();let _m=c.activate().unwrap();assert!(r.configure("other",raw()).is_ok());assert!(r.configure("other",raw()).is_ok()); }
    #[test] fn two_leases_release_only_own_pins() { let (r,c)=prepared();let m=c.activate().unwrap();let n=r.configure("other",protected("lower")).unwrap().verify(&policy()).unwrap().activate().unwrap();assert_eq!(r.pin_count("lower"),Ok(2));drop(m);assert_eq!(r.pin_count("lower"),Ok(1));assert!(matches!(r.configure("lower",raw()),Err(ProtectionError::Busy)));drop(n);assert_eq!(r.pin_count("lower"),Ok(0)); }
    #[test] fn root_mismatch_or_missing_policy_is_rejected() { let r=Registry::new();r.configure("lower",raw()).unwrap();let mut p=policy();p.expected_root=Some([8;32]);assert!(matches!(r.configure("top",protected("lower")).unwrap().verify(&p),Err(ProtectionError::PolicyMismatch)));p.expected_root=None;assert!(matches!(r.configure("top",protected("lower")).unwrap().verify(&p),Err(ProtectionError::PolicyMismatch))); }
    #[test] fn policy_rejects_missing_layers_and_mutable_integrity() { let r=Registry::new();r.configure("lower",raw()).unwrap();for c in [Configuration{encrypted:false,..protected("lower")},Configuration{integrity:false,..protected("lower")},Configuration{read_only:false,..protected("lower")}] { assert!(matches!(r.configure("top",c).unwrap().verify(&policy()),Err(ProtectionError::PolicyMismatch))); } }
    #[test] fn graph_changes_invalidate_all_dependent_receipts() { let (r,c)=prepared();let other=r.configure("other",protected("lower")).unwrap().verify(&policy()).unwrap();r.configure("lower",raw()).unwrap();assert!(matches!(c.activate(),Err(ProtectionError::Stale)));assert!(matches!(other.activate(),Err(ProtectionError::Stale))); }
    #[test] fn shared_graph_dependency_is_pinned_once() { let r=Registry::new();r.configure("lower",raw()).unwrap();for n in ["left","right"]{let mut x=raw();x.dependencies=vec!["lower".into()];r.configure(n,x).unwrap();}let mut top=protected("left");top.dependencies.push("right".into());let m=r.configure("top",top).unwrap().verify(&policy()).unwrap().activate().unwrap();assert_eq!(r.pin_count("lower"),Ok(1));drop(m);assert_eq!(r.pin_count("lower"),Ok(0)); }
    #[test] fn dependency_cycle_is_rejected() { let r=Registry::new();r.configure("a",raw()).unwrap();let mut b=raw();b.dependencies=vec!["a".into()];r.configure("b",b).unwrap();let mut a=raw();a.dependencies=vec!["b".into()];assert!(matches!(r.configure("a",a),Err(ProtectionError::InvalidGraph))); }
    #[test] fn failed_activation_does_not_pin_partial_graph() { let (r,c)=prepared();r.remove("lower").unwrap();assert!(c.activate().is_err());assert_eq!(r.pin_count("top"),Ok(0)); }
    #[test] fn competing_reconfiguration_is_serialized() {
        for _ in 0..100 {
            let (r,c)=prepared();let other=r.clone();let gate=Arc::new(std::sync::Barrier::new(2));let g=gate.clone();
            let t=std::thread::spawn(move||{g.wait();other.configure("lower",raw()).map(|_|())});gate.wait();
            let activated=c.activate();let changed=t.join().unwrap();
            match (activated,changed) { (Ok(m),Err(ProtectionError::Busy))=>drop(m),(Err(ProtectionError::Stale),Ok(()))=>{},_=>panic!("unsafe interleaving") }
        }
    }
}
