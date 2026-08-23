#!/usr/bin/env bash
# Package the macOS desktop app into a signed .app bundle and a DMG, plus a
# CLI-only tarball.
#
# Requires a macOS host: the script uses `sips`, `iconutil`, `codesign` and
# `hdiutil`, all of which are present on GitHub's macOS runners and a stock
# macOS install.
#
# Usage: package-macos.sh <version> <target-triple>
set -euo pipefail

VERSION="${1:?usage: package-macos.sh <version> <target-triple>}"
TARGET="${2:?usage: package-macos.sh <version> <target-triple>}"

BIN_DIR="target/${TARGET}/release"
DIST="dist"
APP_NAME="DeepMate"
APP="${APP_NAME}.app"
BUNDLE_ID="dev.deepmate.app"
DMG_NAME="deepmate-${VERSION}-${TARGET}"
CLI_NAME="deepmate-${VERSION}-${TARGET}"

# --- 1. Assemble the .app bundle -------------------------------------------
APP_ROOT="${DIST}/${APP}"
rm -rf "${APP_ROOT}"
mkdir -p "${APP_ROOT}/Contents/MacOS" "${APP_ROOT}/Contents/Resources"

cp "${BIN_DIR}/deepmate-desktop" "${APP_ROOT}/Contents/MacOS/deepmate-desktop"
chmod +x "${APP_ROOT}/Contents/MacOS/deepmate-desktop"

# --- 2. Info.plist ----------------------------------------------------------
# LSUIElement keeps the menu-bar app out of the Dock; the tray icon is the
# only surface when the control-center window is closed.
cat > "${APP_ROOT}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleExecutable</key>
    <string>deepmate-desktop</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

# --- 3. Icon: logo.png -> .icns --------------------------------------------
ICONSET="${DIST}/AppIcon.iconset"
rm -rf "${ICONSET}"
mkdir -p "${ICONSET}"
SOURCE="logo/logo.png"
for size in 16 32 128 256 512; do
    sips -z "${size}" "${size}" "${SOURCE}" \
        --out "${ICONSET}/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" "${SOURCE}" \
        --out "${ICONSET}/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "${ICONSET}" -o "${APP_ROOT}/Contents/Resources/AppIcon.icns"
rm -rf "${ICONSET}"

# --- 4. Ad-hoc code sign ----------------------------------------------------
# `-` signs with an ad-hoc identity: the app runs locally without a Developer
# ID, but first launch from a download still needs a right-click "Open" to
# clear the Gatekeeper quarantine.
codesign --force --deep --sign - "${APP_ROOT}"

# --- 5. Build the DMG -------------------------------------------------------
DMG_STAGING="${DIST}/dmg-staging"
rm -rf "${DMG_STAGING}"
mkdir -p "${DMG_STAGING}"
cp -R "${APP_ROOT}" "${DMG_STAGING}/"
ln -s /Applications "${DMG_STAGING}/Applications"
# `hdiutil create` is deprecated on recent macOS in favour of `diskutil image
# create from`, but the newer form's `--volumeName` flag is not available on
# the older macOS runners (macos-15-intel). hdiutil stays compatible across
# all supported macOS versions, so it is the right tool for CI.
hdiutil create \
    -volname "${APP_NAME}" \
    -srcfolder "${DMG_STAGING}" \
    -ov \
    -format UDZO \
    "${DIST}/${DMG_NAME}.dmg"
rm -rf "${DMG_STAGING}"

# --- 6. CLI-only tarball ----------------------------------------------------
CLI_DIR="${DIST}/${CLI_NAME}"
rm -rf "${CLI_DIR}"
mkdir -p "${CLI_DIR}"
cp "${BIN_DIR}/deepmate" "${CLI_DIR}/"
cp README.md LICENSE-MIT LICENSE-APACHE "${CLI_DIR}/"
tar -C "${DIST}" -czf "${DIST}/${CLI_NAME}.tar.gz" "${CLI_NAME}"
( cd "${DIST}" && shasum -a 256 "${CLI_NAME}.tar.gz" > "${CLI_NAME}.tar.gz.sha256" )
rm -rf "${CLI_DIR}"

# --- 7. DMG checksum --------------------------------------------------------
( cd "${DIST}" && shasum -a 256 "${DMG_NAME}.dmg" > "${DMG_NAME}.dmg.sha256" )

echo "Packaged ${DIST}/${DMG_NAME}.dmg and ${DIST}/${CLI_NAME}.tar.gz"
