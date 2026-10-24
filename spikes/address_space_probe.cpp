// SPDX-License-Identifier: GPL-2.0-only
// Spike 3: what guest address window can a macOS arm64 process reserve?
//
// Probes (a) fixed low addresses (PS5 titles may link at 0 / low 4 GB),
// (b) the largest contiguous PROT_NONE reservation anywhere, and
// (c) whether MAP_JIT regions can be created inside a reservation.
// Build: c++ -std=c++20 -O2 address_space_probe.cpp -o address_space_probe
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <sys/mman.h>
#include <cerrno>

#ifndef MAP_JIT
#define MAP_JIT 0
#endif
#ifndef MAP_NORESERVE
#define MAP_NORESERVE 0
#endif

static void try_fixed(uint64_t addr, uint64_t len, const char* label) {
  void* p = mmap(reinterpret_cast<void*>(addr), len, PROT_NONE,
                 MAP_PRIVATE | MAP_ANON | MAP_FIXED | MAP_NORESERVE, -1, 0);
  if (p == MAP_FAILED) {
    std::printf("  fixed %-22s 0x%012llx +0x%llx: FAIL (%s)\n", label,
                (unsigned long long)addr, (unsigned long long)len, std::strerror(errno));
  } else {
    std::printf("  fixed %-22s 0x%012llx +0x%llx: ok\n", label,
                (unsigned long long)addr, (unsigned long long)len);
    munmap(p, len);
  }
}

static uint64_t largest_reservation() {
  // Binary search on the size of a single anonymous PROT_NONE mapping.
  uint64_t lo = 0, hi = 1ull << 47;
  while (hi - lo > (1ull << 24)) {
    uint64_t mid = lo + (hi - lo) / 2;
    void* p = mmap(nullptr, mid, PROT_NONE, MAP_PRIVATE | MAP_ANON | MAP_NORESERVE, -1, 0);
    if (p == MAP_FAILED) {
      hi = mid;
    } else {
      munmap(p, mid);
      lo = mid;
    }
  }
  return lo;
}

int main() {
  std::printf("address_space_probe: pointer bits=%zu\n", sizeof(void*) * 8);

  std::puts("[1] fixed low-address attempts (PS5 non-PIE images link near 0):");
  try_fixed(0x0, 1ull << 20, "page zero");
  try_fixed(0x10000, 1ull << 24, "64K");
  try_fixed(0x400000, 1ull << 24, "4M (typical ELF base)");
  try_fixed(0x100000000ull, 1ull << 28, "4G");
  try_fixed(0xFC0000000ull, 1ull << 24, "commpage region");
  try_fixed(0x1000000000ull, 1ull << 28, "gpu carveout start");

  std::puts("[2] largest single PROT_NONE reservation:");
  uint64_t big = largest_reservation();
  std::printf("  %llu GiB (0x%llx)\n", (unsigned long long)(big >> 30), (unsigned long long)big);

  std::puts("[3] guest window: reserve 16 GiB, report base:");
  uint64_t want = 16ull << 30;
  void* w = mmap(nullptr, want, PROT_NONE, MAP_PRIVATE | MAP_ANON | MAP_NORESERVE, -1, 0);
  if (w == MAP_FAILED) {
    std::printf("  FAIL (%s)\n", std::strerror(errno));
    return 1;
  }
  std::printf("  base=%p\n", w);

  std::puts("[4] commit RW inside the window, then try a MAP_JIT page:");
  void* rw = mmap(w, 1 << 20, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON | MAP_FIXED, -1, 0);
  std::printf("  RW commit: %s\n", rw == MAP_FAILED ? std::strerror(errno) : "ok");
  void* jit = mmap(nullptr, 1 << 20, PROT_READ | PROT_WRITE | PROT_EXEC,
                   MAP_PRIVATE | MAP_ANON | MAP_JIT, -1, 0);
  std::printf("  MAP_JIT:   %s (needs com.apple.security.cs.allow-jit when hardened)\n",
              jit == MAP_FAILED ? std::strerror(errno) : "ok");
  return 0;
}
