# SPDX-License-Identifier: GPL-2.0-only
foreach(s address_space_probe tso_bench)
  add_executable(${s} ${s}.cpp)
  target_compile_options(${s} PRIVATE -O2 -Wall -Wextra)
endforeach()
