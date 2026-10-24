# ArmStation research notes

*Written 2026-10-24. Findings are tagged **[Sourced]** (read from a linked page), **[Inference]** (my reasoning from sourced facts or general knowledge) or **[Unverified]** (plausible, but I could not confirm it). Treat untagged design recommendations as inference. Re-verify anything load-bearing before building on it.*

## 1. Summary

1. **PS5 emulation already exists in early form.** KytyPS5, SharpEmu and AnyPS5 are public, and shadPS4 covers the PS4 side. Nobody has a broadly playable PS5 emulator, but ArmStation is not breaking new ground on "can it be done".
2. **Every macOS effort found is x86-64 plus Rosetta 2 plus MoltenVK.** None runs guest code natively on ARM64. That is ArmStation's real opening.
3. **Two macOS facts shape the whole design:**
   - **Rosetta is on a deprecation clock.** macOS 27 is reported as the last release with full Rosetta, so the Rosetta-based approach has a shelf life.
   - **macOS refuses to map the addresses PS5 titles want.** The low 4 GB is reserved and a large GPU carveout sits above it. Titles linked at low, non-relocatable addresses cannot be mapped as-is, even under Rosetta.
4. **Therefore ArmStation needs a guest-to-host address translation layer from day one**, and it cannot be bolted on later. This is the most important architectural decision in the project.
5. **GPU work is the largest cost**, but the path is well mapped: decode command streams, recompile RDNA 2 shaders through an IR, emit SPIR-V or MSL.
6. **Controller and performance tooling are the easy parts.** Apple's frameworks cover most of both.
7. **Licensing needs a decision now.** The closest prior art (shadPS4, SharpEmu) is GPL-2.0. ArmStation is MIT. Reusing their code means GPL.

## 2. Landscape

| Project | Target | Language / license | macOS story | Notes |
|---|---|---|---|---|
| shadPS4 | PS4 | C++, GPLv2 | Builds for macOS; runs as x86-64 under Rosetta | 317 titles "in game", 179 playable as of Nov 2025 (Wikipedia). Shader recompiler, shader cache, optional "readbacks" to emulate shared CPU/GPU memory. [Sourced] |
| KytyPS5 | PS5 | C++ | Experimental, x86-64 under Rosetta, Vulkan 1.3 via MoltenVK | Shader recompiler: decode, IR, control flow, resource tracking, SPIR-V. RDNA 2 ISA guide is the reference. Few titles verified on Apple Silicon. [Sourced] |
| SharpEmu | PS5 | C#, GPL-2.0 | x64 build via Rosetta 2, MoltenVK | Loads eboot.bin and system modules; partial kernel, Fiber and AMPR exports. 12 of 55 tested titles reached gameplay, 6 at a playable 60 FPS (Tom's Hardware). Self-described as research. [Sourced] |
| AnyPS5 | PS5 | C++ | Not an end-user macOS target; macOS appears only in builds and tests | Converts PS5 executables to native Windows/Linux programs and replaces Sony libraries. Has a `relinker` component and handles AMD64-only instructions (SSE4A, CLZERO). [Sourced] |
| Ryty | PS5 | - | Mach-O relinker experiments | Documents exactly why low-address PS5 titles don't fit in a macOS process (section 4). [Sourced] |

Two details matter most:

- shadPS4's own maintainers say its ARM64 CPU path is a stub. All guest code relies on Rosetta. [Sourced, shadPS4 issue 4157]
- Kyty's docs state that external low-level emulation modules are neither required nor planned, meaning system libraries are reimplemented in-process (HLE). [Sourced]

**PS4 versus PS5 is not "one runtime".** [Inference] PS5 runs PS4 titles in a separate compatibility mode, and PS4 and PS5 builds use different graphics APIs (GNM for PS4; AGC for PS5, descended from it). Supporting PS5 executables does not automatically cover PS4 games. PS4 support would be additional work, though much of the loader, kernel and CPU work is shared.

## 3. Rosetta is on a clock

