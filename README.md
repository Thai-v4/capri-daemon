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
- **Clic droit sur elle**, ou son icône dans la barre des menus (macOS) ou la zone de
  notification (Windows) : la cacher ou quitter.
- **⌃⌥C** (Ctrl+Alt+C) la cache ou la fait revenir, depuis n'importe quelle app.
- Sur macOS : elle est sur tous les bureaux, au-dessus des apps en plein écran, sans icône dans le Dock.

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
  capri-platform/  seule couche qui parle au système
    macos.rs       AppKit via objc2
    windows.rs     Win32 via windows-sys
    other.rs       repli (Linux : affichage seul)
  capri-app/       l'application : boucle, souris, menus, raccourci, images
tools/             récupération vérifiée des images
```

Règle : tout ce qui peut être testé sans écran va dans `capri-core`.

## État de validation

Vérifié avant livraison :

- compilation, `clippy -D warnings` et formatage pour **macOS Apple Silicon**, **Windows x64**
  et Linux ;
- 14 tests : animation, lancer et rebonds, hit-test, prémultiplication, et **validation du
  shader** par le compilateur de wgpu ;
- **rendu réel** sous Linux avec un GPU logiciel : le sprite s'affiche, flotte et « bout ».

**Pas encore testé sur un vrai Mac ni un vrai PC Windows.** Au premier lancement, vérifier :

1. le fond est bien transparent (pas de rectangle noir) ;
2. les clics traversent l'écran partout sauf sur elle ;
3. on peut l'attraper, la poser, la lancer ;
4. elle reste visible en changeant de bureau et au-dessus d'une app en plein écran (macOS) ;
5. clic droit, icône de la barre des menus et ⌃⌥C répondent.

Points à surveiller :

- **Windows** : la combinaison transparence (DirectComposition) + passage des clics est la partie
  la moins certaine. Si la fenêtre est invisible ou noire, regarder `capri-platform/src/windows.rs`.
- **macOS** : cliquer sur elle active probablement l'app et retire le focus à la fenêtre en
  cours. À corriger avec une fenêtre non activante (`NSPanel`).

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
