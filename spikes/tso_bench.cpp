// SPDX-License-Identifier: GPL-2.0-only
// Spike 2: what does x86-style (TSO) memory ordering cost on this CPU?
//
// We cannot enable Apple's hardware TSO as a third party, so a translator must
// emit ordered accesses. This measures the raw price of that on arm64:
//   relaxed : plain ldr/str  (what a translator would emit with no TSO)
//   tso     : ldapr/stlr     (acquire loads, release stores; TSO-correct)
//   seqcst  : ldar/stlr + dmb
// across a streaming copy and a pointer-chase, single-threaded.
// It deliberately does NOT model FEX's real codegen; it bounds the floor.
// Build: c++ -std=c++20 -O2 tso_bench.cpp -o tso_bench
#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <numeric>
#include <random>
#include <vector>

using clk = std::chrono::steady_clock;

template <std::memory_order LD, std::memory_order ST>
static double stream(std::vector<uint64_t>& a, std::vector<uint64_t>& b, int reps) {
  auto t0 = clk::now();
  for (int r = 0; r < reps; ++r)
    for (size_t i = 0; i < a.size(); ++i) {
      uint64_t v = __atomic_load_n(&a[i], int(LD));
      __atomic_store_n(&b[i], v + 1, int(ST));
    }
  return std::chrono::duration<double>(clk::now() - t0).count();
}

template <std::memory_order LD>
static double chase(const std::vector<uint32_t>& next, size_t steps) {
  auto t0 = clk::now();
  uint32_t i = 0;
  for (size_t s = 0; s < steps; ++s)
    i = __atomic_load_n(&next[i], int(LD));
  volatile uint32_t sink = i;
  (void)sink;
  return std::chrono::duration<double>(clk::now() - t0).count();
}

int main() {
  const size_t n = 1 << 22;  // 32 MiB per array: larger than L2
  std::vector<uint64_t> a(n, 1), b(n, 0);
  const int reps = 8;

  double r = stream<std::memory_order_relaxed, std::memory_order_relaxed>(a, b, reps);
  double t = stream<std::memory_order_acquire, std::memory_order_release>(a, b, reps);
  double s = stream<std::memory_order_seq_cst, std::memory_order_seq_cst>(a, b, reps);
  std::printf("stream  relaxed %.3fs | acq/rel %.3fs (x%.2f) | seq_cst %.3fs (x%.2f)\n", r, t,
              t / r, s, s / r);

  std::vector<uint32_t> next(n);
  std::iota(next.begin(), next.end(), 0);
  std::mt19937 rng(1);
  for (size_t i = n - 1; i > 0; --i) std::swap(next[i], next[rng() % (i + 1)]);
  // single-cycle permutation via Sattolo would be stricter; shuffled is fine for a floor
  size_t steps = 40'000'000;
  double cr = chase<std::memory_order_relaxed>(next, steps);
  double ca = chase<std::memory_order_acquire>(next, steps);
  std::printf("chase   relaxed %.3fs | acquire %.3fs (x%.2f)\n", cr, ca, ca / cr);
  std::puts("Interpretation: ratios near 1.0 mean ordered accesses are cheap on this core;\n"
            "the real cost to measure next is FEX's atomics/LRCPC path on a real guest.");
  return 0;
}
