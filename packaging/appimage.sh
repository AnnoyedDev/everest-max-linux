#!/bin/sh
set -e
cd "$(dirname "$0")/.."
(cd rust && cargo build --release)
app=build/AppDir
rm -rf "$app"
mkdir -p "$app/usr/bin"
cp rust/target/release/everest "$app/usr/bin/"
cp packaging/everest-basecamp.desktop rust/assets/everest-basecamp.svg "$app/"
ln -s usr/bin/everest "$app/AppRun"
ARCH=x86_64 "${APPIMAGETOOL:-appimagetool}" "$app" "everest-max-linux-$(rust/target/release/everest version)-x86_64.AppImage"
