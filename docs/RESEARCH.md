# ArmStation research notes (v2: translation-layer model)

*Updated 2026-10-24. Tags: **[Sourced]** = read from a linked page; **[Inference]** = my reasoning; **[Unverified]** = plausible but not confirmed. Re-verify anything load-bearing before building on it. This version replaces the earlier emulator-oriented notes.*

## 0. What changed

The goal is a **translation layer**, in the style of CrossOver and AnyPS5: run the game's own code, reimplement the console's system libraries, and translate graphics calls. It is not a console emulator, and it is PS5-only. Two corrections to my earlier advice:

1. **AnyPS5 does no CPU translation.** It relinks the game's x86-64 code to run natively on x86-64 Windows and Linux hosts. So "AnyPS5 for ARM" means AnyPS5 plus an x86-64 to ARM64 execution layer. That layer is the main new engineering. [Sourced]
2. **My earlier Rust-first recommendation should be revisited.** The pieces we'd build on (FEXCore, AnyPS5's libraries and shader recompiler) are all C++. See section 11.

## 1. Summary

- **Architecture that fits the goal:** a macOS ARM64 *runner* process that loads the guest ELF into a reserved address window, executes guest x86-64 through an embedded translator (FEXCore), and routes the guest's library calls into native HLE libraries (ported from AnyPS5) and a Metal-backed graphics layer. This is the CrossOver design with PS5 in place of Windows.
- **A working precedent exists for the hardest CPU piece.** A community fork of FEX-Emu already runs FEXCore inside a macOS ARM64 process with guest memory in a translated window, Apple-compliant JIT, and self-modifying-code tracking. [Sourced]
- **CodeWeavers is solving the same problem right now** for CrossOver: porting FEX to macOS ARM64 because Rosetta is ending. [Sourced]
- **Rosetta ends with macOS 28 (late 2027)** per CodeWeavers; macOS 27 is the last full release. Anything Rosetta-based has about one more OS cycle. [Sourced]
- **Biggest technical risk:** x86 memory ordering. Apple silicon has hardware TSO, but I found no public macOS way for third parties to use it. Software ordering can be severe. Measure first. [Sourced + Inference]
- **Biggest practical lever:** reuse AnyPS5's work (relinker logic, system libraries, RDNA 2 to SPIR-V recompiler). It is **GPL-2.0-only**, so ArmStation would become GPL-2.0-only too.

## 2. How CrossOver and Highball work, and the PS5 mapping

| Role | CrossOver / Highball (Windows) | ArmStation (PS5) |
|---|---|---|
| OS API layer | Wine reimplements Win32 | HLE libraries (libkernel, libc, libSce*), as AnyPS5's `core/libs/prx` |
| CPU | Rosetta 2 today; CrossOver is porting FEX for macOS 28+ | FEXCore (or equivalent) embedded in our runner |
| Graphics | Apple's D3DMetal (Game Porting Toolkit) | AGC command decode, RDNA 2 shader recompiler, then MoltenVK/Metal |
| Packaging | Per-game bottles and settings | Per-title prefix with converted modules |

- **Highball** is a free, open-source Mac app (GPL-3.0) positioned as Whisky's successor. It uses Rosetta and Apple's D3DMetal, which is downloaded separately under Apple's license. [Sourced] Its own README doesn't name its Windows layer, so I can't say whether it uses Wine directly.
- **CrossOver's ARM64 preview** is built on FEX for x86 translation. It is targeted at CrossOver 28, the author calls the macOS port non-trivial, and whether FEX on macOS matches Rosetta's speed is explicitly unproven. [Sourced]
- **CodeWeavers' timeline:** Rosetta ends with macOS 28 in late 2027, and Wine 10 added full ARM64EC support when paired with an emulator. [Sourced]

## 3. AnyPS5 anatomy

[All Sourced from the AnyPS5 repository and its docs unless noted.]

