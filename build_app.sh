#!/bin/bash

# Exit on error
set -e

APP_NAME="mouseless"
EXECUTABLE_NAME="mouseless" # Your cargo binary name

# Determine build mode (default to debug)
BUILD_MODE="debug"
BUNDLE_DIR_BASE="target/debug"
CARGO_BUILD_FLAGS=""

if [ "$1" == "--release" ]; then
  BUILD_MODE="release"
  BUNDLE_DIR_BASE="target/release"
  CARGO_BUILD_FLAGS="--release"
  echo "Building in RELEASE mode..."
else
  echo "Building in DEBUG mode..."
fi

# Build the Rust binary
echo "Building Rust binary (${BUILD_MODE})..."
cargo build ${CARGO_BUILD_FLAGS}

# Define bundle paths
BUNDLE_APP_PATH="${BUNDLE_DIR_BASE}/${APP_NAME}.app"
BUNDLE_CONTENTS_PATH="${BUNDLE_APP_PATH}/Contents"
BUNDLE_MACOS_PATH="${BUNDLE_CONTENTS_PATH}/MacOS"
BUNDLE_RESOURCES_PATH="${BUNDLE_CONTENTS_PATH}/Resources"

# Clean up old bundle
echo "Cleaning up old bundle (if any) at ${BUNDLE_APP_PATH} ..."
rm -rf "${BUNDLE_APP_PATH}"

# Create directory structure
echo "Creating bundle structure..."
mkdir -p "${BUNDLE_MACOS_PATH}"
mkdir -p "${BUNDLE_RESOURCES_PATH}" # For icons etc.

# Copy executable
echo "Copying executable from ${BUNDLE_DIR_BASE}/${EXECUTABLE_NAME} to ${BUNDLE_MACOS_PATH}/${EXECUTABLE_NAME} ..."
cp "${BUNDLE_DIR_BASE}/${EXECUTABLE_NAME}" "${BUNDLE_MACOS_PATH}/${EXECUTABLE_NAME}"

# Copy Info.plist (assuming it's in your project root)
INFO_PLIST_SOURCE="Info.plist" 
if [ ! -f "$INFO_PLIST_SOURCE" ]; then
    echo "Error: Info.plist not found at $INFO_PLIST_SOURCE. Make sure it's in the project root."
    exit 1
fi
echo "Copying Info.plist from ${INFO_PLIST_SOURCE} to ${BUNDLE_CONTENTS_PATH}/Info.plist ..."
cp "${INFO_PLIST_SOURCE}" "${BUNDLE_CONTENTS_PATH}/Info.plist"

# Optional: Copy icon (e.g., AppIcon.icns)
# Create a dummy icon file for now if you want to test this part
# touch AppIcon.icns 
ICON_FILENAME="AppIcon.icns" # Standard macOS icon name
ICON_SOURCE_PATH="AppIcon.icns" # Assuming it's in the project root for this example

if [ -f "$ICON_SOURCE_PATH" ]; then
  echo "Copying icon ${ICON_SOURCE_PATH} to ${BUNDLE_RESOURCES_PATH}/${ICON_FILENAME} ..."
  cp "${ICON_SOURCE_PATH}" "${BUNDLE_RESOURCES_PATH}/${ICON_FILENAME}"
  # Update Info.plist to reference the icon file
  # This requires a tool like PlistBuddy or sed, or ensure your Info.plist already has CFBundleIconFile
  # For simplicity, this script assumes CFBundleIconFile is already in your Info.plist or you add it manually
  # Example if CFBundleIconFile is NOT in Info.plist and you want to add it:
  # defaults write "${BUNDLE_CONTENTS_PATH}/Info.plist" CFBundleIconFile -string "${ICON_FILENAME}"
  # Or using PlistBuddy:
  # /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string ${ICON_FILENAME}" "${BUNDLE_CONTENTS_PATH}/Info.plist"
  echo "Note: Ensure Info.plist references CFBundleIconFile as '${ICON_FILENAME}' for the icon to display."
else
  echo "Icon file not found at ${ICON_SOURCE_PATH}. Skipping icon copy."
fi


echo ""
echo "-----------------------------------------------------"
echo "macOS .app bundle created at: ${BUNDLE_APP_PATH}"
echo "-----------------------------------------------------"
echo "To run the application:"
echo "  open \"${BUNDLE_APP_PATH}\""
echo "Or drag it to your Applications folder and run from there."
echo "Remember to grant Accessibility permissions if needed."
echo "-----------------------------------------------------" 