#!/usr/bin/env bash
# Fabrique target/Capri.app : la même Capri, dans une app que macOS sait reconnaître.
#
# C'est nécessaire pour lui parler au micro : le système n'accorde le micro et la reconnaissance
# vocale qu'à une app dont l'Info.plist dit pourquoi elle les veut. Le binaire lancé nu depuis un
# terminal garde tout le reste, mais ne t'écoute pas.
#
# L'app n'embarque aucune image : elle les lit dans assets/capri/ de ce dépôt, comme `cargo run`
# (voir tools/fetch-assets.sh). Elle est signée « ad hoc », pour cette machine seulement. macOS
# rattache les permissions à cette signature : après chaque reconstruction, il redemande le
# micro et la reconnaissance vocale.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$ROOT/target/Capri.app"
cd "$ROOT"

cargo build --release -p capri-app -p capri-hook

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp target/release/capri target/release/capri-hook "$APP/Contents/MacOS/"
sed "s/@VERSION@/${VERSION:-0.0.0}/g" tools/macos/Info.plist > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" > /dev/null

codesign --force --sign - "$APP/Contents/MacOS/capri-hook"
codesign --force --sign - "$APP"
codesign --verify --strict "$APP"

echo "Capri.app est prête : $APP"
echo "Pour la lancer : open \"$APP\"   (quitte d'abord une Capri déjà lancée)"
