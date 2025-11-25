# CEF Test Setup Guide

This guide documents how to set up and build the `cef-test` project for CEF offscreen rendering with Metal on macOS.

## Overview

The `cef-test` project is a proof-of-concept combining CEF (Chromium Embedded Framework) offscreen rendering with Metal API. It creates a window with an animated background and a CEF browser texture overlay.

## Prerequisites

- macOS 12.0 or later
- Xcode 13.5 to 16.4 (or newer)
- CMake 3.21 or newer

## Setup Instructions

### 1. Download CEF Binary Distribution

Download the CEF Standard Distribution for macOS from:
- https://cef-builds.spotifycdn.com/index.html
- Select your architecture (ARM64 for Apple Silicon, x64 for Intel)
- Choose "Standard Distribution"

### 2. Extract CEF to the `cef` Folder

Extract the downloaded archive into `cef-test/cef/`:

```
cef-test/cef/
├── CMakeLists.txt
├── Debug/
│   └── Chromium Embedded Framework.framework/
├── Release/
│   └── Chromium Embedded Framework.framework/
├── README.txt
├── cmake/
├── include/
├── libcef_dll/
└── tests/
```

### 3. Convert Framework to Versioned Structure (Xcode 26+)

Newer Xcode versions require a versioned framework structure. Run these commands for both Debug and Release:

```bash
cd cef-test/cef/Debug
FRAMEWORK_DIR="Chromium Embedded Framework.framework"

# Create Versions/A structure
mkdir -p "${FRAMEWORK_DIR}/Versions/A"

# Move contents to Versions/A
mv "${FRAMEWORK_DIR}/Chromium Embedded Framework" "${FRAMEWORK_DIR}/Versions/A/"
mv "${FRAMEWORK_DIR}/Libraries" "${FRAMEWORK_DIR}/Versions/A/"
mv "${FRAMEWORK_DIR}/Resources" "${FRAMEWORK_DIR}/Versions/A/"

# Create symlinks
cd "${FRAMEWORK_DIR}/Versions" && ln -sf A Current
cd ..
ln -sf Versions/Current/Chromium\ Embedded\ Framework "Chromium Embedded Framework"
ln -sf Versions/Current/Libraries Libraries
ln -sf Versions/Current/Resources Resources
```

Repeat for `cef-test/cef/Release/`.

### 4. Generate CMake Build Files

Create a build folder and generate the Xcode project:

```bash
cd cef-test/cef
mkdir build && cd build

# For ARM64 (Apple Silicon)
cmake -G "Xcode" -DPROJECT_ARCH="arm64" ..

# For x86_64 (Intel)
cmake -G "Xcode" -DPROJECT_ARCH="x86_64" ..
```

This creates `cef-test/cef/build/cef.xcodeproj`.

### 5. Update Testapp.xcodeproj Settings

The following settings need to be updated in `Testapp.xcodeproj` for compatibility with newer CEF versions:

#### C++ Language Standard
Change from `gnu++14` to `gnu++17`:
```
CLANG_CXX_LANGUAGE_STANDARD = "gnu++17";
```

#### macOS Deployment Target
Update to match CEF requirements (12.0 for CEF 142.x):
```
MACOSX_DEPLOYMENT_TARGET = 12.0;
```

#### Library Search Paths
Add the path to `libcef_dll_wrapper.a`:
```
LIBRARY_SEARCH_PATHS = (
    "$(PROJECT_DIR)/cef/Debug",
    "$(PROJECT_DIR)/cef/build/libcef_dll_wrapper/Debug",
);
```

#### Remove cef_sandbox.a (if not present)
If your CEF distribution doesn't include `cef_sandbox.a`, remove all references to it from the Xcode project.

### 6. Disable Sandbox (if sandbox library not available)

If your CEF distribution doesn't include sandbox support, disable it in the code:

**In `testapp/main.mm`:**
```cpp
CefSettings cefSettings;
cefSettings.windowless_rendering_enabled = true;
cefSettings.no_sandbox = true;  // Add this line
```

**In `testapp Helper/process_helper_mac.cc`:**
```cpp
int main(int argc, char* argv[]) {
  // Comment out sandbox initialization
  // CefScopedSandboxContext sandbox_context;
  // if (!sandbox_context.Initialize(argc, argv))
  //   return 1;

  CefScopedLibraryLoader library_loader;
  if (!library_loader.LoadInHelper())
    return 1;
  // ... rest of code
}
```

### 7. Build and Run

1. Open `Testapp.xcodeproj` in Xcode
2. Select the `testapp` scheme
3. Select Debug configuration
4. Build (⌘B) and Run (⌘R)

Or build from command line:
```bash
cd cef-test
xcodebuild -project Testapp.xcodeproj -scheme testapp -configuration Debug build
```

Run the built app:
```bash
open ~/Library/Developer/Xcode/DerivedData/Testapp-*/Build/Products/Debug/testapp.app
```

## Troubleshooting

### Error: `No type named 'in_place_t' in namespace 'std'`
**Solution:** Update C++ standard to `gnu++17`

### Error: `ld: library not found for -lcef_sandbox`
**Solution:** Remove `cef_sandbox.a` references from Xcode project

### Error: `Framework did not contain an Info.plist`
**Solution:** Convert framework to versioned structure (Step 3)

### Error: `dlopen libcef_sandbox.dylib: no such file`
**Solution:** Disable sandbox in code (Step 6)

### Error: `Failed to load the CEF framework`
**Solution:** Ensure framework is properly placed in app bundle's Frameworks directory with correct versioned structure.

If running from Xcode, ensure the build script creates a symlink at the build products level:
```bash
ln -sf "$EXECUTABLE_NAME.app/Contents/Frameworks/Chromium Embedded Framework.framework" "$BUILT_PRODUCTS_DIR/Chromium Embedded Framework.framework"
```
This symlink is needed because the standalone helper apps at the Debug root look for the framework relative to their location.

## CEF Version Compatibility

This guide was tested with:
- CEF 142.0.15 (Chromium 142.0.7444.176)
- Xcode 26 (17A400)
- macOS 15.x

Different CEF versions may have different requirements. Check the `README.txt` in your CEF distribution for version-specific instructions.

## App Bundle Structure

The final app bundle should have this structure:
```
testapp.app/
└── Contents/
    ├── Frameworks/
    │   ├── Chromium Embedded Framework.framework/
    │   │   ├── Chromium Embedded Framework -> Versions/Current/...
    │   │   ├── Libraries -> Versions/Current/Libraries
    │   │   ├── Resources -> Versions/Current/Resources
    │   │   └── Versions/
    │   │       ├── A/
    │   │       │   ├── Chromium Embedded Framework
    │   │       │   ├── Libraries/
    │   │       │   └── Resources/
    │   │       └── Current -> A
    │   ├── testapp Helper.app/
    │   ├── testapp Helper (GPU).app/
    │   ├── testapp Helper (Plugin).app/
    │   └── testapp Helper (Renderer).app/
    ├── MacOS/
    │   └── testapp
    └── Resources/
```
