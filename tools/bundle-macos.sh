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
# Son icône (Capri.icns) est fabriquée ici, par les outils du système, à partir de capri-face.png
# du même dossier d'images : une image de plus qui entre dans l'app (voir plus bas).
#
# **Cette app ne se distribue pas.** Ni téléversée, ni publiée, ni partagée, ni fabriquée par la
# CI : les images ne sont pas à ce projet (PolyForm Noncommercial pour ses poses, des captures et
# du fan art pour son dessin d'avant, voir CREDITS.md), elles ne sont pas dans le dépôt, et
# chacun les télécharge pour soi. L'app est faite sur cette machine, pour cette machine.
#
# Elle est signée **après** la copie des ressources : la signature les scelle. Remplacer une image
# dans l'app casse donc sa signature ; pour essayer d'autres images, CAPRI_ASSETS reste le moyen.
# macOS rattache les permissions (le micro, la reconnaissance vocale) à cette signature :
# - sans identité de signature, elle est « ad hoc », pour cette machine seulement, et change à
#   chaque fabrication : après chacune, il les redemande ;
# - avec une identité « Developer ID Application » de ton trousseau (le certificat d'un compte
#   développeur Apple, et sa clé), ce que macOS retient de l'app est son identifiant et ton
#   équipe, qui ne changent pas d'une fabrication à l'autre.
# Laquelle : CAPRI_SIGN_IDENTITY si elle est posée (un nom ou une empreinte que codesign connaît,
# ou « - » pour « ad hoc ») ; sinon la seule identité « Developer ID Application » valide du
# trousseau, s'il y en a une ; sinon « ad hoc ». S'il y en a plusieurs, le script s'arrête plutôt
# que de choisir. Du trousseau, il ne lit que les noms des identités de signature (`security
# find-identity`) : c'est codesign qui se sert de la clé, et macOS demande de l'y autoriser.
#
# Signer se fait sur cette machine, **sans rien envoyer** : pas d'horodatage par le serveur
# d'Apple (--timestamp=none), et jamais de notarisation, qui téléverserait l'app et ses images
# chez Apple. Une identité de signature ne change rien à « cette app ne se distribue pas ».
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
APP="$ROOT/target/Capri.app"
ASSETS="${CAPRI_ASSETS:-$ROOT/assets}"
# un chemin relatif se lit depuis là où le script est lancé, pas depuis le dépôt
if [ -d "$ASSETS" ]; then ASSETS="$(cd "$ASSETS" && pwd -P)"; fi
cd "$ROOT"

# Les images que lit le code : son dessin d'avant, indispensable (assets.rs : SPRITE), sa pose
# assise (capri_core::sheets), sa pose debout (capri_core::sheets::stand), les planches de sa
# vie (capri_core::sheets::life) et l'intro du lancement (capri_core::intro : 01.png à 22.png).
# Les autres images que télécharge tools/fetch-assets.sh ne servent pas : elles restent hors de
# l'app.
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
  capri/life/arrive.png
  capri/life/leave.png
  capri/intro/01.png
  capri/intro/02.png
  capri/intro/03.png
  capri/intro/04.png
  capri/intro/05.png
  capri/intro/06.png
  capri/intro/07.png
  capri/intro/08.png
  capri/intro/09.png
  capri/intro/10.png
  capri/intro/11.png
  capri/intro/12.png
  capri/intro/13.png
  capri/intro/14.png
  capri/intro/15.png
  capri/intro/16.png
  capri/intro/17.png
  capri/intro/18.png
  capri/intro/19.png
  capri/intro/20.png
  capri/intro/21.png
  capri/intro/22.png
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
if ! command -v cargo > /dev/null; then
  echo "cargo est introuvable : installe Rust (https://rustup.rs), ou ajoute son dossier au PATH." >&2
  exit 1
fi

# Avec quoi signer (voir l'en-tête), décidé avant de compiler : CAPRI_SIGN_IDENTITY, sinon la seule
# identité « Developer ID Application » valide du trousseau, sinon « ad hoc » (« - » pour codesign).
SIGN_IDENTITY="${CAPRI_SIGN_IDENTITY:-}"
if [ "$SIGN_IDENTITY" = "-" ]; then
  SIGN_SAYS="« ad hoc », comme le demande CAPRI_SIGN_IDENTITY"
elif [ -n "$SIGN_IDENTITY" ]; then
  SIGN_SAYS="avec l'identité que désigne CAPRI_SIGN_IDENTITY"