- Tahoe (macOS 26) is the last release for Intel Macs, and macOS 27 is reported as the last release with full general-purpose Rosetta. After that, only a limited Rosetta for older games is reported to remain. [Sourced: TechRepublic report; I did not find an official Apple statement]
- The attribution is second-hand, so confirm against Apple's developer documentation before planning around exact dates.
- **Implication:** an x86-64 + Rosetta ArmStation would work for roughly one more OS cycle. A native ARM64 execution path is not optional for a long-lived project. [Inference]

## 4. The address-space problem

### What macOS does [Sourced]

- The first 4 GB of the address space is deliberately unmapped on both x86-64 and arm64, so truncated 32-bit pointers crash instead of corrupting memory. `MAP_FIXED` below 4 GB fails and `MAP_32BIT` doesn't exist (Apple Developer Forums).
- On an M4 Max, a shadPS4 contributor measured further reserved ranges: a commpage at `0xFC0000000` to `0xFFFFFFFFF` and a GPU carveout of roughly `0x1000000000` to `0x6FC0000000`. User mappings start around `0x7000000000`. `MAP_FIXED` into the gap returns `ENOMEM`. (shadPS4 issue 4157)
- A second report tested `MAP_FIXED` at `0x1000000000` on native arm64 as well as Rosetta and got `ENOMEM` for every size. (shadPS4 issue 4694)

### What PS5 titles do [Sourced]

- Some titles (Stray, Biomutant per the Ryty thread) are linked at low addresses, starting at or near 0, and are **not position-independent**.
- The Ryty author found that under Rosetta only a classic 4 GB `__PAGEZERO` layout loads at all, and `MAP_FIXED` fails below about `0x10000000`. A contributor concluded such titles cannot be mapped inside a Rosetta process.
- shadPS4 titles such as GTA V (fixed mapping near `0x1307200000`) and The Last of Us (PRT aperture at `0x1000000000`) crash because the OS won't grant the requested address. Maintainers say there is no general fix, since many games hardcode these addresses. shadPS4 uses identity-mapped guest and host addresses, so honoring them would need a real translation layer.

### Options

| Option | Verdict |
|---|---|
| **A. Translate every guest address** (`host = base + guest`) inside the translated code | **Recommended.** The only approach that handles low-address and GPU-aperture titles. Costs an add per memory access, which can fold into ARM64 addressing modes. [Inference] |
| B. Relink and rebase the binary | Fails for non-PIE titles with absolute addresses, unless the recompiler patches every absolute reference. High effort. |
| C. Run an ARM64 Linux VM (Hypervisor/Virtualization framework) with an x86 translator inside | A Linux guest kernel can map low addresses. But there is no obvious path to give the VM direct Metal access, and I could not verify Rosetta-for-Linux limits (fetch failed). Worth a later spike, not a base design. [Unverified] |
| D. Rosetta with a shrunk `__PAGEZERO` | Works only for titles that fit above roughly `0x10000000`. Doesn't solve the deprecation clock. |

**Design consequence for A:** reserve one large `PROT_NONE` window and treat guest addresses as offsets into it. Guest pointers up to 32 bits map cheaply (ARM64 supports a zero-extended 32-bit index register in addressing modes). Higher guest addresses need an explicit add. [Inference]

**First spike:** empirically measure how large a `PROT_NONE` reservation macOS arm64 grants on this machine, and where. Don't design around a number until this is run.

## 5. CPU bridge

### Constraints

- **Guest ISA:** Zen 2 x86-64, so AVX/AVX2 and BMI are in play. AnyPS5's test suite also covers AMD-only instructions (SSE4A EXTRQ/INSERTQ, CLZERO) that any translator must handle. [Sourced]
- **Rosetta and AVX/BMI:** evidence is inconclusive. A Stockfish report shows an AVX2-built binary dying on macOS 15.1.1 under Rosetta at a `blsrq` (BMI1) instruction. That specific instruction isn't AVX, so it doesn't settle AVX support. [Sourced, inconclusive]
- **Hypervisor.framework** runs ARM64 guests only. It cannot execute x86 code, so it is not a CPU bridge on its own. [Inference]
- **Memory ordering:** x86 is TSO; ARM is weakly ordered. Apple CPUs have a hardware TSO switch (`ACTLR.TSOEN`), and Linux patches expose it to x86 emulators. On macOS the bit was only modifiable on 14.4+ in that test setup. [Sourced] I found **no public macOS API** for third parties to enable it. Plan for software ordering (acquire/release forms or barriers). [Unverified]
- **Page size:** Apple Silicon uses 16 KB pages. Box64 added 16K-page support for Linux on Apple Silicon. [Sourced]

