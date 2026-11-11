#!/usr/bin/env bash
# Récupère les images de Capri, chacune depuis son dépôt d'origine, à une révision figée, et
# vérifie l'empreinte SHA-256 de chaque fichier. Voir CREDITS.md pour leur provenance.
#
# - mochiiwuff/capri-desktop : le dessin debout, les visages, l'introduction (assets/capri/) ;
# - lachydotmcg/capri-desktop : sa pose assise, dessinée par l'artiste de ce projet
#   (assets/capri/perch/). PolyForm Noncommercial 1.0.0 : usage non commercial seulement.
set -euo pipefail

DEST="$(cd "$(dirname "$0")/.." && pwd)/assets/capri"

sha() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

# fetch <dépôt> <révision> <dossier dans le dépôt> <dossier local>, la liste sur l'entrée standard
fetch() {
  local repo="$1" commit="$2" from="$3" dest="$4"
  mkdir -p "$dest"
  while read -r hash file; do
    [ -z "$file" ] && continue
    local out="$dest/$file"
    if [ -f "$out" ] && [ "$(sha "$out")" = "$hash" ]; then continue; fi
    curl -fsSL "https://raw.githubusercontent.com/$repo/$commit/$from/$file" -o "$out.part"
    local got
    got="$(sha "$out.part")"
    if [ "$got" != "$hash" ]; then
      rm -f "$out.part"
      echo "Empreinte inattendue pour $file ($got) : fichier refusé." >&2
      exit 1
    fi
    mv "$out.part" "$out"
    echo "ok  $file"
  done
}

# 2026-11-09
fetch mochiiwuff/capri-desktop c011764ca1fdbd191925321e3aaa8af5b84babe8 assets/capri "$DEST" <<'LIST'
d2b505f3a649f7ded725835d7f33dc5402d4f3c3f867f313310e394ebec9b45d  capri-cutout.png
56d2546d06ec389f46ab3bce265524e8f3fd46ea7b8a666b4886984e83a0bba0  capri-face.png
7df28bdd6a264307d3a25bafaf12c2bba308d5aecd2dc03dcc9b176109f21aeb  capri-full.png
a0d6fb18b7a384f866d9151c859c0e8762328c933af785fb128c524e3ae36852  cutin-eyes.png
24b98c9f7e717aefb7855cd7b9532fec0bd24e29a1d15713408332d8ecdb3d65  cutin-grin.png
e856783445e411536e4d34a7622a83405f8b3d2da2786883a2146105a8f16496  cutin-laugh.png
ec693184070926921e0f7b3b5d325a2acf4405bb05540ccc09ba97e297192f4e  cutin-look.png
3e374eca7ab4d1d3401c3d5bc82499e20edd4ceb5029e868d4f55353d9f2e5a4  cutin-many.png
b559b2678ec42439fb2b2801dcffcc47c33bb6c48f33f97f31985609d51ba4d6  face-aside.png
befe8f647c6d12058b3c710519ea49b5750502732d6e10c80a04b4d9f754c6b8  face-calm.png
89cbf307dee4421532fe02959a75fdce6322ad2dd1063b1cd5b4b5a22e32594f  face-laugh.png
fe589f8a1ad8f8e5bdd3dd9bb9304a6da723e7678ca9cfc8d72eb4301ccbc9e5  face-look.png
9263f26c5ffb45a7981b2c11bb348be6f4ca3bfbba787517da1f77d421145e09  face-many.png
37dec337c2825a63cf4697de04c57192a4f650d92697de2108c6928e74bf2299  face-smug.png
56bd354b6df06202453b189c72c4f1cccd69b9dfb74628c014bd73a41c92371e  face-wide.png
def390fa517e44376cce6f4ff86ebebd74e67d71290ccc8406dbe8da1867bb84  intro-grab.webp
59b6f223e88a623da0a9d58251275f974662504cc62238e264273920108267ee  intro-happy.webp
f2e9ed502e55f097e86dc278ad424374b98bada206385d2b220e1e2acbbe7076  intro-mouth1.webp
0458d7be033bf8507fa9fc040657cf6d07ffc7c25b4a802fb04ec9b00c0da07b  intro-mouth2.webp
c1acf8ba1312c5194fefa666ccb3fe960c9177b1f8b86d7e1e2bc6c692409c26  intro-mouth3.webp
3b883513eb88179bb03ad94b17c1a777a9b1ea307be72802d82fefd1eaa0f1d1  intro-smear.webp
76f6f2fa0aa71fd761f72bbdf51d1be8babcfe5d0f7b0cf796659bd500f50b73  intro-wave1.webp
c14cbd4a64ca2bb48c30f9a51f5bda2f7d93de2a54e65cd955c58a89652b9b1b  intro-wave2.webp
66134c6cdd5b502ad0b52fb0f5db33901164f098b33384f222dfe4df54583ed8  intro-wave3.webp
LIST

# 2026-11-11 ; seulement la pose assise, créditée à l'artiste du projet
fetch lachydotmcg/capri-desktop 6f719c4269b0fa0e841587f072a1b1594eebcb91 assets/sprites "$DEST/perch" <<'LIST'
431e418dcb419d92a2893ec026493ce9fd8b6e5942b76c734e53c459ff9df66b  perch.png
b9091804740d6e71a16292ecd296748e3ef5ba668f30f62c96d5f7613621abd7  perch_arrive.png
bb67abb8470fb0e006daba804d120d846805da55431691d05fbb95cd99800233  perch_blink.png
61027b971f66b966e99247283636a8545070ad7d33fa5d7cf797536d1386ab78  perch_leave.png
358a49b783ada47453f3a64113310121a2e6379749492109f31a5c943fe0cf15  perch_music.png
0f2cdeb193b805dd379d05cb8fc6a8ea1dcba1b197ad3c0731204d91862069d7  perch_talk.png
LIST

echo "Images prêtes dans $DEST"
