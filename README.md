# ArmStation

> A native, Metal-first compatibility layer that aims to run **decrypted PlayStation 5 executables** on Apple Silicon Macs, with no Wine, Proton, or CrossOver involved.

**Status: pre-alpha.** Today ArmStation can parse and describe a decrypted PS5 ELF. It cannot run games, and it will be a long time before it can. See the [roadmap](#roadmap) for an honest picture of what is left.

## Why not CrossOver?

CrossOver, Wine and Proton translate the **Windows** API and ABI (and D3D to Metal/Vulkan). A PS5 title is not a Windows program: it targets a FreeBSD-derived OS (Orbis/Prospero), Sony's system libraries, and AMD GPU command streams. There is no Windows layer for Wine to take over, so a Wine-based approach has nothing to build on.

ArmStation instead targets the real interfaces a PS5 game uses:

- **Direct-to-Metal graphics.** GPU work is translated straight to Metal and MSL, with no D3D/Vulkan/MoltenVK hop in between.
- **Native system layer.** `libkernel` and other system library calls are handled by purpose-built stubs rather than a general-purpose OS emulation.
- **No framework overhead.** A pure Rust workspace; no Electron, no Flutter, no runtime beyond the standard library.
- **Memory-safe loader.** The binary loader is `#![forbid(unsafe_code)]` and treats every input as hostile.

## Architecture

```
armstation/
├── Cargo.toml                 workspace root
├── rust-toolchain.toml        stable, aarch64-apple-darwin
└── crates/
    ├── armstation-loader/     ELF64 parser, (later) relinker. No unsafe code.
    ├── armstation-cli/        `armstation` binary
    ├── armstation-stubs/      HLE of PS5 system libraries (libkernel, ...)
    ├── armstation-gfx/        GPU command stream and shader translation -> Metal/MSL
    └── armstation-cpu/        x86-64 guest execution backends on ARM64
```

| Layer | Purpose | State |
|---|---|---|
| Loader | Parse and validate the executable, later map and relocate segments | Parser done |
| Stubs | Resolve imported system functions to host implementations | Placeholder |
| Graphics | Translate PS5 GPU commands and shaders to Metal | Placeholder |
| CPU bridge | Execute x86-64 guest code on ARM64 | Placeholder |

The CPU bridge is deliberately left open: a user-space dynamic binary translator and a Hypervisor.framework approach have very different tradeoffs, and the `CpuBackend` trait exists so both can be tried and compared.

### Portability by design

The name is deliberate: macOS on Apple Silicon is the first host, not the last. Longer term the goal is to run on other ARM64 hosts such as Windows on ARM and Linux on ARM64. To keep that door open, all host-specific code stays behind traits (`CpuBackend` for execution, and a graphics backend trait to come), and the loader and stubs layers must stay free of macOS-only APIs. Metal is the first graphics backend, not a permanent assumption.

## Building

Requires macOS on Apple Silicon and a recent stable Rust toolchain.

```sh
cargo build --release
cargo test --workspace
```

## Usage

```sh
./target/release/armstation inspect /path/to/decrypted/eboot.bin
```

Prints the ELF header (machine, OS/ABI, type, entry point), all program headers (including Sony `PT_SCE_*` segments), section headers when present, and any structural warnings.

## Legal and scope

- ArmStation **does not decrypt anything** and ships no keys, firmware, or game content. It rejects signed SELF containers and only accepts plain ELF input.
- You are responsible for obtaining software lawfully, for example from games you own and dumped yourself.
- This project is not affiliated with or endorsed by Sony Interactive Entertainment or Apple.
- `.gitignore` excludes `*.elf`, `*.self`, `*.bin`, `*.prx` and `*.sprx`. Never commit game files.

## Roadmap

**Phase 0: Foundation (current)**
- [x] Workspace layout
- [x] Hardened ELF64 parser, with tests for truncated and hostile input
- [x] `inspect` command

**Phase 1: Loader and relinker**
- [ ] Map `PT_LOAD` segments into a reserved guest address space
- [ ] Parse `PT_DYNAMIC` / `PT_SCE_DYNLIBDATA`: symbol tables, NIDs, relocations
- [ ] Apply relocations and build the import/export tables
- [ ] Module dependency resolution (`.prx`)

**Phase 2: System stubs**
- [ ] Import registry with loud unresolved-NID traps
- [ ] Minimal `libkernel`: memory, threads, synchronization, time
- [ ] Filesystem and logging shims

**Phase 3: CPU bridge**
- [ ] Prototype and benchmark both execution strategies behind `CpuBackend`
- [ ] Run a trivial hand-built x86-64 guest end to end
- [ ] Guest thread model, TLS, signal/exception handling

**Phase 4: Graphics**
- [ ] GPU command stream decoding
- [ ] Shader translation to MSL
- [ ] Present a first triangle from a homebrew sample

**Phase 5: Everything else**
- [ ] Audio, input, networking stubs
- [ ] Performance work, compatibility database

**Future: other ARM64 hosts**
- [ ] Define a host-neutral graphics backend interface (Metal first)
- [ ] Linux ARM64 host: Vulkan backend, native `mmap`-based memory manager
- [ ] Windows on ARM host: D3D12 or Vulkan backend
- [ ] CI builds for each host target

Phases 3 and 4 are research-grade problems. Contributors with experience in emulators, binary translation or GPU ISAs are especially welcome.

## Contributing

Issues and PRs are welcome. Please run `cargo fmt`, `cargo clippy --workspace` and `cargo test --workspace` before submitting. Do not attach or link copyrighted game content or keys.

## License

MIT. See [LICENSE](LICENSE).
