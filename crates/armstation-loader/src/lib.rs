//! ArmStation binary loader: parses *already-decrypted* PS5 ELF64 executables.
//!
//! This crate performs no decryption and contains no keys. It is `forbid(unsafe_code)`
//! and treats the input as hostile: every offset and size is bounds-checked.

#![forbid(unsafe_code)]

pub mod elf;
pub mod error;
mod reader;

pub use elf::{parse, ElfFile, ElfHeader, ProgramHeader, SectionHeader};
pub use error::{LoadError, Result};
