#!/bin/bash
# Build script for rune-ffi Rust static library
#
# This script builds the Rust rune-ffi crate as a static library
# and copies it to rune-app/lib/ for linking in Xcode.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LIB_DIR="$SCRIPT_DIR/lib"

echo "Building rune-ffi Rust library..."
echo "Project root: $PROJECT_ROOT"

# Build the Rust library
cd "$PROJECT_ROOT"

# Determine build mode from Xcode environment
if [ "$CONFIGURATION" = "Debug" ]; then
    CARGO_PROFILE="debug"
    cargo build -p rune-ffi
else
    CARGO_PROFILE="release"

    # Favor smaller static libs for Xcode consumers.
    # These env vars override the release profile without touching Cargo.toml.
    export CARGO_PROFILE_RELEASE_LTO=false
    export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
    export CARGO_PROFILE_RELEASE_OPT_LEVEL=z
    export CARGO_PROFILE_RELEASE_PANIC=abort
    export CARGO_PROFILE_RELEASE_STRIP=symbols

    cargo build -p rune-ffi --release
fi

# Create lib directory if needed
mkdir -p "$LIB_DIR"

# Copy the built library
LIB_PATH="$PROJECT_ROOT/target/$CARGO_PROFILE/librune_ffi.a"

if [ -f "$LIB_PATH" ]; then
    cp "$LIB_PATH" "$LIB_DIR/"

    # Strip debug info from the copied archive to keep size low.
    if command -v strip >/dev/null 2>&1; then
        strip -x "$LIB_DIR/librune_ffi.a" 2>/dev/null || strip -S "$LIB_DIR/librune_ffi.a" || true
    fi

    echo "Copied $LIB_PATH to $LIB_DIR/"
    echo "Library size: $(ls -lh "$LIB_DIR/librune_ffi.a" | awk '{print $5}')"
else
    echo "ERROR: Library not found at $LIB_PATH"
    echo "Checking for library in target directory..."
    find "$PROJECT_ROOT/target" -name "librune_ffi.a" 2>/dev/null || echo "No library found"
    exit 1
fi

echo "Build complete!"
