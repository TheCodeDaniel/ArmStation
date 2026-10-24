# ArmStation

> A translation layer, in the spirit of CrossOver and AnyPS5, that aims to run **decrypted PlayStation 5 executables** natively on ARM64 hosts, starting with Apple Silicon Macs.

**Status: research and planning.** Today the repository contains a hardened ELF64 inspector for decrypted PS5 executables and a detailed research document. It cannot run games. See [docs/RESEARCH.md](docs/RESEARCH.md) for what exists, what is hard, and the proposed build order.

## What this is (and isn't)

ArmStation is **not a console emulator**. It follows the translation-layer model:

| Role | CrossOver (Windows) | ArmStation (PS5) |
|---|---|---|
| System API layer | Wine reimplements Win32 | Reimplemented PS5 system libraries (libkernel, libc, libSce*) |
| CPU | Rosetta 2, moving to FEX | x86-64 to ARM64 translation, with FEXCore as the leading candidate |
| Graphics | D3DMetal | AGC command decode, RDNA 2 shader recompiler, MoltenVK then Metal |

The closest prior art is [AnyPS5](https://github.com/boykopovar/AnyPS5), which relinks PS5 executables to run natively on x86-64 Windows and Linux. It does no CPU translation, so it cannot run on ARM hosts. ArmStation's goal is to supply the ARM host side: a macOS ARM64 runner, an embedded x86-64 translator, native-ABI bridging for the system libraries, and a Metal backend.

## Why now

- Rosetta 2 is ending: macOS 27 is the last release with full support, and CodeWeavers expects it to end with macOS 28.
- macOS refuses to map the low 4 GB and a large GPU carveout, which PS5 titles expect to use, so guest addresses must be translated into a reserved window.
- A community fork has shown FEXCore running inside a macOS ARM64 process, so the hardest CPU piece has a working precedent.

## Current layout

```
armstation/
├── Cargo.toml                 workspace root
└── crates/
    ├── armstation-loader/     ELF64 parser, no unsafe code
    ├── armstation-cli/        `armstation inspect <elf>`
    ├── armstation-stubs/      placeholder
    ├── armstation-gfx/        placeholder
    └── armstation-cpu/        placeholder
```

The core runtime language is under review: most of what we would build on (FEXCore, AnyPS5's libraries and shader recompiler) is C++20. The Rust crates are kept as inspection tooling for now.

## Building the inspector

Requires a recent stable Rust toolchain.

```sh
cargo build --release
cargo test --workspace
./target/release/armstation inspect /path/to/decrypted/eboot.bin
```

It prints the ELF header (machine, OS/ABI, type, entry point), all program headers including Sony `PT_SCE_*` segments, section headers when present, and structural warnings.

## Legal and scope

- ArmStation **does not decrypt anything** and ships no keys, firmware, Sony SDK material, or game content. It rejects signed SELF containers and accepts only plain, already-decrypted ELF input supplied by the user.
- You are responsible for obtaining software lawfully.
- This project is not affiliated with or endorsed by Sony Interactive Entertainment, Apple, or Arm.
- `.gitignore` excludes `*.elf`, `*.self`, `*.bin`, `*.prx` and `*.sprx`. Never commit game files.

## Roadmap

The detailed version is in [docs/RESEARCH.md](docs/RESEARCH.md), section 12.

**Spikes (decide before building)**
- [ ] FEXCore on macOS: build the Darwin fork and run a trivial x86-64 ELF on Apple Silicon (see [docs/SPIKES.md](docs/SPIKES.md))
- [ ] Memory-ordering cost: measure x86 TSO emulation overhead on Apple Silicon
- [ ] Guest window: measure what a macOS arm64 process can reserve

**Build**
- [ ] Build AnyPS5's relinker and system libraries for macOS ARM64
- [ ] Runner: load a converted guest into the window and execute it via the translator
- [ ] ABI thunks (System V AMD64 to AAPCS64) and a libkernel/libc subset
- [ ] First 2D title bring-up
- [ ] Graphics via MoltenVK, then a Metal backend
- [ ] Persistent translation cache, memory-ordering tuning, telemetry
- [ ] DualSense support (touchpad, adaptive triggers, motion), audio

**Future: other ARM64 hosts**
- [ ] Linux ARM64 and Windows on ARM hosts behind a host-abstraction layer

## Contributing

Issues and PRs are welcome. Do not attach or link copyrighted game content, keys, or leaked Sony material.

## License

GPL-2.0-only. See [LICENSE](LICENSE). This keeps ArmStation compatible with AnyPS5's GPL-2.0-only relinker, system libraries and shader recompiler, which we intend to vendor as submodules rather than fork. (GPL-3.0 code such as Highball cannot be combined with it.) FEXCore is MIT and compatible.
