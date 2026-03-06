#!/bin/bash
set -euo pipefail

APP_NAME="Ossue"
REPO="kaplanelad/ossue"

echo "Installing ${APP_NAME}..."

# macOS only
if [ "$(uname)" != "Darwin" ]; then
  echo "Error: This install script is for macOS only."
  echo "For Linux/Windows, download from: https://github.com/${REPO}/releases"
  exit 1
fi

# Detect architecture
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
  DMG_PATTERN="_aarch64.dmg"
elif [ "$ARCH" = "x86_64" ]; then
  DMG_PATTERN="_x64.dmg"
else
  echo "Error: Unsupported architecture: ${ARCH}"
  exit 1
fi

# Get latest release tag
TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
if [ -z "$TAG" ]; then
  echo "Error: Could not determine latest release."
  exit 1
fi
echo "Latest release: ${TAG}"

# Find the DMG asset URL
DMG_URL=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"browser_download_url"' | grep "${DMG_PATTERN}" | sed -E 's/.*"browser_download_url": *"([^"]+)".*/\1/')
if [ -z "$DMG_URL" ]; then
  echo "Error: Could not find DMG for architecture ${ARCH}."
  exit 1
fi

# Download
TMPDIR_PATH=$(mktemp -d)
DMG_PATH="${TMPDIR_PATH}/${APP_NAME}.dmg"
echo "Downloading ${APP_NAME} (${ARCH})..."
curl -fSL --progress-bar -o "$DMG_PATH" "$DMG_URL"

# Mount DMG
echo "Installing..."
MOUNT_POINT=$(hdiutil attach "$DMG_PATH" -nobrowse -noautoopen 2>/dev/null | grep '/Volumes/' | sed 's/.*\/Volumes/\/Volumes/')
if [ -z "$MOUNT_POINT" ]; then
  echo "Error: Failed to mount DMG."
  rm -rf "$TMPDIR_PATH"
  exit 1
fi

# Copy to Applications (remove old version if exists)
if [ -d "/Applications/${APP_NAME}.app" ]; then
  echo "Removing previous installation..."
  rm -rf "/Applications/${APP_NAME}.app"
fi
cp -R "${MOUNT_POINT}/${APP_NAME}.app" /Applications/

# Unmount
hdiutil detach "$MOUNT_POINT" -quiet 2>/dev/null || true

# Clean up
rm -rf "$TMPDIR_PATH"

# Remove quarantine attribute
xattr -cr "/Applications/${APP_NAME}.app" 2>/dev/null || true

echo ""
echo "${APP_NAME} has been installed to /Applications/${APP_NAME}.app"
echo "Opening ${APP_NAME}..."
open "/Applications/${APP_NAME}.app"
