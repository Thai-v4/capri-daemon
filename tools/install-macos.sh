#!/usr/bin/env bash
# Installe Capri.app dans /Applications (ou dans le dossier donné) : une app qui ne dépend plus de
# ce dépôt, ni de son dossier target/ (un `cargo clean` ou le dépôt déplacé ne la cassent plus).
#
#   tools/install-macos.sh [--no-hook] [dossier]
#
# Dans l'ordre :
# 1. il refuse si une Capri de ce paquet tourne : celle de target/Capri.app de ce dépôt, ou celle
#    de la destination. Quitte-la par « Quitter » dans son menu. Ce script ne quitte, ne tue ni
#    ne signale jamais aucun processus : il lit seulement la liste (pgrep, lsof). Il regarde avant
#    de fabriquer l'app (après, il aurait déjà remplacé l'app sous une Capri qui tourne), puis de
#    nouveau juste avant de la mettre en place ;
# 2. il fabrique l'app, avec ses images, par tools/bundle-macos.sh (CAPRI_ASSETS lui est
#    transmise) ;
# 3. il la copie dans la destination d'un seul coup : une copie complète sous un nom temporaire,
#    à côté, dont la signature est vérifiée, puis un renommage. Une installation précédente (une
#    Capri, jamais une autre app du même nom) est mise de côté le temps du renommage, puis retirée,
#    ou remise en place si le renommage échoue : un échec ne laisse jamais une demi-app ;
# 4. le hook de Claude Code : si, et seulement si, un hook de Capri est déjà inscrit, il suit
#    l'app installée, par sa commande `capri hook install --if-plugged` (sauvegarde du fichier,
#    écriture d'un seul coup). Sans hook de Capri, rien n'est inscrit. Ce script ne lit ni ne
#    modifie jamais settings.json lui-même. --no-hook saute cette étape. (Avant l'étape 5 : le
#    hook mène à l'app installée avant que l'ancienne disparaisse.) ;
# 5. il retire target/Capri.app : il ne reste qu'une app de cet identifiant sur la machine, et
#    c'est ce qui fait que l'app « a changé de place » ;
# 6. l'ouverture de session : il n'écrit rien. Si « Se lancer à l'ouverture de session » est
#    cochée, l'app répare elle-même son agent au lancement suivant, depuis sa nouvelle place,
#    parce que l'app inscrite n'existe plus (capri_core::login::repair) ;
# 7. il dit quoi faire ensuite, sans lancer l'app.
#
# Jamais sudo : si la destination n'est pas inscriptible, il le dit et s'arrête. L'app installée
# contient les images : elle ne se distribue pas (voir tools/bundle-macos.sh).
set -euo pipefail

usage() {
  cat <<'USAGE'
usage : tools/install-macos.sh [--no-hook] [dossier]

Fabrique Capri.app avec ses images, l'installe dans le dossier (par défaut /Applications), puis
retire target/Capri.app. Si Claude Code est déjà branché sur Capri, son hook suit l'app installée.

  --no-hook   ne pas toucher au hook de Claude Code
USAGE
}

die() {
  echo "install-macos : $*" >&2
  exit 1
}

DEST_DIR=""
FOLLOW_HOOK=1
while [ $# -gt 0 ]; do
  case "$1" in
    --no-hook) FOLLOW_HOOK=0 ;;
    -h | --help)
      usage
      exit 0
      ;;
    -*)
      usage >&2
      exit 2
      ;;
    *)
      if [ -n "$DEST_DIR" ]; then
        usage >&2
        exit 2
      fi
      DEST_DIR="$1"
      ;;
  esac
  shift
done
DEST_DIR="${DEST_DIR:-/Applications}"

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
BUILT="$ROOT/target/Capri.app"
BUNDLE_ID="local.capri-daemon.capri"

# Deux chemins mènent-ils à la même place ? Sans tenir compte de la casse, comme le disque de macOS
# par défaut (un chemin tapé en minuscules mène à la même app).
lower() { printf '%s' "$1" | tr '[:upper:]' '[:lower:]'; }

# La destination : un dossier qui existe, à toi.
[ -d "$DEST_DIR" ] || die "$DEST_DIR n'existe pas ou n'est pas un dossier : rien n'est installé."
DEST_DIR="$(cd "$DEST_DIR" && pwd -P)"
[ -w "$DEST_DIR" ] || die "$DEST_DIR n'est pas inscriptible pour toi : rien n'est installé. \
Ce script ne demande jamais sudo ; choisis un dossier où tu peux écrire (par exemple ~/Applications)."
INSTALLED="$DEST_DIR/Capri.app"
case "$(lower "$INSTALLED")/" in
  "$(lower "$BUILT")/"*) die "la destination est target/Capri.app de ce dépôt, que ce script retire : rien n'est installé." ;;
esac

# Ce qui porte déjà ce nom dans la destination doit être une Capri : il ne remplace jamais
# l'app de quelqu'un d'autre, ni un lien, ni un fichier.
check_previous() {
  local found
  if [ -L "$INSTALLED" ] || { [ -e "$INSTALLED" ] && [ ! -d "$INSTALLED" ]; }; then
    die "$INSTALLED existe et n'est pas une app (un lien ou un fichier) : rien n'est installé."
  fi
  if [ -d "$INSTALLED" ]; then
    found="$(plutil -extract CFBundleIdentifier raw -o - "$INSTALLED/Contents/Info.plist" 2>/dev/null || true)"
    [ "$found" = "$BUNDLE_ID" ] ||
      die "$INSTALLED n'est pas Capri (identifiant « ${found:-aucun} ») : rien n'est installé."
  fi
}

