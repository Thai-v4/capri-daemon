#!/usr/bin/env bash
# Fabrique target/Capri.app : la même Capri, dans une app que macOS sait reconnaître, et qui
# emporte ses images.
#
# C'est nécessaire pour lui parler au micro : le système n'accorde le micro et la reconnaissance
# vocale qu'à une app dont l'Info.plist dit pourquoi elle les veut. Le binaire lancé nu depuis un
# terminal garde tout le reste, mais ne t'écoute pas.
#
# Les images : l'app emporte celles que lit le code, et seulement elles (la liste IMAGES
# ci-dessous ; un essai de capri-app vérifie qu'elle suit le code), dans
# Contents/Resources/assets/capri/, là où Capri les cherche d'abord (après CAPRI_ASSETS). Elles
# sont prises dans assets/ de ce dépôt, ou dans le dossier que désigne CAPRI_ASSETS (celui qui
# contient capri/, comme pour l'app). Ce script ne télécharge rien : s'il en manque une, il
# s'arrête et dit quoi lancer (tools/fetch-assets.sh). CREDITS.md les accompagne, dans
# Contents/Resources/ : il porte les mentions que leurs licences exigent.
#
# **Cette app ne se distribue pas.** Ni téléversée, ni publiée, ni partagée, ni fabriquée par la
# CI : les images ne sont pas à ce projet (PolyForm Noncommercial pour ses poses, des captures et
# du fan art pour son dessin d'avant, voir CREDITS.md), elles ne sont pas dans le dépôt, et
# chacun les télécharge pour soi. L'app est faite sur cette machine, pour cette machine.
#
# Elle est signée « ad hoc », pour cette machine seulement, **après** la copie des ressources :
# la signature les scelle. Remplacer une image dans l'app casse donc sa signature ; pour essayer
# d'autres images, CAPRI_ASSETS reste le moyen. macOS rattache les permissions à cette signature :
# après chaque reconstruction, il redemande le micro et la reconnaissance vocale.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
APP="$ROOT/target/Capri.app"
ASSETS="${CAPRI_ASSETS:-$ROOT/assets}"
# un chemin relatif se lit depuis là où le script est lancé, pas depuis le dépôt
if [ -d "$ASSETS" ]; then ASSETS="$(cd "$ASSETS" && pwd -P)"; fi
cd "$ROOT"

# Les images que lit le code : son dessin d'avant, indispensable (assets.rs : SPRITE), sa pose
# assise (capri_core::sheets) et sa pose debout (capri_core::sheets::stand). Les autres images
# que télécharge tools/fetch-assets.sh ne servent pas : elles restent hors de l'app.
IMAGES=(
  capri/capri-cutout.png
  capri/perch/perch.png
  capri/perch/perch_blink.png
  capri/perch/perch_talk.png
  capri/perch/perch_music.png
  capri/perch/perch_arrive.png
  capri/perch/perch_leave.png
  capri/stand/idle.png
  capri/stand/idle_blink.png
  capri/stand/idle_mouth.png
)

# Avant de compiler : toutes les images sont-elles là ?
missing=""
for image in "${IMAGES[@]}"; do
  [ -f "$ASSETS/$image" ] || missing="$missing $image"
done
if [ -n "$missing" ]; then
  echo "Images de Capri introuvables dans $ASSETS :$missing" >&2
  echo "Lance d'abord ./tools/fetch-assets.sh (il les télécharge et vérifie leurs empreintes)," >&2
  echo "ou indique leur dossier (celui qui contient capri/) avec CAPRI_ASSETS=…" >&2
  exit 1
fi

cargo build --release -p capri-app -p capri-hook

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/capri target/release/capri-hook "$APP/Contents/MacOS/"
sed "s/@VERSION@/${VERSION:-0.0.0}/g" tools/macos/Info.plist > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" > /dev/null

# Les ressources, sans recopier les attributs étendus des originaux (-X) : des informations du
# Finder ou une ressource « fork » feraient refuser la signature. (macOS ajoute de lui-même
# com.apple.provenance aux fichiers écrits, que codesign accepte.)
for image in "${IMAGES[@]}"; do
  mkdir -p "$APP/Contents/Resources/assets/$(dirname "$image")"
  cp -X "$ASSETS/$image" "$APP/Contents/Resources/assets/$image"
done
cp -X CREDITS.md "$APP/Contents/Resources/CREDITS.md"

# Signer en dernier : le second programme d'abord, puis l'app, qui scelle tout ce qu'elle contient.
codesign --force --sign - "$APP/Contents/MacOS/capri-hook"
codesign --force --sign - "$APP"
codesign --verify --strict "$APP"

echo "Capri.app est prête, avec ses images : $APP"
echo "Pour la lancer : open \"$APP\"   (quitte d'abord une Capri déjà lancée)"
