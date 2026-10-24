//! System function stubs (HLE of PS5 OS libraries such as libkernel).
//!
//! Not implemented yet. Planned design: the relinker resolves each imported NID
//! (hashed symbol name) against a registry of `HleModule`s; unresolved imports bind
//! to a trap that logs the NID and aborts, so missing functions are loud and findable.

/// A high-level-emulated system library.
pub trait HleModule {
    /// Library name as it appears in the import table, e.g. `"libkernel"`.
    fn name(&self) -> &'static str;
}
