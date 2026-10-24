//! CPU bridge: runs guest x86-64 code on Apple Silicon.
//!
//! Not implemented yet. Backends are expected to sit behind `CpuBackend` so a
//! user-space dynamic translator and a Hypervisor.framework-based one can be
//! compared against each other.

/// An execution strategy for guest x86-64 code.
pub trait CpuBackend {
    fn name(&self) -> &'static str;
}
