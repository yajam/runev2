# Copyright (c) The Chromium Embedded Framework Authors.
# Shim integration for rune-cef.

cmake_minimum_required(VERSION 3.21)

if(NOT OS_MAC)
  # The shim currently only targets macOS.
  return()
endif()

set(SHIM_TARGET rune_cef_shim)

add_library(${SHIM_TARGET} SHARED
  rune_cef_shim.cc
)

# Use the same compiler/linker settings as other CEF libraries.
SET_LIBRARY_TARGET_PROPERTIES(${SHIM_TARGET})

# Link against the CEF C++ wrapper and standard libs.
add_dependencies(${SHIM_TARGET} libcef_dll_wrapper)
target_link_libraries(${SHIM_TARGET} PRIVATE libcef_dll_wrapper ${CEF_STANDARD_LIBS})

# Copy the built dylib next to the CEF framework (Debug/Release).
add_custom_command(
  TARGET ${SHIM_TARGET}
  POST_BUILD
  COMMAND ${CMAKE_COMMAND} -E copy_if_different
          "$<TARGET_FILE:${SHIM_TARGET}>"
          "${CEF_BINARY_DIR_DEBUG}/$<TARGET_FILE_NAME:${SHIM_TARGET}>"
  COMMAND ${CMAKE_COMMAND} -E copy_if_different
          "$<TARGET_FILE:${SHIM_TARGET}>"
          "${CEF_BINARY_DIR_RELEASE}/$<TARGET_FILE_NAME:${SHIM_TARGET}>"
  VERBATIM
)

