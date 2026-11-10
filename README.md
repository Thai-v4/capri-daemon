# Capri

Capri, la construct de *Cyberpunk: Edgerunners* saison 2, vit sur ton bureau. Écrite en Rust,
100 % native, pour **macOS** et **Windows**.

> Projet de fan non commercial. Voir [CREDITS.md](CREDITS.md) pour le personnage et les images.

## Jalon 1 : ce qu'elle fait

- Elle flotte en bas à droite de l'écran, animée façon anime : des poses clés tenues plusieurs
  images, un trait qui « bout » à chaque dessin, une copie violette mal repérée.
- La souris traverse la fenêtre partout, sauf sur elle, au pixel près.
- **Attrape-la** au clic gauche pour la déplacer. **Lâche-la** doucement et elle reste là ;
  **lance-la** et elle file, ralentit et rebondit sur les bords de l'écran.
- **Clic droit sur elle** (ou Ctrl + clic sur macOS), ou son icône dans la barre des menus
  (macOS) ou la zone de notification (Windows) : la cacher ou quitter.
- **⌃⌥C** (Ctrl+Alt+C) la cache ou la fait revenir, depuis n'importe quelle app.
- Sur macOS : elle est sur tous les bureaux, au-dessus des apps en plein écran, sans icône dans
  le Dock, et la toucher ne prend jamais le clavier à l'app en cours.

## Installer et lancer

Il faut [Rust](https://rustup.rs) 1.90 ou plus récent (`rustup update`).

**macOS**

```bash
xcode-select --install        # si ce n'est pas déjà fait
./tools/fetch-assets.sh       # télécharge et vérifie les images
cargo run --release
```

**Windows** (avec les outils de compilation C++ de Visual Studio, demandés par rustup)

```powershell
powershell -ExecutionPolicy Bypass -File tools\fetch-assets.ps1
cargo run --release
```

Aucune permission système n'est nécessaire pour ce jalon. Pour plus de détails dans le terminal :
`RUST_LOG=debug cargo run --release`.

## Organisation

```
crates/
  capri-core/      logique pure : temps d'animation, mouvement, lancer, hit-test (testée)
  capri-render/    rendu wgpu : Metal sur macOS, DirectX 12 + DirectComposition sur Windows
    sprite.wgsl    le shader : boil et copie violette
  capri-platform/  seule couche qui parle au système : la fenêtre « Overlay »
    macos.rs         AppKit via objc2 : un NSPanel non activant, sans fenêtre winit
    winit_overlay.rs fenêtre winit, pour Windows et les autres systèmes
    windows.rs       Win32 via windows-sys
    other.rs         repli (Linux : affichage seul)
  capri-app/       l'application : boucle, souris, menus, raccourci, images
tools/             récupération vérifiée des images
```

Règle : tout ce qui peut être testé sans écran va dans `capri-core`.

## État de validation

Vérifié automatiquement :

- formatage, `clippy -D warnings` et 16 tests sur **macOS Apple Silicon** : animation, lancer et
  rebonds, hit-test, interprétation des clics, prémultiplication, et **validation du shader** par
  le compilateur de wgpu ;
- `clippy -D warnings` en compilation croisée pour **Windows x64** et **Linux x64** (le code
  compile, il n'est pas exécuté).

Le rendu réel sous Linux avec un GPU logiciel (le sprite s'affiche, flotte et « bout ») avait été
vérifié avant le passage à la fenêtre « Overlay » ; il n'a pas été revérifié depuis.

**Vérifié à la main sur un vrai Mac** (MacBook Pro M5 Pro, macOS 27.2, le 10 novembre 2026) :

1. le fond est transparent (pas de rectangle noir) ;
2. les clics traversent l'écran partout sauf sur elle ;
3. on peut l'attraper, la poser, la lancer ;
4. elle reste visible en changeant de bureau et au-dessus d'une app en plein écran ;
5. clic droit, Ctrl + clic, icône de la barre des menus et ⌃⌥C répondent ; « Quitter » la ferme
   proprement ;
6. le clavier reste dans l'app en cours quand on l'attrape, qu'on la lance ou qu'on ouvre son
   menu, et elle ne prend pas le focus à son lancement ;
7. elle suit un changement de résolution de l'écran.

**Pas encore testé sur un vrai PC Windows** : y repasser les points 1 à 5.

Points à surveiller :

- **Windows** : la combinaison transparence (DirectComposition) + passage des clics est la partie
  la moins certaine. Si la fenêtre est invisible ou noire, regarder `capri-platform/src/windows.rs`.
- **macOS** : non testé avec plusieurs écrans, ni en branchant ou débranchant un écran externe
  (elle vit sur l'écran qui porte la barre des menus).
- **macOS** : au tout premier lancement, l'icône de la barre des menus n'a pas été vue ; elle
  était là à tous les lancements suivants. Non reproduit, cause inconnue.

## Feuille de route

1. ~~Elle flotte, on l'attrape, on la lance~~ ← jalon 1
2. **Le hook Claude Code** : un petit binaire Rust, une socket Unix réservée à ton utilisateur,
   et un bandeau qui affiche ce que fait Claude.
3. Elle s'assoit sur tes fenêtres (macOS : CoreGraphics + Accessibilité ; Windows : Win32).
4. Le chat avec un modèle d'IA (Ollama ou une API compatible OpenAI), clés dans le trousseau.
5. App signée et notarisée (`.app` / installeur), mises à jour.

## Licence

Licence du **code** à choisir (MIT ou Apache-2.0 par exemple). Les **images** n'en relèvent pas :
voir [CREDITS.md](CREDITS.md).
