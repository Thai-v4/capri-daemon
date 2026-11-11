# Récupère les images de Capri, chacune depuis son dépôt d'origine, à une révision figée, et
# vérifie l'empreinte SHA-256 de chaque fichier. Voir CREDITS.md pour leur provenance.
#
# - mochiiwuff/capri-desktop : le dessin debout, les visages, l'introduction (assets\capri\) ;
# - lachydotmcg/capri-desktop : sa pose assise et sa pose debout, dessinées par l'artiste de ce
#   projet (assets\capri\perch\, assets\capri\stand\). PolyForm Noncommercial 1.0.0 : usage non
#   commercial seulement.
$ErrorActionPreference = 'Stop'
$Dest = Join-Path (Split-Path -Parent $PSScriptRoot) 'assets\capri'

function Fetch($Repo, $Commit, $From, $To, $Files) {
    New-Item -ItemType Directory -Force -Path $To | Out-Null
    foreach ($name in $Files.Keys) {
        $out = Join-Path $To $name
        $hash = $Files[$name]
        if ((Test-Path $out) -and ((Get-FileHash $out -Algorithm SHA256).Hash.ToLower() -eq $hash)) { continue }
        $part = "$out.part"
        Invoke-WebRequest -UseBasicParsing -Uri "https://raw.githubusercontent.com/$Repo/$Commit/$From/$name" -OutFile $part
        $got = (Get-FileHash $part -Algorithm SHA256).Hash.ToLower()
        if ($got -ne $hash) {
            Remove-Item $part
            throw "Empreinte inattendue pour $name ($got) : fichier refusé."
        }
        Move-Item -Force $part $out
        Write-Host "ok  $name"
    }
}

# 2026-11-09
Fetch 'mochiiwuff/capri-desktop' 'c011764ca1fdbd191925321e3aaa8af5b84babe8' 'assets/capri' $Dest ([ordered]@{
    'capri-cutout.png' = 'd2b505f3a649f7ded725835d7f33dc5402d4f3c3f867f313310e394ebec9b45d'
    'capri-face.png' = '56d2546d06ec389f46ab3bce265524e8f3fd46ea7b8a666b4886984e83a0bba0'
    'capri-full.png' = '7df28bdd6a264307d3a25bafaf12c2bba308d5aecd2dc03dcc9b176109f21aeb'
    'cutin-eyes.png' = 'a0d6fb18b7a384f866d9151c859c0e8762328c933af785fb128c524e3ae36852'
    'cutin-grin.png' = '24b98c9f7e717aefb7855cd7b9532fec0bd24e29a1d15713408332d8ecdb3d65'
    'cutin-laugh.png' = 'e856783445e411536e4d34a7622a83405f8b3d2da2786883a2146105a8f16496'
    'cutin-look.png' = 'ec693184070926921e0f7b3b5d325a2acf4405bb05540ccc09ba97e297192f4e'
    'cutin-many.png' = '3e374eca7ab4d1d3401c3d5bc82499e20edd4ceb5029e868d4f55353d9f2e5a4'
    'face-aside.png' = 'b559b2678ec42439fb2b2801dcffcc47c33bb6c48f33f97f31985609d51ba4d6'
    'face-calm.png' = 'befe8f647c6d12058b3c710519ea49b5750502732d6e10c80a04b4d9f754c6b8'
    'face-laugh.png' = '89cbf307dee4421532fe02959a75fdce6322ad2dd1063b1cd5b4b5a22e32594f'
    'face-look.png' = 'fe589f8a1ad8f8e5bdd3dd9bb9304a6da723e7678ca9cfc8d72eb4301ccbc9e5'
    'face-many.png' = '9263f26c5ffb45a7981b2c11bb348be6f4ca3bfbba787517da1f77d421145e09'
    'face-smug.png' = '37dec337c2825a63cf4697de04c57192a4f650d92697de2108c6928e74bf2299'
    'face-wide.png' = '56bd354b6df06202453b189c72c4f1cccd69b9dfb74628c014bd73a41c92371e'
    'intro-grab.webp' = 'def390fa517e44376cce6f4ff86ebebd74e67d71290ccc8406dbe8da1867bb84'
    'intro-happy.webp' = '59b6f223e88a623da0a9d58251275f974662504cc62238e264273920108267ee'
    'intro-mouth1.webp' = 'f2e9ed502e55f097e86dc278ad424374b98bada206385d2b220e1e2acbbe7076'
    'intro-mouth2.webp' = '0458d7be033bf8507fa9fc040657cf6d07ffc7c25b4a802fb04ec9b00c0da07b'
    'intro-mouth3.webp' = 'c1acf8ba1312c5194fefa666ccb3fe960c9177b1f8b86d7e1e2bc6c692409c26'
    'intro-smear.webp' = '3b883513eb88179bb03ad94b17c1a777a9b1ea307be72802d82fefd1eaa0f1d1'
    'intro-wave1.webp' = '76f6f2fa0aa71fd761f72bbdf51d1be8babcfe5d0f7b0cf796659bd500f50b73'
    'intro-wave2.webp' = 'c14cbd4a64ca2bb48c30f9a51f5bda2f7d93de2a54e65cd955c58a89652b9b1b'
    'intro-wave3.webp' = '66134c6cdd5b502ad0b52fb0f5db33901164f098b33384f222dfe4df54583ed8'
})

# 2026-11-11 ; seulement la pose assise, créditée à l'artiste du projet
Fetch 'lachydotmcg/capri-desktop' '6f719c4269b0fa0e841587f072a1b1594eebcb91' 'assets/sprites' (Join-Path $Dest 'perch') ([ordered]@{
    'perch.png' = '431e418dcb419d92a2893ec026493ce9fd8b6e5942b76c734e53c459ff9df66b'
    'perch_arrive.png' = 'b9091804740d6e71a16292ecd296748e3ef5ba668f30f62c96d5f7613621abd7'
    'perch_blink.png' = 'bb67abb8470fb0e006daba804d120d846805da55431691d05fbb95cd99800233'
    'perch_leave.png' = '61027b971f66b966e99247283636a8545070ad7d33fa5d7cf797536d1386ab78'
    'perch_music.png' = '358a49b783ada47453f3a64113310121a2e6379749492109f31a5c943fe0cf15'
    'perch_talk.png' = '0f2cdeb193b805dd379d05cb8fc6a8ea1dcba1b197ad3c0731204d91862069d7'
})

# même dépôt, même révision ; seulement sa pose debout, la boucle que le dépôt attribue à l'artiste
Fetch 'lachydotmcg/capri-desktop' '6f719c4269b0fa0e841587f072a1b1594eebcb91' 'assets/sprites' (Join-Path $Dest 'stand') ([ordered]@{
    'idle.png' = 'a3657cdd00c8b730b215510a8ed3599b6c646a4b2e953d68399ac8afaf9cd0a1'
    'idle_blink.png' = '16dddd29ff55790654f974298845f43f28d978ea18c21a28727fafb531a9a178'
    'idle_mouth.png' = 'e8c3e5fbdeb1647406da3149e4f24ff129d723e905d4b42585fca89efdcba75f'
})

Write-Host "Images prêtes dans $Dest"
