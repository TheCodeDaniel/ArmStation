use std::process::ExitCode;

use armstation_loader::elf::{
    e_type_name, os_abi_name, p_flags_string, p_type_name, sh_type_name, EM_X86_64,
};
use armstation_loader::{parse, ElfFile};

const USAGE: &str = "\
ArmStation - native PS5 compatibility layer for Apple Silicon (pre-alpha)

USAGE:
    armstation inspect <path-to-decrypted-eboot.bin>
    armstation --help | --version
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [a] if a == "--help" || a == "-h" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        [a] if a == "--version" || a == "-V" => {
            println!("armstation {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [cmd, path] if cmd == "inspect" => inspect(path),
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn inspect(path: &str) -> ExitCode {
    // TODO: switch to mmap once images are large enough to matter.
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    match parse(&data) {
        Ok(elf) => {
            print_report(path, data.len(), &elf);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {path}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_report(path: &str, size: usize, elf: &ElfFile) {
    let h = &elf.header;
    println!("File:            {path} ({size} bytes)");
    println!("Class/Encoding:  ELF64, little-endian");
    println!(
        "Machine:         {:#06x} ({})",
        h.e_machine,
        if h.e_machine == EM_X86_64 { "x86-64 confirmed" } else { "unexpected" }
    );
    println!("OS/ABI:          {} ({})", h.os_abi, os_abi_name(h.os_abi));
    println!("Type:            {:#06x} ({})", h.e_type, e_type_name(h.e_type));
    println!("Entry point:     {:#018x}", h.e_entry);
    println!("Flags:           {:#x}", h.e_flags);
    println!(
        "PH table:        offset {:#x}, {} entries x {} bytes",
        h.e_phoff, h.e_phnum, h.e_phentsize
    );
    println!(
        "SH table:        offset {:#x}, {} entries x {} bytes",
        h.e_shoff, h.e_shnum, h.e_shentsize
    );

    println!("\nProgram headers ({}):", elf.program_headers.len());
    println!(
        "  {:>2}  {:<20} {:<3} {:>12} {:>18} {:>12} {:>12} {:>8}",
        "#", "Type", "Flg", "Offset", "VirtAddr", "FileSz", "MemSz", "Align"
    );
    for (i, p) in elf.program_headers.iter().enumerate() {
        let ty = match p_type_name(p.p_type) {
            "unknown" => format!("{:#010x}", p.p_type),
            n => n.to_string(),
        };
        println!(
            "  {:>2}  {:<20} {:<3} {:>#12x} {:>#18x} {:>#12x} {:>#12x} {:>#8x}",
            i, ty, p_flags_string(p.p_flags), p.p_offset, p.p_vaddr, p.p_filesz, p.p_memsz, p.p_align
        );
    }

    if elf.section_headers.is_empty() {
        println!("\nSection headers: none (normal for stripped retail images)");
    } else {
        println!("\nSection headers ({}):", elf.section_headers.len());
        println!(
            "  {:>2}  {:<20} {:<10} {:>18} {:>12} {:>12}",
            "#", "Name", "Type", "Addr", "Offset", "Size"
        );
        for (i, s) in elf.section_headers.iter().enumerate() {
            println!(
                "  {:>2}  {:<20} {:<10} {:>#18x} {:>#12x} {:>#12x}",
                i,
                s.name.as_deref().unwrap_or("?"),
                sh_type_name(s.sh_type),
                s.sh_addr,
                s.sh_offset,
                s.sh_size
            );
        }
    }

    if !elf.warnings.is_empty() {
        println!("\nWarnings:");
        for w in &elf.warnings {
            println!("  - {w}");
        }
    }
}
