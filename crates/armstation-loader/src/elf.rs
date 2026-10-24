//! ELF64 parsing for decrypted PS5 executables (eboot.bin, .prx/.sprx once decrypted).
//!
//! PS5 userland images are standard little-endian ELF64 x86-64 files with Sony-specific
//! `e_type` values and `PT_SCE_*` program header types. Retail images usually carry no
//! section headers at all, so a missing section table is normal, not an error.

use crate::error::{LoadError, Result};
use crate::reader::Reader;

pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
/// Magic of the signed SELF container that wraps retail ELFs.
pub const SELF_MAGIC: [u8; 4] = [0x4F, 0x15, 0x3D, 0x1D];

pub const EHDR_SIZE: usize = 64;
pub const PHDR_SIZE: u16 = 56;
pub const SHDR_SIZE: u16 = 64;
pub const EM_X86_64: u16 = 0x3E;

pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfHeader {
    pub class: u8,
    pub data: u8,
    pub ident_version: u8,
    pub os_abi: u8,
    pub abi_version: u8,
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramHeader {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionHeader {
    /// Resolved from .shstrtab when available.
    pub name: Option<String>,
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u64,
    pub sh_addr: u64,
    pub sh_offset: u64,
    pub sh_size: u64,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u64,
    pub sh_entsize: u64,
}

#[derive(Debug, Clone)]
pub struct ElfFile {
    pub header: ElfHeader,
    pub program_headers: Vec<ProgramHeader>,
    pub section_headers: Vec<SectionHeader>,
    /// Non-fatal oddities (segments past EOF, entry outside code, ...).
    pub warnings: Vec<String>,
}

/// Parse a decrypted PS5 ELF64 image from memory.
pub fn parse(data: &[u8]) -> Result<ElfFile> {
    let header = parse_header(data)?;
    let program_headers = parse_program_headers(data, &header)?;
    let mut section_headers = parse_section_headers(data, &header)?;
    resolve_section_names(data, &header, &mut section_headers);
    let warnings = collect_warnings(data, &header, &program_headers);
    Ok(ElfFile {
        header,
        program_headers,
        section_headers,
        warnings,
    })
}

fn parse_header(data: &[u8]) -> Result<ElfHeader> {
    let mut r = Reader::new(data, 0, "ELF header")?;
    let magic = r.bytes::<4>()?;
    if magic == SELF_MAGIC {
        return Err(LoadError::SignedSelf);
    }
    if magic != ELF_MAGIC {
        return Err(LoadError::NotElf);
    }
    let class = r.u8()?;
    let enc = r.u8()?;
    let ident_version = r.u8()?;
    let os_abi = r.u8()?;
    let abi_version = r.u8()?;
    r.skip(7)?;
    if class != 2 {
        return Err(LoadError::UnsupportedClass(class));
    }
    if enc != 1 {
        return Err(LoadError::UnsupportedEndian(enc));
    }
    let h = ElfHeader {
        class,
        data: enc,
        ident_version,
        os_abi,
        abi_version,
        e_type: r.u16()?,
        e_machine: r.u16()?,
        e_version: r.u32()?,
        e_entry: r.u64()?,
        e_phoff: r.u64()?,
        e_shoff: r.u64()?,
        e_flags: r.u32()?,
        e_ehsize: r.u16()?,
        e_phentsize: r.u16()?,
        e_phnum: r.u16()?,
        e_shentsize: r.u16()?,
        e_shnum: r.u16()?,
        e_shstrndx: r.u16()?,
    };
    if h.e_machine != EM_X86_64 {
        return Err(LoadError::WrongMachine(h.e_machine));
    }
    Ok(h)
}

fn entry_offset(base: u64, index: u64, size: u16, what: &'static str) -> Result<u64> {
    index
        .checked_mul(u64::from(size))
        .and_then(|o| base.checked_add(o))
        .ok_or(LoadError::OutOfBounds { what })
}

fn parse_program_headers(data: &[u8], h: &ElfHeader) -> Result<Vec<ProgramHeader>> {
    if h.e_phnum == 0 {
        return Ok(Vec::new());
    }
    if h.e_phentsize < PHDR_SIZE {
        return Err(LoadError::BadEntrySize {
            what: "program header",
            found: h.e_phentsize,
            expected: PHDR_SIZE,
        });
    }
    let mut out = Vec::with_capacity(usize::from(h.e_phnum));
    for i in 0..u64::from(h.e_phnum) {
        let off = entry_offset(h.e_phoff, i, h.e_phentsize, "program header table")?;
        let mut r = Reader::new(data, off, "program header")?;
        out.push(ProgramHeader {
            p_type: r.u32()?,
            p_flags: r.u32()?,
            p_offset: r.u64()?,
            p_vaddr: r.u64()?,
            p_paddr: r.u64()?,
            p_filesz: r.u64()?,
            p_memsz: r.u64()?,
            p_align: r.u64()?,
        });
    }
    Ok(out)
}

fn parse_section_headers(data: &[u8], h: &ElfHeader) -> Result<Vec<SectionHeader>> {
    if h.e_shnum == 0 || h.e_shoff == 0 {
        return Ok(Vec::new());
    }
    if h.e_shentsize < SHDR_SIZE {
        return Err(LoadError::BadEntrySize {
            what: "section header",
            found: h.e_shentsize,
            expected: SHDR_SIZE,
        });
    }
    let mut out = Vec::with_capacity(usize::from(h.e_shnum));
    for i in 0..u64::from(h.e_shnum) {
        let off = entry_offset(h.e_shoff, i, h.e_shentsize, "section header table")?;
        let mut r = Reader::new(data, off, "section header")?;
        out.push(SectionHeader {
            name: None,
            sh_name: r.u32()?,
            sh_type: r.u32()?,
            sh_flags: r.u64()?,
            sh_addr: r.u64()?,
            sh_offset: r.u64()?,
            sh_size: r.u64()?,
            sh_link: r.u32()?,
            sh_info: r.u32()?,
            sh_addralign: r.u64()?,
            sh_entsize: r.u64()?,
        });
    }
    Ok(out)
}

/// Best effort: a malformed string table just leaves names unresolved.
fn resolve_section_names(data: &[u8], h: &ElfHeader, sections: &mut [SectionHeader]) {
    let Some(strtab) = sections.get(usize::from(h.e_shstrndx)) else {
        return;
    };
    let Some(start) = usize::try_from(strtab.sh_offset).ok() else {
        return;
    };
    let Some(end) = usize::try_from(strtab.sh_size)
        .ok()
        .and_then(|s| start.checked_add(s))
    else {
        return;
    };
    let Some(table) = data.get(start..end) else {
        return;
    };
    let table = table.to_vec();
    for s in sections.iter_mut() {
        let Some(tail) = usize::try_from(s.sh_name).ok().and_then(|n| table.get(n..)) else {
            continue;
        };
        let len = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
        s.name = Some(String::from_utf8_lossy(&tail[..len]).into_owned());
    }
}

fn collect_warnings(data: &[u8], h: &ElfHeader, phdrs: &[ProgramHeader]) -> Vec<String> {
    let mut w = Vec::new();
    let len = data.len() as u64;
    for (i, p) in phdrs.iter().enumerate() {
        let end = p.p_offset.checked_add(p.p_filesz);
        if end.map_or(true, |e| e > len) {
            w.push(format!(
                "program header {i}: file range {:#x}+{:#x} extends past end of file ({len:#x} bytes)",
                p.p_offset, p.p_filesz
            ));
        }
        if p.p_type == PT_LOAD && p.p_memsz < p.p_filesz {
            w.push(format!("program header {i}: PT_LOAD memsz < filesz"));
        }
    }
    if h.e_entry != 0 {
        let in_exec = phdrs.iter().any(|p| {
            p.p_type == PT_LOAD
                && p.p_flags & PF_X != 0
                && h.e_entry >= p.p_vaddr
                && p.p_vaddr
                    .checked_add(p.p_memsz)
                    .is_some_and(|e| h.e_entry < e)
        });
        if !in_exec {
            w.push(format!(
                "entry point {:#x} is not inside any executable PT_LOAD segment (may be fine for dynamic images with PT_SCE_DYNLIBDATA relocations)",
                h.e_entry
            ));
        }
    }
    w
}

// ---------------------------------------------------------------------------
// Human-readable names
// ---------------------------------------------------------------------------

pub fn os_abi_name(v: u8) -> &'static str {
    match v {
        0 => "SYSV",
        3 => "Linux",
        9 => "FreeBSD",
        _ => "unknown",
    }
}

pub fn e_type_name(v: u16) -> &'static str {
    match v {
        0 => "ET_NONE",
        1 => "ET_REL",
        2 => "ET_EXEC",
        3 => "ET_DYN",
        0xFE00 => "ET_SCE_EXEC",
        0xFE01 => "ET_SCE_REPLAY_EXEC",
        0xFE04 => "ET_SCE_RELEXEC",
        0xFE0C => "ET_SCE_STUBLIB",
        0xFE10 => "ET_SCE_DYNEXEC",
        0xFE18 => "ET_SCE_DYNAMIC",
        _ => "unknown",
    }
}