### JIT rules on macOS [Sourced, Apple documentation]

- Use `mmap` with `MAP_JIT`. Under the Hardened Runtime you need the `com.apple.security.cs.allow-jit` entitlement, and Apple's page says only one `MAP_JIT` region can be created. Allocate one big region up front.
- Apple Silicon enforces write-xor-execute per thread. Write via `pthread_jit_write_with_callback_np` (needs the `jit-write-allowlist` entitlement) or `pthread_jit_write_protect_np`, and call `sys_icache_invalidate` before executing.
- A command-line tool must be signed (ad-hoc is enough locally) with these entitlements.

### Candidate strategies

| Strategy | Notes |
|---|---|
| Rosetta | Fastest to prototype, short-lived, and blocked by the address problem for low-address titles. Use only as a comparison baseline. |
| Adapt FEX-Emu or Box64 | Both are Linux-oriented. A macOS FEX fork exists but I could not establish its status. Heavy porting; unclear fit with address translation. [Sourced: fork exists] |
| **Own translator, ahead-of-time with JIT fallback** | **Recommended direction.** Retail PS5 games generally don't generate code at runtime, so most of an executable could be translated offline and cached, with a dynamic translator only for indirect-jump misses. [Unverified: depends on titles; verify on real binaries] |

**First CPU spike:** a minimal x86-64 integer-subset translator running a hand-assembled test through the address-translation window. Measure instruction expansion and decide AOT versus JIT with data.

## 6. System layer

- **Loader:** the ELF parser exists. Next: map `PT_LOAD` segments into the guest window, parse `PT_DYNAMIC` and `PT_SCE_DYNLIBDATA`, apply relocations, build import and export tables.
- **Symbol IDs:** the PS4 library documentation project catalogs symbols by numeric id per module and library. [Sourced] How ids are computed wasn't covered; check OpenOrbis documentation. [Unverified]
- **HLE modules:** libkernel first (memory, threads, synchronization, time, files), then video-out, pad, user service, audio-out. Kyty and SharpEmu both take the in-process HLE route. [Sourced]
- **PS5-specific:** SharpEmu lists Fiber and AMPR exports. AMPR is Sony's storage and async-I/O machinery. The PS5's streaming-oriented I/O is cited as a hard part to reproduce. [Sourced]
- **Threads and TLS:** x86 uses an FS segment for thread-local storage; ARM64 has no equivalent. A translator must map guest TLS to a reserved register or `TPIDR_EL0`. [Inference]
- **Unresolved imports** should trap loudly with the symbol id, so gaps are easy to find.

## 7. Graphics

### Pipeline

Command stream decode, then state tracking, then shader recompilation, then Metal submission. Kyty's recompiler is a useful reference for the shape: instruction decoder, IR, control flow, resource tracking, SPIR-V emission, driven by AMD's RDNA 2 ISA guide. [Sourced]

AMD publishes the RDNA 2 ISA reference as a PDF (`amd.com/system/files/TechDocs/rdna2-shader-instruction-set-architecture.pdf`). [Sourced]

### Shader path decision

- **Stepping stone:** RDNA 2 to your own IR, then SPIR-V, then MSL via SPIRV-Cross. This mirrors what existing emulators do and de-risks the front end.
- **Later:** direct MSL emission from the IR, dropping the SPIR-V hop and MoltenVK. This is where "direct to Metal" actually pays off. Apple's Metal Shader Converter targets DXIL (D3D), not SPIR-V or RDNA, so it isn't a shortcut. [Inference from general knowledge]
- **Front-end reuse:** the IR and recompiler front end can be shared between SPIR-V and MSL back ends. Design the IR once.

### What Apple GPUs support [Sourced, Metal feature set tables]

Tessellation (max factor 64), mesh shaders, ray tracing (compute and render pipelines), BC texture compression (all Apple9+ and all Apple7/8 Macs), sparse textures, argument buffers tier 2, programmable blending and raster order groups. The document does not list geometry shaders. Check runtime capability rather than hardcoding by chip. M1 is Apple7, M2 Apple8, M3/M4 Apple9, M5 Apple10.