- **What it is:** a relinker that converts a decrypted PS5 executable into a native Linux ELF or Windows PE. "Nothing is emulated": the game's x86-64 code runs as host code and calls native reimplementations of system libraries. **GPL-2.0-only.**
- **Layout:** `core/relinker`, `core/libs/prx` (system library implementations built as shared libraries), `core/shader/recompiler`, plus `3rdparty/` (SPIRV-Tools), a web converter, docs, and tools.
- **Relinker pipeline:** optional `--to-intel` pass that lowers AMD-only instructions (e.g., SSE4a, CLZERO); read imports by NID; check syscalls; filter unused imports; build a System V dynamic section; convert the bundled `sce_module/*` modules; write the final ELF or PE.
- **NID binding:** each library export is renamed to its NID (a hash computed from the function name). The host OS loader binds the game's imports by NID.
- **Graphics path:** the game submits command buffers through the AGC driver; a PM4 executor handles state, draws and dispatches; shader variants are looked up in memory and on disk, else recompiled (RDNA decoder, control flow and structurization, translation to an IR, SSA and resource optimization, SPIR-V backend), then a Vulkan pipeline is built. A persistent shader cache exists.
- **Rules of the road:** every function either does exactly what it should or throws; missing imports fail at startup; silent stubs only for UI-only gaps and logged as technical debt; third-party code goes in as submodules; the relinker uses only the C++20 standard library; PRs must disclose AI assistance.
- **Status:** v0.1.0 and v0.1.1 appeared on 28 Sept 2026. The compatibility list had one confirmed title (Dreaming Sarah, a 2D platformer) at the time of one report. A separate report tested Dead Cells and measured about 55-60 FPS indoors and 30-35 outdoors on a high-end PC. Large 3D games are described as not close.
- **Known gaps** (its own technical-debt doc): stubbed save-data and common dialogs, no HMD support, barycentric coordinates skipped in shaders, several unknown AGC and libkernel signatures, a few audio and image codec limits, `--to-intel` cannot lower some instructions (RDPRU, MCOMMIT, some SHA forms). 3D audio produces no sound.
- **Build:** x86-64 Linux and Windows toolchains only; Windows needs a specific MinGW-w64. No macOS build guide. [Sourced]
- **Forks:** *Ryty* (GPL-2.0) adds PS4 support and an experimental `--macos` target with an Objective-C/C++ Metal graphics bridge; its macOS output is x86-64 under Rosetta. *PortPS5* (GPL-2.0-only) is a Windows-focused hard fork with a defined 1.0 gate (five pinned titles at 30+ fps).
- **Worth noting:** the Ryty macOS investigation found low-address PS5 titles cannot be mapped inside a macOS process at all. A translated-window design (below) is what removes that blocker.

## 4. The missing piece: CPU execution on ARM64

### Candidates

| Option | License | Status for our use |
|---|---|---|
| **FEXCore** (core of FEX-Emu) | MIT | **Recommended.** A Darwin port of FEXCore exists as a community fork: guest memory in a window at a host offset (fits a 47-bit guest space), JIT per Apple's W^X rules (`MAP_JIT`, `pthread_jit_write_protect_np`), asynchronous signals only at guest instruction boundaries, self-modifying code tracking per host page, Darwin replacements for Linux helpers. It builds FEXCore only (no tools or thunks) and serves guest Linux syscalls from a host program. Upstream FEX has said it has no plans to support macOS. [Sourced] |
| **Box64 dynarec** | MIT (repo) | Mature ARM64 dynarec; AVX and AVX2 mapped onto NEON since 0.3.0; code cache on disk; wrapped host libraries. Linux-oriented, no macOS mention. Strong reference, harder to embed. [Sourced] |
| **Static whole-binary translation** (Elevator paper, Remill + LLVM, rev.ng, Lasagne) | Mixed | Research-grade. Elevator translates whole x86-64 executables to AArch64 with no heuristics but with large code-size growth; no public release confirmed. Remill (Apache-2.0) lifts x86-64 including AVX to LLVM IR but is a library, not a recompiler. rev.ng is a decompiler in beta. Use only as a later optimization idea. [Sourced] |
| **Rosetta** | n/a | Useful as a baseline only; ends with macOS 28, and fixed low addresses fail under it. |

**Strategy:** FEXCore JIT first, with a persistent translation cache. Only consider ahead-of-time translation after real profiling of real titles. [Inference]

