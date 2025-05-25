#!/bin/bash

# Exit on error
set -e

APP_NAME="Mouseless.app" # This should match the .app folder name from your build script
SOURCE_APP_DIR_RELATIVE="target/release" # Relative to project root
DEST_APP_ROOT="/Applications"

# Determine Project Root (assuming this script is in the project root)
# SCRIPT_DIR should resolve to the directory where this script itself is located.
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
PROJECT_ROOT="${SCRIPT_DIR}"

# Construct full paths
SOURCE_APP_PATH="${PROJECT_ROOT}/${SOURCE_APP_DIR_RELATIVE}/${APP_NAME}"
DEST_APP_PATH="${DEST_APP_ROOT}/${APP_NAME}"

# 1. Check if source app exists
if [ ! -d "${SOURCE_APP_PATH}" ]; then
    echo "Error: Source application not found at ${SOURCE_APP_PATH}"
    echo "       (Expected at ./${SOURCE_APP_DIR_RELATIVE}/${APP_NAME} relative to script location)"
    echo "Please ensure you have built the release version (e.g., run ./build_release_app.sh)."
    exit 1
fi

echo "Source application found: ${SOURCE_APP_PATH}"

# Inform about potential sudo need upfront
PERMISSION_ERROR_ADVICE="If you see 'Permission denied' errors, try running this script with 'sudo': sudo $0"
NEEDS_SUDO_HINT=false

# Check write permissions for the destination
# If destination exists, check writability of the existing app bundle itself for rm.
# If destination does not exist, check writability of /Applications for cp.
if [ -d "${DEST_APP_PATH}" ]; then
    PARENT_OF_DEST_APP_PATH=$(dirname "${DEST_APP_PATH}")
    if [ ! -w "${DEST_APP_PATH}" ] && [ ! -w "${PARENT_OF_DEST_APP_PATH}" ] && [ "$(id -u)" != "0" ]; then
        NEEDS_SUDO_HINT=true
    fi
elif [ ! -w "${DEST_APP_ROOT}" ] &&  [ "$(id -u)" != "0" ]; then
    NEEDS_SUDO_HINT=true
fi

if ${NEEDS_SUDO_HINT}; then
    echo "--------------------------------------------------------------------------"
    echo "INFO: You might need to run this script with 'sudo' to update/install"
    echo "      the application in ${DEST_APP_ROOT}."
    echo "--------------------------------------------------------------------------"
fi

# 2. Check if destination app exists
if [ -d "${DEST_APP_PATH}" ]; then
    echo "Application '${APP_NAME}' already exists at: ${DEST_APP_PATH}"
    read -p "Do you want to overwrite it with the new build from '${SOURCE_APP_DIR_RELATIVE}'? (y/N): " choice
    case "$choice" in
      y|Y )
        echo "Proceeding with overwrite..."
        echo "Attempting to remove old version at ${DEST_APP_PATH}..."
        rm -rf "${DEST_APP_PATH}" # This might require sudo
        echo "Attempting to copy new version from ${SOURCE_APP_PATH} to ${DEST_APP_PATH}..."
        cp -R "${SOURCE_APP_PATH}" "${DEST_APP_PATH}" # This might require sudo
        echo "Application updated successfully at ${DEST_APP_PATH}"
        ;;
      * )
        echo "Update cancelled by user."
        exit 0
        ;;
    esac
else
    # 3. If destination doesn't exist, just copy
    echo "No existing application found at ${DEST_APP_PATH}."
    echo "Attempting to copy new version from ${SOURCE_APP_PATH} to ${DEST_APP_PATH}..."
    cp -R "${SOURCE_APP_PATH}" "${DEST_APP_PATH}" # This might require sudo
    echo "Application installed successfully at ${DEST_APP_PATH}"
fi

echo ""
echo "-----------------------------------------------------"
echo "Update/Installation complete."
echo "If '${APP_NAME}' was running, please quit and restart it for changes to take effect."
echo "Remember to grant Accessibility permissions if needed for the new/updated app."
if ${NEEDS_SUDO_HINT}; then
    echo "$PERMISSION_ERROR_ADVICE"
fi
echo "-----------------------------------------------------"

exit 0 