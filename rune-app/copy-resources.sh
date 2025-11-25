#!/bin/bash
# Copy resources (images, fonts) to the app bundle's Resources folder
#
# This script should be run after building or as a build phase in Xcode.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Determine the target app bundle (check Debug first, then Release)
if [ -d "$SCRIPT_DIR/build/Build/Products/Debug/rune-app.app" ]; then
    APP_BUNDLE="$SCRIPT_DIR/build/Build/Products/Debug/rune-app.app"
elif [ -d "$SCRIPT_DIR/build/Build/Products/Release/rune-app.app" ]; then
    APP_BUNDLE="$SCRIPT_DIR/build/Build/Products/Release/rune-app.app"
else
    # Fall back to checking BUILT_PRODUCTS_DIR from Xcode environment
    if [ -n "$BUILT_PRODUCTS_DIR" ] && [ -d "$BUILT_PRODUCTS_DIR/rune-app.app" ]; then
        APP_BUNDLE="$BUILT_PRODUCTS_DIR/rune-app.app"
    else
        echo "ERROR: Could not find rune-app.app bundle"
        echo "Run this script after building in Xcode"
        exit 1
    fi
fi

RESOURCES_DIR="$APP_BUNDLE/Contents/Resources"

echo "Copying resources to: $RESOURCES_DIR"

# Create directories
mkdir -p "$RESOURCES_DIR/images"
mkdir -p "$RESOURCES_DIR/fonts"

# Copy images
rsync -av "$PROJECT_ROOT/images/" "$RESOURCES_DIR/images/"

# Copy fonts
rsync -av "$PROJECT_ROOT/fonts/" "$RESOURCES_DIR/fonts/"

echo "Resources copied successfully!"