pub fn p_type_name(v: u32) -> &'static str {
    match v {
        0 => "PT_NULL",
        1 => "PT_LOAD",
        2 => "PT_DYNAMIC",
        3 => "PT_INTERP",
        4 => "PT_NOTE",
        5 => "PT_SHLIB",
        6 => "PT_PHDR",
        7 => "PT_TLS",
        0x6474_E550 => "PT_GNU_EH_FRAME",
        0x6474_E551 => "PT_GNU_STACK",
        0x6474_E552 => "PT_GNU_RELRO",
        0x6000_0000 => "PT_SCE_RELA",
        0x6100_0000 => "PT_SCE_DYNLIBDATA",
        0x6100_0001 => "PT_SCE_PROCPARAM",
        0x6100_0002 => "PT_SCE_MODULEPARAM",
        0x6100_0010 => "PT_SCE_RELRO",
        0x6FFF_FF00 => "PT_SCE_COMMENT",
        0x6FFF_FF01 => "PT_SCE_LIBVERSION",
        _ => "unknown",
    }
}

pub fn p_flags_string(f: u32) -> String {
    format!(
        "{}{}{}",
        if f & PF_R != 0 { 'R' } else { '-' },
        if f & PF_W != 0 { 'W' } else { '-' },
        if f & PF_X != 0 { 'X' } else { '-' }
    )
}

