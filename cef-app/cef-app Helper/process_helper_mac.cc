// Copyright (c) 2013 The Chromium Embedded Framework Authors. All rights
// reserved. Use of this source code is governed by a BSD-style license that can
// be found in the LICENSE file.

#include "include/cef_app.h"
#include "include/wrapper/cef_library_loader.h"
#include "include/cef_sandbox_mac.h"

// Entry point function for sub-processes.
int main(int argc, char* argv[]) {
  // Sandbox disabled - library not available in this distribution
  // CefScopedSandboxContext sandbox_context;
  // if (!sandbox_context.Initialize(argc, argv))
  //   return 1;

  // Load the CEF framework library at runtime instead of linking directly
  // as required by the macOS sandbox implementation.
  CefScopedLibraryLoader library_loader;
  if (!library_loader.LoadInHelper())
    return 1;

  // Provide CEF with command-line arguments.
  CefMainArgs main_args(argc, argv);

  // Execute the sub-process.
  return CefExecuteProcess(main_args, nullptr, nullptr);
}