else
  # Une ligne par identité valide de cette sorte : son empreinte, puis son nom. Rien d'autre
  # n'est lu du trousseau, et aucune clé.
  identities="$(security find-identity -v -p codesigning 2> /dev/null |
    sed -n 's/^ *[0-9]*) \([0-9A-F]\{40\}\) "\(Developer ID Application: .*\)"$/\1 \2/p' || true)"
  count="$(printf '%s\n' "$identities" | grep -c . || true)"
  case "$count" in
    0)
      SIGN_IDENTITY="-"
      SIGN_SAYS="« ad hoc » (aucune identité « Developer ID Application » valide dans ton trousseau)"
      ;;
    1)
      SIGN_IDENTITY="${identities%% *}"
      SIGN_SAYS="avec ton identité « ${identities#* } »"
      ;;
    *)
      echo "Plusieurs identités « Developer ID Application » sont dans ton trousseau :" >&2
      printf '%s\n' "$identities" | sed 's/^/  /' >&2
      echo "Choisis-en une : relance avec CAPRI_SIGN_IDENTITY=<son empreinte> devant la commande" >&2
      echo "(ou CAPRI_SIGN_IDENTITY=- pour une signature « ad hoc »)." >&2
      exit 1
      ;;
  esac
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

# L'icône : Contents/Resources/Capri.icns (celle que nomme CFBundleIconFile, dans Info.plist),
# tirée de capri-face.png, que l'utilisateur a déjà téléchargée. Elle n'est pas dans IMAGES : le
# code ne la lit pas, et sans elle l'app est fabriquée quand même, sans icône. L'image mesure
# 256 x 256 : on ne fabrique que les tailles de 16 à 256 (réductions par sips), jamais un
# agrandissement pour les tailles 512 et 1024 (iconutil accepte un jeu sans elles). Rien d'autre
# que des outils du système ; les fichiers de travail vont dans un dossier temporaire, retiré à
# la sortie. Avant la signature, qui scelle cette ressource.
FACE="$ASSETS/capri/capri-face.png"
if [ -f "$FACE" ]; then
  ICON_WORK="$(mktemp -d "${TMPDIR:-/tmp}/capri-icone.XXXXXX")"
  trap 'rm -rf "$ICON_WORK"' EXIT
  mkdir "$ICON_WORK/Capri.iconset"
  # taille:nom, les noms qu'impose iconutil (man iconutil, « iconset »)
  for spec in 16:icon_16x16 32:icon_16x16@2x 32:icon_32x32 64:icon_32x32@2x \
    128:icon_128x128 256:icon_128x128@2x 256:icon_256x256; do
    size="${spec%%:*}"
    /usr/bin/sips -z "$size" "$size" -s format png "$FACE" \
      --out "$ICON_WORK/Capri.iconset/${spec#*:}.png" > /dev/null
  done
  /usr/bin/iconutil -c icns -o "$ICON_WORK/Capri.icns" "$ICON_WORK/Capri.iconset"
  cp -X "$ICON_WORK/Capri.icns" "$APP/Contents/Resources/Capri.icns"
else
  echo "capri-face.png est introuvable dans $ASSETS/capri : Capri.app est fabriquée sans icône." >&2
fi

# Signer en dernier, sur cette machine et sans rien envoyer (--timestamp=none : pas d'horodatage
# par un serveur) : le second programme d'abord, puis l'app, qui scelle tout ce qu'elle contient.
# Si une signature échoue (identité inconnue, accès à sa clé refusé dans la fenêtre du trousseau),
# l'app à moitié faite est retirée : il ne reste jamais une Capri.app qui ne serait pas signée.
sign() {
  if ! codesign --force --timestamp=none --sign "$SIGN_IDENTITY" "$1"; then
    rm -rf "$APP"
    echo "La signature a échoué ($SIGN_SAYS) : Capri.app n'est pas fabriquée." >&2
    echo "Pour une signature « ad hoc » à la place : relance avec CAPRI_SIGN_IDENTITY=- devant la commande." >&2
    exit 1
  fi
}
sign "$APP/Contents/MacOS/capri-hook"
sign "$APP"
codesign --verify --strict "$APP"

echo "Capri.app est prête, avec ses images : $APP"
echo "Elle est signée $SIGN_SAYS."
echo "Pour la lancer : open \"$APP\"   (quitte d'abord une Capri déjà lancée)"
