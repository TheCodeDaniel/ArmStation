# SPDX-License-Identifier: GPL-2.0-only
cmake_minimum_required(VERSION 3.20)
project(ArmStation LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

option(ARMSTATION_BUILD_SPIKES "Build feasibility spikes" ON)
if(ARMSTATION_BUILD_SPIKES)
  add_subdirectory(spikes)
endif()