### Memory ordering (the main risk)

- x86 is TSO; ARM is weakly ordered. Apple silicon implements an x86-style TSO mode in hardware, which is credited for Rosetta's performance. [Sourced]
- **Linux patches** show the bit (`ACTLR.TSOEN`) can be set and context-switched by a kernel, including for virtualized Linux on Apple silicon. FEX can use it where the kernel allows. [Sourced]
- **I found no public macOS API for third-party processes to enable it.** [Unverified: absence of evidence]
- **Without it,** FEX's own notes say its software memory-model emulation (atomics plus LRCPC) "absolutely demolishes performance," and users often turn it off, which risks crashes (Unity games especially). Write-combined GPU memory is an extreme worst case; unified-memory systems fare much better. [Sourced]
- **Implication:** build a microbenchmark and a TSO strategy early. Options: software ordering for shared pages only, per-title configuration, or (long shot) a Linux-VM host that can use hardware TSO. [Inference]

### Other CPU facts

- **AVX/AVX2:** FEX's AVX work targets 128-bit ASIMD on Apple cores; Apple chips lack SVE2 256-bit, which blocked an earlier approach. [Sourced]
- **AMD-only instructions:** real PS5 code uses them (AnyPS5 maintains converters and tests for SSE4a, CLZERO and others). Verify FEXCore handles or can lower each one. [Sourced + Unverified]
- **JIT rules:** one `MAP_JIT` region under the Hardened Runtime, per-thread write-xor-execute, `sys_icache_invalidate` before executing. [Sourced]
- **Page size:** Apple silicon uses 16 KB pages; upstream FEX expected 4 KB, which is why a VM or the Darwin fork was needed. [Sourced]

## 5. Address space

- macOS leaves the first 4 GB unmapped, and further ranges (commpage, a GPU carveout of roughly `0x1000000000` to `0x6FC0000000`) are unavailable to user mappings. `MAP_FIXED` there fails with `ENOMEM`, natively and under Rosetta. [Sourced]
- Some PS5 titles link at address zero and are not position-independent. [Sourced]
- **Design:** reserve one large window and treat guest addresses as offsets into it (what the Darwin FEX fork does). Guest-visible GPU apertures then work too, because they are just guest offsets. [Sourced + Inference]
- **Consequence for the relinker:** on macOS we do not produce a Mach-O copy of the game. We produce a *prepared prefix* (converted modules plus metadata) and a **runner** that maps it into the window. [Inference]

## 6. HLE libraries: how guest code reaches native code

AnyPS5's libraries are x86-64 System V shared libraries that run natively beside the game. On an ARM host there are two ways to reuse them:

1. **Run them through the translator too.** Simple, but they are built against a host Linux or Windows userland (libc, pthread, SDL, Vulkan). Under translation they would need a whole x86 userland. Not attractive. [Inference]
2. **Recompile the library sources natively for macOS ARM64 and call them via thunks.** Each guest call crosses an ABI boundary: System V AMD64 (integers in `rdi, rsi, rdx, rcx, r8, r9`, floats in `xmm0-7`) to AAPCS64 (`x0-x7`, `v0-v7`), plus varargs, struct passing, stack alignment, TLS, and callbacks from native code back into guest code. FEX and Box64 both use generated thunks or wrapped libraries for exactly this. **Recommended.** [Inference; Box64 approach Sourced]

Known gotchas to plan for: guest `wchar_t` is 16 bits while the host's is 32 (AnyPS5 already hit this), thread cancellation is not modeled, `PTHREAD_PROCESS_SHARED` semantics are unclear, and the libraries need host abstraction for Linux/Windows specifics (futex-like waits, `mmap`, signals). [Sourced]

## 7. Graphics

