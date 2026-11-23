#!/usr/bin/env bash
# Écrit `latest.json`, le manifeste d'une version, sur la sortie standard.
#
#   tools/release-manifest.sh <dossier> <version> <format> <bundle>
#
# <dossier> contient les cinq fichiers de la version (pas `latest.json`) :
# capri-macos-arm64, capri-hook-macos-arm64, capri-windows-x86_64.exe,
# capri-hook-windows-x86_64.exe et Info.plist. Le script ne touche à rien : il lit, mesure,
# calcule les empreintes SHA-256 et imprime. Aucun réseau, pas de jq.
# Compatible bash 3.2 (celui de macOS) et le bash d'Ubuntu.
set -eu

fail() {
  echo "release-manifest : $*" >&2
  exit 1
}

[ "$#" -eq 4 ] || fail "usage : release-manifest.sh <dossier> <version> <format> <bundle>"
DIR="$1"
VERSION="$2"
FORMAT="$3"
BUNDLE="$4"

[ -d "$DIR" ] || fail "pas un dossier : $DIR"
case "$VERSION" in
  '' | *[!0-9.]* | .* | *. | *..*) fail "version illisible : « $VERSION »" ;;
esac
[ "$(printf '%s' "$VERSION" | tr -cd '.' | wc -c | tr -d ' ')" = "2" ] \
  || fail "la version doit avoir la forme MAJEUR.MINEUR.CORRECTIF : « $VERSION »"
case "$FORMAT" in '' | *[!0-9]*) fail "format illisible : « $FORMAT »" ;; esac
case "$BUNDLE" in '' | *[!0-9]*) fail "bundle illisible : « $BUNDLE »" ;; esac
[ "$FORMAT" = "1" ] || fail "ce script n'écrit que le format 1 : « $FORMAT »"

if command -v sha256sum > /dev/null 2>&1; then
  sha() { sha256sum "$1" | cut -d ' ' -f 1; }
elif command -v shasum > /dev/null 2>&1; then
  sha() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
else
  fail "ni sha256sum ni shasum"
fi

# $1 = nom installé, $2 = fichier de la version ; imprime l'objet JSON, sans virgule finale.
file_entry() {
  name="$1"
  asset="$2"
  for text in "$name" "$asset"; do
    case "$text" in
      '' | *[!A-Za-z0-9._-]*) fail "nom non conforme : « $text »" ;;
    esac
  done
  path="$DIR/$asset"
  { [ -f "$path" ] && [ ! -L "$path" ]; } || fail "fichier manquant (ou pas ordinaire) : $asset"
  size="$(wc -c < "$path" | tr -d ' ')"
  [ "$size" -gt 0 ] || fail "fichier vide : $asset"
  digest="$(sha "$path")"
  case "$digest" in
    *[!0-9a-f]*) fail "empreinte illisible pour $asset" ;;
  esac
  [ "${#digest}" -eq 64 ] || fail "empreinte de longueur inattendue pour $asset"
  printf '        { "name": "%s", "asset": "%s", "size": %s, "sha256": "%s" }' \
    "$name" "$asset" "$size" "$digest"
}

# Tout est construit avant d'imprimer : un refus n'écrit rien sur la sortie.
out="$(
  printf '{\n  "format": %s,\n  "version": "%s",\n  "bundle": %s,\n  "targets": {\n' \
    "$FORMAT" "$VERSION" "$BUNDLE"
  printf '    "macos-arm64": {\n      "files": [\n'
  file_entry capri capri-macos-arm64
  printf ',\n'
  file_entry capri-hook capri-hook-macos-arm64
  printf ',\n'
  file_entry Info.plist Info.plist
  printf '\n      ]\n    },\n    "windows-x86_64": {\n      "files": [\n'
  file_entry capri.exe capri-windows-x86_64.exe
  printf ',\n'
  file_entry capri-hook.exe capri-hook-windows-x86_64.exe
  printf '\n      ]\n    }\n  }\n}\n'
)" || exit 1
printf '%s\n' "$out"
