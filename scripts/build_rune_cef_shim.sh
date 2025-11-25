#!/usr/bin/env bash
set -euo pipefail

# Build the rune_cef_shim shared library via CEF's CMake project
# and copy it into cef/Debug + cef/Release (handled by CMake).

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CEF_ROOT="$ROOT/cef"
BUILD_DIR="$CEF_ROOT/build"

log() {
  printf '[shim-build] %s\n' "$1"
}

if [[ ! -d "$BUILD_DIR" ]]; then
  log "Creating CEF build directory at $BUILD_DIR"
  mkdir -p "$BUILD_DIR"
fi

cd "$BUILD_DIR"

if [[ ! -f "CMakeCache.txt" ]]; then
  # No configuration yet; default to Ninja + Release.
  ARCH="${PROJECT_ARCH:-$(uname -m)}"
  log "Configuring CEF CMake project (ARCH=$ARCH)"
  cmake -G "Ninja" -DPROJECT_ARCH="$ARCH" -DCMAKE_BUILD_TYPE=Release ..
else
   # Refresh configuration so new targets (like rune_cef_shim) are visible.
   log "Re-configuring CEF CMake project using existing generator"
   cmake ..
fi

log "Building rune_cef_shim via CMake (generator: $(grep -m1 '^CMAKE_GENERATOR' CMakeCache.txt | cut -d= -f2))"
cmake --build . --target rune_cef_shim --config Release

log "Done. Shim should be in:"
log "  $CEF_ROOT/Debug/librune_cef_shim.dylib (if built Debug)"
log "  $CEF_ROOT/Release/librune_cef_shim.dylib (if built Release)"
