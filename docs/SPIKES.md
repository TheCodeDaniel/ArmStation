# Feasibility spikes

Three small experiments that decide whether the plan is viable **before** we build the runner. Each is days of work, not a prototype. All must be run on a real Apple Silicon Mac; the sandbox this repo was scaffolded in is Linux and can only compile-check them.

## Spike 3: address window (`spikes/address_space_probe.cpp`)

```sh
cmake -S . -B build -G Ninja && cmake --build build
./build/spikes/address_space_probe
```

Question: which of the low addresses PS5 titles want can macOS give us, and how big a contiguous reservation can we make? Research says the low 4 GB is unmapped, a commpage sits at `0xFC0000000`, and a GPU carveout occupies roughly `0x1000000000`-`0x6FC0000000`, so `MAP_FIXED` there should fail with ENOMEM. If section [1] shows FAIL and [3] shows a 16 GiB+ base, the guest-window (`host = GuestBase + guest`) design is confirmed; if low addresses succeed, an identity map (the `native-arm64` approach) is on the table. Paste the output into an issue.

Note: `MAP_JIT` in [4] needs the `com.apple.security.cs.allow-jit` entitlement once the binary is signed with the hardened runtime; unsigned local builds work without it.

## Spike 2: ordering cost (`spikes/tso_bench.cpp`)

```sh
./build/spikes/tso_bench
```

Question: how expensive is TSO-correct memory ordering on M-series cores? This is a **floor** (acquire/release on a stream and a pointer chase), not FEX's real codegen. Apple's hardware TSO is not available to third parties, so the translator must pay this. Ratios near 1.0 are good news; ratios of 2x+ mean the project needs per-region relaxation (e.g. only shared pages get TSO). The decisive follow-up is running a real multithreaded guest under FEX with TSO on and off.

## Spike 1: FEXCore on macOS (manual, unverified)

Prior art: the Darwin fork `Zagrios/FEX` (FEXCore only, `GuestBase` window, `MAP_JIT`) and `Zagrios/voile-lkd`, which serves Linux syscalls to guests via `fexdarwin-run`. Steps, all **unverified by us**:

1. Clone `Zagrios/FEX` and `Zagrios/voile-lkd`; follow `voile-lkd/build.sh` and `docs/`.
2. Run a static x86-64 Linux "hello world" through `fexdarwin-run`.
3. Record: does it run, JIT warm-up time, and whether SSE4a / ADX / RDSEED paths exist in the decoder (they do appear in FEXCore's decoder tables; coverage on real PS5 code is the open question).

Pass criteria: hello-world runs, and a tight integer loop is within ~3x native. Anything worse sends us back to the AOT-translation design.

## Why these three

They are the cheapest tests of the three biggest unknowns: can we place guest memory, can we afford x86 ordering, and does the CPU engine run on macOS at all.

## Results so far (GitHub Actions, macOS 14, Apple M1 virtual, 3 cores)

Raw output: `docs/spike-results/macos-arm64.txt`. Re-run with the `spikes` workflow (workflow_dispatch). Rerun on real hardware (M3 Max) before trusting any number.

- **Address window:** every fixed mapping failed with ENOMEM, including address 0, 4 MiB, 4 GiB, the commpage region and the GPU carveout start. A 16 GiB PROT_NONE reservation succeeded (base `0x300000000`), the largest single reservation was ~97,000 GiB, and MAP_JIT worked. **Confirms the guest-window design; identity mapping of low-linked PS5 images is not possible.**
- **Ordering cost (floor only):** acquire/release was 1.2-2.2x a plain stream copy (the plain loop may be auto-vectorised, so this overstates it) and 0.8-1.0x on a dependent pointer chase, i.e. roughly free when latency-bound. seq_cst was 1.3-2.3x. Run-to-run noise is large on a shared 3-core VM. Encouraging, not conclusive: FEX's real codegen is the test that matters.
- **Host features:** the runner reports FEAT_LRCPC and FEAT_LSE, the instructions a cheap TSO path relies on.