### Hard problems [Inference unless noted]

- **Tile-based deferred rendering.** Apple's GPU is TBDR; the PS5's is immediate-mode. Multi-pass renderers that bounce intermediates through memory may need restructuring, and fragment code that assumes ordered read-modify-write across overlapping primitives (custom blending) needs raster order groups. [Sourced, one blog]
- **Descriptors in memory.** AMD-style shaders fetch resource descriptors from memory. Metal's argument buffers use their own opaque format, so descriptors must be translated per draw.
- **Shared memory model.** Games assume CPU and GPU see the same memory. shadPS4's optional "readbacks" exist to emulate this and are costly. [Sourced] Apple's unified memory helps, but the GPU address carveout means guest GPU-visible addresses also go through translation (section 4).
- **Shader compile stalls.** shadPS4's shader cache (v0.13.0) improved performance. Plan a persistent shader cache early. [Sourced]
- **Missing stages.** Games using geometry-shader-like behavior need emulation on mesh shaders or compute. [Inference]

## 8. Input

- Apple's `GCDualSenseGamepad` (macOS 11.3+) exposes the touchpad button, two-finger touch positions, adaptive triggers with dynamic resistance, and motion. It inherits the extended gamepad profile. [Sourced]
- Haptics and lightbar: Apple also provides `GCController.haptics` (CoreHaptics) and light control. I could not retrieve the documentation, so confirm capabilities, especially for the DualSense's audio-driven haptics. [Unverified]
- **Plan:** GameController framework as the primary backend (also covers Xbox and other pads). Map it to the guest's pad library (libScePad-style API): buttons, sticks, analog triggers, touchpad, motion, trigger effects, rumble. If the framework can't reach a feature, raw HID is the fallback. [Inference]
- Add keyboard and mouse mapping for users without a pad.

## 9. Performance and observability

