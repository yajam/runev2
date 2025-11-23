#!/usr/bin/env bash
set -euo pipefail

# Bundle the demo-app binary into a macOS .app with the CEF framework/helpers.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${PROFILE:-debug}"
TARGET_DIR="$ROOT/target/$PROFILE"
APP_NAME="demo-app"
APP_DIR="$TARGET_DIR/${APP_NAME}.app"
BINARY="$TARGET_DIR/$APP_NAME"

log() {
  printf '[bundle] %s\n' "$1"
}

build_binary() {
  if [[ -x "$BINARY" ]]; then
    return
  fi
  log "Building $APP_NAME ($PROFILE)"
  if [[ "$PROFILE" == "release" ]]; then
    cargo build -p demo-app --features cef --release
  else
    cargo build -p demo-app --features cef
  fi
}

write_main_plist() {
  cat >"$APP_DIR/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>demo-app</string>
  <key>CFBundleIdentifier</key>
  <string>com.rune.demo-app</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>demo-app</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSEnvironment</key>
  <dict>
    <key>MallocNanoZone</key>
    <string>0</string>
    <key>RUNE_CEF_SHIM_PATH</key>
    <string>@executable_path/../Frameworks</string>
  </dict>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSPrincipalClass</key>
  <string>NSApplication</string>
  <key>NSSupportsAutomaticGraphicsSwitching</key>
  <true/>
</dict>
</plist>
EOF
}

write_helper_plist() {
  local dest="$1"
  local exec_name="$2"
  local display_name="$3"
  local bundle_id="$4"
  cat >"$dest/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>${display_name}</string>
  <key>CFBundleExecutable</key>
  <string>${exec_name}</string>
  <key>CFBundleIdentifier</key>
  <string>${bundle_id}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${display_name}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0</string>
  <key>CFBundleSignature</key>
  <string>????</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSEnvironment</key>
  <dict>
    <key>MallocNanoZone</key>
    <string>0</string>
  </dict>
  <key>LSFileQuarantineEnabled</key>
  <true/>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>LSUIElement</key>
  <string>1</string>
  <key>NSSupportsAutomaticGraphicsSwitching</key>
  <true/>
</dict>
</plist>
EOF
}

copy_helpers() {
  local cef_root="$ROOT/cef/build/tests/cefsimple/Release"
  local variants=("" " (GPU)" " (Renderer)" " (Plugin)" " (Alerts)")
  for suffix in "${variants[@]}"; do
    local src="$cef_root/cefsimple Helper${suffix}.app"
    local dest="$APP_DIR/Contents/Frameworks/demo-app Helper${suffix}.app"
    if [[ ! -d "$src" ]]; then
      log "Skipping missing helper: $src"
      continue
    fi

    log "Bundling helper${suffix:- (main)}"
    rm -rf "$dest"
    mkdir -p "$dest/Contents/MacOS"

    local exec_src="$src/Contents/MacOS/cefsimple Helper${suffix}"
    local exec_dest="$dest/Contents/MacOS/demo-app Helper${suffix}"
    cp "$exec_src" "$exec_dest"
    chmod +x "$exec_dest"

    local bundle_id="com.rune.demo-app.helper"
    case "$suffix" in
      " (GPU)") bundle_id="${bundle_id}.gpu" ;;
      " (Renderer)") bundle_id="${bundle_id}.renderer" ;;
      " (Plugin)") bundle_id="${bundle_id}.plugin" ;;
      " (Alerts)") bundle_id="${bundle_id}.alerts" ;;
    esac

    write_helper_plist "$dest" "demo-app Helper${suffix}" "demo-app Helper${suffix}" "$bundle_id"
  done
}

copy_resources() {
  local resources_src="$ROOT/cef/build/tests/cefsimple/Release/cefsimple.app/Contents/Resources"
  if [[ -d "$resources_src" ]]; then
    log "Copying Resources/"
    rsync -a "$resources_src/" "$APP_DIR/Contents/Resources/"
  fi
}

copy_framework() {
  local framework_src="$ROOT/cef/Release/Chromium Embedded Framework.framework"
  if [[ ! -d "$framework_src" ]]; then
    echo "CEF framework not found at $framework_src" >&2
    exit 1
  fi
  log "Copying Chromium Embedded Framework.framework"
  rsync -a "$framework_src" "$APP_DIR/Contents/Frameworks/"
  # Also copy the Libraries next to the helper executables so dyld can find them.
  local lib_src="$framework_src/Libraries"
  if [[ -d "$lib_src" ]]; then
    for helper in "$APP_DIR"/Contents/Frameworks/demo-app\ Helper*.app; do
      [[ -d "$helper" ]] || continue
      local macos_dir="$helper/Contents/MacOS"
      mkdir -p "$macos_dir"
      for lib in libEGL.dylib libGLESv2.dylib libcef_sandbox.dylib libvk_swiftshader.dylib; do
        if [[ -f "$lib_src/$lib" ]]; then
          ln -sf "$lib_src/$lib" "$macos_dir/$lib"
        fi
      done
    done
  fi
}

copy_shim() {
  local shim_src="$ROOT/cef/Release/librune_cef_shim.dylib"
  if [[ ! -f "$shim_src" ]]; then
    log "Shim library not found at $shim_src (build cef/rune_cef_shim first)"
    return
  fi
  log "Copying librune_cef_shim.dylib"
  cp "$shim_src" "$APP_DIR/Contents/Frameworks/"
}

main() {
  build_binary

  log "Creating bundle at $APP_DIR"
  rm -rf "$APP_DIR"
  mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Frameworks" "$APP_DIR/Contents/Resources"

  write_main_plist
  cp "$BINARY" "$APP_DIR/Contents/MacOS/$APP_NAME"
  chmod +x "$APP_DIR/Contents/MacOS/$APP_NAME"

  copy_resources
  copy_framework
  copy_shim
  copy_helpers

  # Ad-hoc sign the entire bundle (main app, helpers, and nested frameworks).
  if command -v codesign >/dev/null 2>&1; then
    log "Codesigning bundle (ad-hoc)"
    codesign --force --deep --sign - "$APP_DIR"
    log "Verifying signature"
    codesign --verify --deep --strict --verbose=2 "$APP_DIR"
  else
    log "codesign not found; skipping signing"
  fi

  log "Bundle ready: $APP_DIR"
}

main "$@"
