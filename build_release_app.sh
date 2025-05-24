#!/bin/bash

# Exit on error
set -e

APP_NAME="Mouseless" # Desired app name
EXECUTABLE_NAME="mouseless" # Your cargo binary name

# --- This script is specifically for RELEASE builds ---
BUILD_MODE="release"
BUNDLE_DIR_BASE="target/release"
CARGO_BUILD_FLAGS="--release"
echo "Building in RELEASE mode for ${APP_NAME}.app..."

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
mkdir -p "${BUNDLE_RESOURCES_PATH}"

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

# Copy icon (AppIcon.icns)
ICON_FILENAME="AppIcon.icns" # Standard macOS icon name
ICON_SOURCE_PATH="AppIcon.icns" # Assuming it's in the project root

if [ -f "$ICON_SOURCE_PATH" ]; then
  echo "Copying icon ${ICON_SOURCE_PATH} to ${BUNDLE_RESOURCES_PATH}/${ICON_FILENAME} ..."
  cp "${ICON_SOURCE_PATH}" "${BUNDLE_RESOURCES_PATH}/${ICON_FILENAME}"
  # Ensure your Info.plist has CFBundleIconFile set to AppIcon.icns
  echo "Note: Ensure Info.plist references CFBundleIconFile as '${ICON_FILENAME}' for the icon to display."
else
  echo "Icon file not found at ${ICON_SOURCE_PATH} (${PWD}/${ICON_SOURCE_PATH}). Skipping icon copy."
  echo "To add an icon, create AppIcon.icns in your project root."
fi

# --- Codesigning (do this only once after all files are in place) ---
# IMPORTANT: Replace "YOUR_DEVELOPER_ID_APPLICATION_IDENTITY" with your actual
#            Developer ID Application certificate name from Keychain Access.
#            (e.g., "Developer ID Application: Your Name (TEAMID)")
#            This requires a paid Apple Developer Program membership and correct setup.
DEVELOPER_ID_IDENTITY="YOUR_DEVELOPER_ID_APPLICATION_IDENTITY" # <<< CHANGE THIS!

if [ "$DEVELOPER_ID_IDENTITY" == "YOUR_DEVELOPER_ID_APPLICATION_IDENTITY" ]; then
  echo ""
  echo "--------------------------------------------------------------------------"
  echo "WARNING: Codesigning identity is not set. Please edit this script"
  echo "         and replace 'YOUR_DEVELOPER_ID_APPLICATION_IDENTITY'"
  echo "         with your actual Apple Developer ID Application identity."
  echo "         Skipping codesigning."
  echo "         Your app will not be notarized and may not run on other Macs"
  echo "         without right-clicking and choosing 'Open'."
  echo "--------------------------------------------------------------------------"
else
  echo ""
  echo "Codesigning ${BUNDLE_APP_PATH}..."
  codesign --force --deep --sign "${DEVELOPER_ID_IDENTITY}" --timestamp --options runtime "${BUNDLE_APP_PATH}"
  echo "Codesigning complete."
  echo "Verifying codesignature..."
  codesign --verify --verbose=4 "${BUNDLE_APP_PATH}"
  spctl --assess --type execute --verbose "${BUNDLE_APP_PATH}"
fi

echo ""
echo "-----------------------------------------------------"
echo "macOS .app bundle created at: ${BUNDLE_APP_PATH}"
echo "-----------------------------------------------------"
echo "To run the application:"
echo "  open \"${BUNDLE_APP_PATH}\""
echo "Or drag it to your Applications folder and run from there."
echo "If not codesigned properly, you might need to right-click -> Open."
echo "Remember to grant Accessibility permissions if needed."
echo "-----------------------------------------------------" 