- **Pipeline** (as AnyPS5): AGC submission, PM4 execution, shader recompile, pipeline build. Reuse the recompiler front end (RDNA decode, IR, SPIR-V) rather than rewriting it. AMD publishes the RDNA 2 ISA guide. [Sourced]
- **Stage 1 on macOS:** SPIR-V through MoltenVK. Both KytyPS5 and SharpEmu already do this on macOS. [Sourced]
- **Stage 2:** an MSL backend on the same IR (SPIRV-Cross as a stopgap, direct emission later) to drop the Vulkan layer. [Inference]
- **Apple GPU features** (M1-M5): tessellation (factor 64), mesh shaders, ray tracing, BC textures (guaranteed on Apple9+), sparse textures, argument buffers tier 2, programmable blending, raster order groups. Geometry shaders are not listed. [Sourced]
- **TBDR differences:** multi-pass renderers that bounce through memory may need restructuring; custom blending needs raster order groups. [Sourced, one blog]
- **Known recompiler gaps** (AnyPS5): barycentrics skipped, `v_fma_f64` not fully exact for subnormals, some AGC signatures unknown. Dead Cells needs MRT and RGBA16F. [Sourced]
- **Descriptors:** AMD-style descriptors in memory must be translated to Metal argument buffers per draw. [Inference]

## 8. Input, audio, performance