- **Metal Performance HUD** [Sourced]: shows device, resolution, memory, FPS, GPU time and a frame-interval graph. Enable with `MTL_HUD_ENABLED=1`, per-frame logging with `MTL_HUD_LOG_ENABLED=1`, and shader-compile logging with `MTL_HUD_LOG_SHADER_ENABLED=1`. It can also be enabled from `Info.plist` (`MetalHudEnabled`) or user defaults.
- **Counter sampling** [Sourced]: Metal exposes counter sets and counter sample buffers for GPU timestamps and statistics. Caveat from Apple's page: the HUD's encoder-timing feature is unavailable if your app uses counter sample buffers itself. Decide up front which tool owns GPU timing.
- **Frame pacing:** `CAMetalDisplayLink` (macOS 14+) is the likely mechanism for display-synchronized presentation. [Unverified: documentation page didn't load]
- **Own telemetry (build early):**
  - Frame time, drawn from the guest's flip events.
  - Translator stats: blocks translated, cache hit rate, indirect-branch misses.
  - Shader compile counts and times, plus cache hits.
  - Guest syscall and unimplemented-import counters.
  - `os_signpost` for Instruments, and a JSON trace export.
- **Guest timers** (TSC reads, process time) must be backed by `mach_absolute_time` with correct scaling, or frame pacing and game logic break. [Inference]

## 10. Legal and licensing

- Emulators are lawful to write; circumvention of encryption (DMCA section 1201) is the exposure. ArmStation accepts only already-decrypted ELF and ships no keys, firmware or game content. Keep it that way. Public reporting notes lawful dumping is fragmented and that anti-circumvention exemptions don't give consumers a blanket right. [Sourced: asoasis]
- **License decision needed.** shadPS4 is GPLv2 and SharpEmu is GPL-2.0. [Sourced] I did not confirm Kyty's license. Reusing any of their source, including shader recompiler logic, makes ArmStation GPL. Options: (a) relicense ArmStation GPLv2 and reuse freely; (b) stay MIT and work clean-room from public specs (the AMD ISA guide, observed behavior). Pick before writing more than glue code.
- Name and branding: see the trademark notes in the project history; keep the non-affiliation notice.

## 11. Proposed build order

Spike first, commit later. Each milestone has an exit test.

| # | Milestone | Exit criterion |
|---|---|---|
| 0 | **Address-space spike** | Measured map of what a macOS arm64 process can reserve; guest window design chosen |
| 1 | Loader complete | Dynamic section, relocations, import/export tables parsed on real or homebrew ELFs |
| 2 | CPU spike | Integer-subset translator runs a hand-built guest through the translation window; AOT versus JIT decided with numbers |
| 3 | libkernel HLE minimum | A trivial homebrew prints through stubbed syscalls |
| 4 | Presentation | Guest flip drives a Metal layer; solid-color then framebuffer output |
| 5 | Command decode and first shader | One triangle from a homebrew sample |
| 6 | Shader recompiler breadth | Shader cache, descriptor translation, TBDR-aware pass handling |
| 7 | Input and audio | DualSense via GameController; basic audio-out |
| 8 | Telemetry (parallel from milestone 2) | HUD, translator stats and frame-time trace available |

**Kill criteria:** if milestone 0 shows macOS cannot grant a window big enough for the guest layouts, or milestone 2 shows an unusable translation overhead, revisit option C (Linux VM) before investing further.

## 12. Not yet researched

- Audio: Tempest 3D audio, audio-out semantics, Core Audio mapping.
- PS5 kernel syscall ABI and libkernel specifics; how symbol ids are computed (OpenOrbis docs).
- AGC command stream details and PM4-style packet formats.
- Storage and I/O: AMPR, asset decompression.
- Rosetta-for-Linux-VM capabilities (AVX, TSO), needed to evaluate option C.
- Exact `CAMetalDisplayLink` and `GCController.haptics` behavior (pages didn't load).
- Save data, networking, trophies, user management stubs.
- Kyty's license.

## 13. Sources

- shadPS4 overview: https://en.wikipedia.org/wiki/ShadPS4
- shadPS4 macOS address-space issues: https://github.com/shadps4-emu/shadPS4/issues/4157 and https://github.com/shadps4-emu/shadPS4/issues/4694
- Ryty macOS relinker thread: https://github.com/aarvsn/Ryty/issues/9
- AnyPS5 test PR (macOS, Rosetta, zero page): https://github.com/boykopovar/AnyPS5/pull/6524
- KytyPS5 wiki: https://github.com/Jetsku/KytyPS5/wiki
- SharpEmu repository: https://github.com/GACWR/sharpemu
- SharpEmu results (Tom's Hardware): https://www.tomshardware.com/desktops/gaming-pcs/ps5-emulator-successfully-runs-six-titles-at-a-playable-60-fps-ps5-emulation-continues-to-gather-momentum-as-developers-improve-shader-translation-and-vulkan-support
- PS5 emulation overview: https://asoasis.tech/news/2026-09-28-2039-ps5-emulation/
- Rosetta end of life report: https://www.techrepublic.com/article/news-apple-macos-27-drops-intel-mac-support-rosetta/
- Rosetta and AVX2 report: https://github.com/official-stockfish/Stockfish/issues/5707
- macOS low-memory restriction: https://developer.apple.com/forums/thread/781966
- Apple Silicon TSO patches: https://lkml.iu.edu/hypermail/linux/kernel/2404.1/04656.html
- Box64 16K pages: https://www.phoronix.com/news/Box64-0.2.8-Released
- FEX macOS fork: https://github.com/Jpkovas/FEX_MacOs
- Apple JIT guidance: https://developer.apple.com/documentation/apple-silicon/porting-just-in-time-compilers-to-apple-silicon
- Metal feature set tables: https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
- Apple GPU TBDR notes: https://blakecrosley.com/blog/apple-silicon-tbdr
- AMD RDNA 2 ISA: https://gpuopen.com/rdna2-isa-available/
- DualSense in GameController: https://developer.apple.com/documentation/gamecontroller/gcdualsensegamepad
- Metal Performance HUD: https://developer.apple.com/documentation/xcode/monitoring-your-metal-apps-graphics-performance
- Metal GPU counters: https://developer.apple.com/documentation/metal/gpu-counters-and-counter-sample-buffers
- PS4 library documentation format: https://github.com/Inori/ps4libdoc