pub fn sh_type_name(v: u32) -> &'static str {
    match v {
        0 => "NULL",
        1 => "PROGBITS",
        2 => "SYMTAB",
        3 => "STRTAB",
        4 => "RELA",
        5 => "HASH",
        6 => "DYNAMIC",
        7 => "NOTE",
        8 => "NOBITS",
        9 => "REL",
        11 => "DYNSYM",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le16(v: &mut Vec<u8>, x: u16) {
        v.extend_from_slice(&x.to_le_bytes());
    }
    fn le32(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_le_bytes());
    }
    fn le64(v: &mut Vec<u8>, x: u64) {
        v.extend_from_slice(&x.to_le_bytes());
    }

    /// Minimal valid image: ELF header + two program headers (LOAD R-X, SCE_DYNLIBDATA).
    fn synthetic() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&ELF_MAGIC);
        v.extend_from_slice(&[2, 1, 1, 9, 0, 0, 0, 0, 0, 0, 0, 0]);
        le16(&mut v, 0xFE10); // e_type
        le16(&mut v, EM_X86_64);
        le32(&mut v, 1);
        le64(&mut v, 0x1000); // entry
        le64(&mut v, 64); // phoff
        le64(&mut v, 0); // shoff
        le32(&mut v, 0);
        le16(&mut v, 64);
        le16(&mut v, 56);
        le16(&mut v, 2);
        le16(&mut v, 64);
        le16(&mut v, 0);
        le16(&mut v, 0);
        // PT_LOAD R-X
        le32(&mut v, PT_LOAD);
        le32(&mut v, PF_R | PF_X);
        le64(&mut v, 0);
        le64(&mut v, 0x1000);
        le64(&mut v, 0x1000);
        le64(&mut v, 0x10);
        le64(&mut v, 0x2000);
        le64(&mut v, 0x4000);
        // PT_SCE_DYNLIBDATA
        le32(&mut v, 0x6100_0000);
        le32(&mut v, PF_R);
        le64(&mut v, 0x10);
        le64(&mut v, 0x8000);
        le64(&mut v, 0x8000);
        le64(&mut v, 0x10);
        le64(&mut v, 0x10);
        le64(&mut v, 0x10);
        v.resize(0x200, 0);
        v
    }

    #[test]
    fn parses_synthetic_image() {
        let f = parse(&synthetic()).unwrap();
        assert_eq!(f.header.e_entry, 0x1000);
        assert_eq!(f.header.os_abi, 9);
        assert_eq!(e_type_name(f.header.e_type), "ET_SCE_DYNEXEC");
        assert_eq!(f.program_headers.len(), 2);
        assert_eq!(p_type_name(f.program_headers[1].p_type), "PT_SCE_DYNLIBDATA");
        assert_eq!(p_flags_string(f.program_headers[0].p_flags), "R-X");
        assert!(f.section_headers.is_empty());
    }

    #[test]
    fn rejects_non_elf_and_self() {
        assert!(matches!(parse(b"hello world, definitely not elf"), Err(LoadError::NotElf)));
        let mut s = SELF_MAGIC.to_vec();
        s.resize(64, 0);
        assert!(matches!(parse(&s), Err(LoadError::SignedSelf)));
    }

    #[test]
    fn rejects_wrong_machine() {
        let mut d = synthetic();
        d[18] = 0xB7; // EM_AARCH64 = 0xB7
        d[19] = 0x00;
        assert!(matches!(parse(&d), Err(LoadError::WrongMachine(0xB7))));
    }

    #[test]
    fn truncated_never_panics() {
        let d = synthetic();
        for n in 0..d.len().min(0x90) {
            let _ = parse(&d[..n]);
        }
        assert!(matches!(parse(&d[..100]), Err(LoadError::Truncated { .. })));
    }

    #[test]
    fn hostile_phoff_does_not_overflow() {
        let mut d = synthetic();
        d[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(parse(&d).is_err());
    }
}
