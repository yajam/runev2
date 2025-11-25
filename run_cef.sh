#!/usr/bin/env bash
set -euo pipefail

# Run demo-app with CEF on macOS, wiring in the framework and helpers.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROFILE="${PROFILE:-debug}"
URL="${1:-https://example.com}"

CEF_PATH="${CEF_PATH:-"$ROOT/cef/Release"}"
CEF_HELPER_PATH="${CEF_HELPER_PATH:-"$ROOT/cef/build/tests/cefsimple/Release/cefsimple Helper.app/Contents/MacOS/cefsimple Helper"}"

CEF_LIB_DIR="$CEF_PATH/Chromium Embedded Framework.framework/Libraries"
RUNE_CEF_SHIM_PATH="${RUNE_CEF_SHIM_PATH:-"$CEF_PATH"}"

patch_helper_app() {
  local helper_app="$1"
  local macos_dir="$helper_app/Contents/MacOS"
  local framework_dir="$ROOT/cef/build/tests/cefsimple/Release/Chromium Embedded Framework.framework"

  # Ensure framework is visible relative to helper.
  if [[ -d "$CEF_PATH/Chromium Embedded Framework.framework" && ! -e "$framework_dir" ]]; then
    ln -sf "$CEF_PATH/Chromium Embedded Framework.framework" "$framework_dir"
  fi

  # Symlink core libs next to the helper executable.
  if [[ -d "$CEF_LIB_DIR" && -d "$macos_dir" ]]; then
    for lib in libEGL.dylib libGLESv2.dylib libcef_sandbox.dylib libvk_swiftshader.dylib; do
      if [[ -f "$CEF_LIB_DIR/$lib" && ! -e "$macos_dir/$lib" ]]; then
        ln -sf "$CEF_LIB_DIR/$lib" "$macos_dir/$lib"
      fi
    done
  fi
}

patch_helper_app "$(dirname "$(dirname "$CEF_HELPER_PATH")")"
patch_helper_app "$ROOT/cef/build/tests/cefsimple/Release/cefsimple Helper (GPU).app"
patch_helper_app "$ROOT/cef/build/tests/cefsimple/Release/cefsimple Helper (Renderer).app"
patch_helper_app "$ROOT/cef/build/tests/cefsimple/Release/cefsimple Helper (Plugin).app"
patch_helper_app "$ROOT/cef/build/tests/cefsimple/Release/cefsimple Helper (Alerts).app"

export CEF_PATH
export CEF_HELPER_PATH
export RUNE_CEF_SHIM_PATH
export DYLD_FRAMEWORK_PATH="$CEF_PATH"
export DYLD_LIBRARY_PATH="$CEF_LIB_DIR"

BIN="$ROOT/target/$PROFILE/demo-app"
if [[ "$PROFILE" == "release" ]]; then
  cargo build -p demo-app --features cef --release
else
  cargo build -p demo-app --features cef
fi

CEF_SWITCHES="${CEF_SWITCHES:---ignore-certificate-errors --disable-gpu --use-mock-keychain}"

echo "Running demo-app with CEF"
echo "  URL: $URL"
echo "  CEF_PATH: $CEF_PATH"
echo "  CEF_HELPER_PATH: $CEF_HELPER_PATH"
echo "  RUNE_CEF_SHIM_PATH: $RUNE_CEF_SHIM_PATH"
echo "  CEF_SWITCHES: $CEF_SWITCHES"

exec "$BIN" --scene=cef --cef-url="$URL" $CEF_SWITCHES
