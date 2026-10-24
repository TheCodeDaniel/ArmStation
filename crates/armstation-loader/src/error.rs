//! Error type for the loader. Hand-rolled so the crate stays dependency-free.

use std::fmt;

pub type Result<T> = std::result::Result<T, LoadError>;

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    /// A read would run past the end of the file.
    Truncated {
        what: &'static str,
        offset: u64,
        needed: u64,
        len: u64,
    },
    /// Magic bytes are not `\x7fELF`.
    NotElf,
    /// Magic matches a (signed/encrypted) SELF container, not a plain ELF.
    SignedSelf,
    UnsupportedClass(u8),
    UnsupportedEndian(u8),
    WrongMachine(u16),
    BadEntrySize {
        what: &'static str,
        found: u16,
        expected: u16,
    },
    /// A table offset/size computation overflowed or points outside the file.
    OutOfBounds {
        what: &'static str,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Truncated { what, offset, needed, len } => write!(
                f,
                "truncated file while reading {what}: need {needed} byte(s) at offset {offset:#x}, file is {len} byte(s)"
            ),
            Self::NotElf => write!(f, "not an ELF file (bad magic)"),
            Self::SignedSelf => write!(
                f,
                "file is a SELF container (signed/encrypted); ArmStation only accepts already-decrypted plain ELF input"
            ),
            Self::UnsupportedClass(c) => write!(f, "unsupported ELF class {c} (expected 2 = 64-bit)"),
            Self::UnsupportedEndian(d) => write!(f, "unsupported ELF data encoding {d} (expected 1 = little-endian)"),
            Self::WrongMachine(m) => write!(f, "unexpected machine {m:#06x} (expected 0x003e = x86-64)"),
            Self::BadEntrySize { what, found, expected } => {
                write!(f, "bad {what} entry size {found} (expected {expected})")
            }
            Self::OutOfBounds { what } => write!(f, "{what} lies outside the file or overflows"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