- **Input:** AnyPS5 uses SDL mappings plus an `anyps5-input.ini`. On macOS, use the GameController framework for DualSense features (touchpad, adaptive triggers, motion), with SDL as a baseline. [Sourced]
- **Audio:** AnyPS5 ships a software NGS2 engine for Dead Cells; 3D audio is silent. Core Audio backend still to research. [Sourced]
- **Performance tooling:** Metal Performance HUD via `MTL_HUD_ENABLED=1` (plus logging variables) or Info.plist; Metal counter sample buffers for GPU timing (note: the HUD's encoder timing is unavailable if the app uses counter buffers itself). Build our own telemetry from day one: frame time from guest flips, translator cache stats, shader compile counts, unimplemented-import counters, `os_signpost`. [Sourced + Inference]

## 9. Licensing and legal

| Component | License |
|---|---|
| AnyPS5, Ryty, PortPS5 | GPL-2.0-only |
| shadPS4, SharpEmu | GPL-2.0 |
| Highball | GPL-3.0 (cannot mix code with GPL-2.0-only) |
| FEX-Emu, Box64 | MIT |
| Remill | Apache-2.0 (the FSF treats Apache-2.0 as incompatible with GPL-2.0-only; LLVM itself carries an exception, Remill's page lists plain Apache-2.0) |

- **Recommendation:** if ArmStation vendors AnyPS5 libraries or shader code, release it as **GPL-2.0-only**. MIT code (FEXCore) combines fine into it. This also lets us upstream fixes. Avoid Apache-2.0-only dependencies in the GPL core. This is not legal advice.
- **Posture:** like AnyPS5, ship no firmware, keys, Sony headers or games, and accept only user-supplied, already-decrypted binaries. The DMCA section 1201 anti-circumvention rule is the main exposure; keep decryption entirely out of scope. [Sourced context]
- AnyPS5's contribution rules don't state a clean-room policy. We should write our own and avoid leaked or proprietary Sony material.

## 10. Strategy options

| Option | Pros | Cons |
|---|---|---|
| **A. Fork AnyPS5/Ryty, add an ARM host** | Fastest to first pixels; shared fixes | Inherit their architecture; macOS needs deep surgery; heavy rebasing |
| **B. New host runtime that vendors AnyPS5 libs and recompiler as submodules (recommended)** | Our code is the new part (runner, FEXCore bridge, window memory, thunks, Metal); upstream stays upstream | Needs upstream cooperation on host abstraction |
| C. Write everything from scratch | Clean slate | Years of duplicated work; not recommended |

**Talk to the AnyPS5 maintainers early** about a host-abstraction layer in the libraries. Their cooperation determines how painful option B is. [Inference]

## 11. Language

The ecosystem points to **C++20 with CMake** for the core: FEXCore has no stable C API, AnyPS5's libraries and shader recompiler are C++20, and thunk generation and ABI work sit right on that boundary. Rust is a good fit for the standalone ELF/NID inspection tooling we already wrote, but forcing Rust through a C++ FFI for the runner would add friction for little gain. **Decision needed from you:** keep the Rust crates as an inspection tool and start the runner in C++, or restructure everything around C++. [Inference]

## 12. Revised milestones

| # | Milestone | Exit criterion |
|---|---|---|
| 0a | **FEXCore-on-macOS spike** | Build the Darwin FEX fork on your M3 Max; run a trivial static x86-64 ELF |
| 0b | **TSO microbenchmark** | Measure software-ordering cost on this machine; pick a strategy |
| 0c | **Window spike** | Confirm how large a reservation macOS grants; choose the guest window |
| 1 | Build AnyPS5's relinker and libraries for macOS ARM64 | Relinker runs; libraries compile natively |
| 2 | Runner loads a converted guest into the window under FEXCore | Guest reaches `main` and returns |
| 3 | Thunk generator plus libkernel/libc subset | A trivial title prints via HLE |
| 4 | First 2D title bring-up | A compatibility-list 2D title (e.g., Dreaming Sarah, user-supplied) reaches gameplay |
| 5 | Graphics stage 1 (MoltenVK) | Same title renders |
| 6 | Persistent translation cache, TSO tuning, telemetry | Frame times stable and measurable |
| 7 | Metal backend, DualSense features, audio | Parity with stage 1 plus controller features |

**Kill criteria:** if 0a fails to run guest code on macOS, or 0b shows ordering costs make 2D titles unplayable, reassess the Linux-VM host option (which can use hardware TSO but complicates graphics) before continuing.

## 13. Not yet researched or unverified

- Whether macOS exposes hardware TSO to third-party processes in any form.
- FEXCore's handling of each AMD-only instruction PS5 code uses.
- Ryty's Metal bridge design (documentation not accessible).
- Rosetta-for-Linux-VM capabilities and the CodeWeavers technical post (pages did not load).
- PS5 kernel ABI details, AGC packet formats, AMPR and storage behavior.
- Core Audio mapping and Tempest 3D audio.
- Whether upstream AnyPS5 would accept host-abstraction changes.

## 14. Sources

- AnyPS5 repository: https://github.com/boykopovar/AnyPS5
- AnyPS5 architecture, technical debt, build, usage and contribution docs: https://github.com/boykopovar/AnyPS5/blob/main/docs/dev/ARCHITECTURE.md , https://github.com/boykopovar/AnyPS5/blob/main/docs/dev/TechnicalDebt.md , https://github.com/boykopovar/AnyPS5/blob/main/docs/dev/BUILD.md , https://github.com/boykopovar/AnyPS5/blob/main/docs/user/USAGE.md , https://github.com/boykopovar/AnyPS5/blob/main/CONTRIBUTING.md
- Ryty: https://github.com/aarvsn/Ryty and https://github.com/aarvsn/Ryty/issues/9
- PortPS5: https://github.com/ymatrix1/PortPS5/wiki
- AnyPS5 explainers: https://pasqualepillitteri.it/en/news/21731/anyps5-ps5-games-windows-linux , https://explainx.ai/blog/anyps5-ps5-games-native-pc-no-emulator-how-relinking-works-2026
- Dead Cells native port: https://godisageek.com/2026/09/dead-cells-ps5-native-windows-port/
- FEX-Emu: https://github.com/FEX-Emu/FEX and discussion https://github.com/FEX-Emu/FEX/discussions/3267
- FEX Darwin port (community fork): https://github.com/Zagrios/FEX
- FEX hardware TSO release notes: https://fex-emu.com/FEX-2306/
- Box64: https://github.com/ptitSeb/box64 and https://www.heise.de/en/news/Box64-0-3-0-x86-64-emulator-for-Linux-supports-AVX-and-AVX2-9797911.html
- CrossOver ARM64 preview coverage: https://www.ithinkdiff.com/?p=343215
- CodeWeavers PortJump update: https://www.codeweavers.com/blog/orudge/2026/6/19/portjump-update-upcoming-changes-to-macos-support-for-intel-based-applications
- Highball: https://iphonesoft.fr/2026/10/06/highball-nouvelle-solution-jouer-jeux-windows-mac
- Elevator static translation: https://arxiv.org/abs/2605.08419
- Remill: https://github.com/lifting-bits/remill
- rev.ng: https://rev.ng/blog/open-sourcing-renvg-decompiler-ui-closed-beta
- Memory-model emulation: https://hackaday.com/2026/09/19/emulating-memory-access-how-hard-can-it-be/
- Apple silicon TSO patches: https://lkml.iu.edu/hypermail/linux/kernel/2404.1/04656.html
- Rosetta timeline report: https://www.techrepublic.com/article/news-apple-macos-27-drops-intel-mac-support-rosetta/
- macOS address space: https://developer.apple.com/forums/thread/781966 , https://github.com/shadps4-emu/shadPS4/issues/4157 , https://github.com/shadps4-emu/shadPS4/issues/4694
- Apple JIT guidance: https://developer.apple.com/documentation/apple-silicon/porting-just-in-time-compilers-to-apple-silicon
- Metal feature set tables: https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
- Apple GPU TBDR notes: https://blakecrosley.com/blog/apple-silicon-tbdr
- AMD RDNA 2 ISA: https://gpuopen.com/rdna2-isa-available/
- DualSense in GameController: https://developer.apple.com/documentation/gamecontroller/gcdualsensegamepad
- Metal Performance HUD and counters: https://developer.apple.com/documentation/xcode/monitoring-your-metal-apps-graphics-performance , https://developer.apple.com/documentation/metal/gpu-counters-and-counter-sample-buffers

---

## 13. Round 2b: community prior art, test reality, and what changed

Sources are public repos and reports; items I could not open are marked unverified.

**Overlap with existing ARM work.** Community macOS builds of AnyPS5 already exist, all running x86-64 under Rosetta 2 plus MoltenVK. One fork (`utkuhalis/AnyPS5-ARM`, branch `native-arm64`) goes further: it translates guest code ahead of time (Capstone to LLVM IR to arm64), identity-maps guest memory, and reports homebrew DOOM playing natively with no Rosetta. Its own roadmap lists the hard parts we also identified: setjmp/longjmp and exception landing pads, memory ordering, variadic and `long double` bridges. Its measurements also say real 3D titles are CPU/driver-bound (Stray ~100-150 ms/frame, GPU busy ~41-45 ms), so the graphics backend is not the first bottleneck.

**Design tradeoff.**

| | Guest window + JIT (FEXCore, ours) | Identity map + AOT (`native-arm64`) |
|---|---|---|
| Guest addresses | `host = GuestBase + guest`; works around the unmapped low 4 GB | Same values as host; cannot host low-linked images |
| Code | Runtime JIT, handles self-modifying code and late-loaded modules | Ahead of time; unknown indirect targets stop the process |
| Ordering | Software TSO via atomics/LRCPC, cost unmeasured | Acquire/release option, cost unmeasured |
| Maturity | FEXCore is mature; macOS fork is a community port | Early; small number of fixtures |

We keep the window + JIT design as the primary path because it handles the low-address and dynamic-code cases; the spikes exist to falsify that choice cheaply.

**Rosetta gaps** (from community reports, unverified by us): no SSE4a or ADX; faults on RDSEED, CLWB, RDPID. FEXCore's decoder tables contain these opcodes, which is encouraging but not proof of correct behavior.

**Metal/MoltenVK gaps:** no f64, 64-bit atomics or shader clock in Metal; MoltenVK has a SPIRV-Cross loop-continue bug and lacks unrestricted depth range. These constrain the shader recompiler's output.

**Legal.** Yuzu's maker settled for $2.4M on anti-circumvention claims even though it shipped no keys, so ArmStation stays strictly on decrypted input and ships no decryption. Sony has reportedly not acted against AnyPS5. This is not legal advice.

**Reported compatibility reality:** 2D indie titles reach 55-60 fps on high-end PCs; independent tests of other PS5 projects found no stable play in a six-game sample. Expect a long tail.

**Strategy change.** Rather than compete, ArmStation should position itself as the Rosetta-free runtime that vendors AnyPS5's libraries and recompiler (GPL-2.0-only, hence our license) and coordinate with the `native-arm64` maintainer. Outreach is something I can draft; nothing has been posted.

**Timeline (my estimate):** spikes, days; hello-world guest through the translator, weeks; a playable 2D title, months; AAA 3D, years.
