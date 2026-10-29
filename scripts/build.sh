#!/bin/sh
set -eu
PROJECT_ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
"$PROJECT_ROOT/scripts/cargo.sh" build --release --locked
APP="$PROJECT_ROOT/dist/Ministat.app"
mkdir -p "$APP/Contents/MacOS"
cp "$PROJECT_ROOT/target/release/ministat" "$APP/Contents/MacOS/ministat"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>ministat</string>
<key>CFBundleIdentifier</key><string>com.albovsky.ministat</string>
<key>CFBundleName</key><string>Ministat</string>
<key>CFBundleDisplayName</key><string>Ministat</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>LSUIElement</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
/usr/bin/xattr -cr "$APP"
/usr/bin/codesign --force --sign - --timestamp=none "$APP"
/usr/bin/codesign --verify --strict "$APP"
printf 'Built %s\n' "$APP"