# Le numéro d'un processus « capri » qui tourne depuis l'app $1, s'il y en a un. Le chemin de son
# programme est lu par lsof (le vrai chemin, même pour un programme lancé par un chemin relatif),
# à défaut par ps. Rien d'autre : aucun signal, aucun processus touché.
running_from() {
  local program pid path
  program="$(lower "$1/Contents/MacOS/capri")"
  for pid in $(pgrep -x capri || true); do
    path="$(lsof -a -p "$pid" -d txt -Fn 2>/dev/null | sed -n 's/^n//p' | head -n 1 || true)"
    [ -n "$path" ] || path="$(ps -o comm= -p "$pid" 2>/dev/null || true)"
    if [ "$(lower "$path")" = "$program" ]; then
      echo "$pid"
      return 0
    fi
  done
  return 1
}

refuse_if_running() {
  local app pid
  for app in "$BUILT" "$INSTALLED"; do
    if pid="$(running_from "$app")"; then
      die "Capri tourne depuis $app (processus $pid) : quitte-la par « Quitter » dans son menu, \
puis relance ce script. Il ne quitte jamais Capri lui-même."
    fi
  done
}

check_previous
refuse_if_running

# 2. Fabriquer l'app.
"$ROOT/tools/bundle-macos.sh"

# 3. La copier d'un seul coup. Les noms temporaires ne finissent pas par .app : ni le Finder ni
# macOS ne les prennent pour une app.
STAGED="$DEST_DIR/.Capri.app.capri-install-$$"
ASIDE="$DEST_DIR/.Capri.app.capri-previous-$$"
# À la sortie, quelle qu'elle soit (une erreur, Ctrl+C) : rien de temporaire ne reste, et une
# installation précédente mise de côté est remise en place si la nouvelle n'y est pas.
cleanup() {
  if [ -e "$STAGED" ]; then rm -rf "$STAGED"; fi
  if [ -e "$ASIDE" ]; then
    if [ -e "$INSTALLED" ]; then
      rm -rf "$ASIDE"
    elif mv "$ASIDE" "$INSTALLED"; then
      echo "install-macos : l'installation précédente est remise en place : $INSTALLED" >&2
    else
      echo "install-macos : l'installation précédente est restée dans $ASIDE" >&2
    fi
  fi
}
trap cleanup EXIT
trap 'exit 130' INT TERM HUP

echo "Copie dans ${DEST_DIR}…"
ditto "$BUILT" "$STAGED"
codesign --verify --strict "$STAGED" || die "la copie n'a pas une signature valide : rien n'est installé."
check_previous
refuse_if_running
if [ -d "$INSTALLED" ]; then mv "$INSTALLED" "$ASIDE"; fi
# mv mettrait la copie *dans* un dossier qui serait réapparu entre-temps
[ ! -e "$INSTALLED" ] || die "$INSTALLED est réapparu pendant l'installation : rien n'est installé."
mv "$STAGED" "$INSTALLED"
if [ -e "$ASIDE" ]; then rm -rf "$ASIDE"; fi
codesign --verify --strict "$INSTALLED"
echo "Capri est installée : $INSTALLED (signature vérifiée)"

# 4. Le hook de Claude Code, par la commande de l'app installée, seulement s'il est déjà branché.
CAPRI="$INSTALLED/Contents/MacOS/capri"
HOOK_FAILED=0
echo
if [ "$FOLLOW_HOOK" = 1 ]; then
  echo "Le hook de Claude Code (il ne suit l'app installée que s'il était déjà branché) :"
  if ! "$CAPRI" hook install --if-plugged; then
    HOOK_FAILED=1
    echo "install-macos : le hook de Claude Code n'a pas suivi (voir ci-dessus). L'app est installée ;" >&2
    echo "pour le rebrancher : « Réglages… » dans son menu, rubrique Claude Code, ou :" >&2
    echo "  \"$CAPRI\" hook install" >&2
  fi
else
  echo "Le hook de Claude Code n'est pas touché (--no-hook). S'il était branché sur l'ancienne app,"
  echo "il mène à un programme qui n'existera plus ; pour qu'il suive l'app installée :"
  echo "  \"$CAPRI\" hook install --if-plugged"
fi

# 5. Il ne reste qu'une Capri.app de cet identifiant.
rm -rf "$BUILT"
echo
echo "target/Capri.app est retirée : l'app installée ne dépend plus de ce dépôt."

# 6. et 7. Ce qui reste à faire, sans lancer l'app.
cat <<EOF

Ensuite :
- lance-la : open "$INSTALLED" (ou un double-clic dans le Finder) ; ce script ne la lance pas ;
- à ta première écoute, macOS te redemandera le micro et la reconnaissance vocale : sa signature
  « ad hoc » change à chaque fabrication ;
- si « Se lancer à l'ouverture de session » était cochée, lance-la une fois depuis sa nouvelle
  place avant ta prochaine ouverture de session : elle répare elle-même son inscription (d'ici là,
  l'ouverture de session chercherait l'ancienne app, qui n'existe plus).
EOF
exit "$HOOK_FAILED"
