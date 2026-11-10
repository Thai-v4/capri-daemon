# Capri — consignes pour Claude Code

Compagne de bureau (Capri, *Cyberpunk: Edgerunners* 2) écrite en Rust, 100 % native, pour macOS
(cible principale, Apple Silicon) et Windows. Projet de fan non commercial. Le README décrit l'état
du projet et la feuille de route ; jalon suivant : le hook Claude Code (jalon 2).

## Architecture : où va quoi

- `capri-core` : logique pure, **aucune dépendance système ni GPU**. Tout ce qui peut être testé
  sans écran va ici, avec des tests.
- `capri-render` : wgpu uniquement. Couleurs prémultipliées de bout en bout.
- `capri-platform` : **seul endroit** pour les appels système et l'`unsafe` lié à l'OS.
  Une fonction publique dans `lib.rs`, une implémentation par plateforme (`macos.rs`,
  `windows.rs`, `other.rs`). Toute nouvelle fonction doit exister dans les trois.
- `capri-app` : assemble le tout. Pas d'appel système direct ici (sauf menus via `tray-icon`).

## Vérifier avant de dire « c'est fait »

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Le code Windows ne se teste pas depuis un Mac : la CI GitHub Actions le compile et le teste.
Tout comportement visible (transparence, clics, animation) se vérifie à la main : dire
précisément quoi vérifier plutôt que de l'affirmer.

## Règles

- **API des crates** : wgpu 30, winit 0.30, objc2 0.6 / objc2-app-kit 0.3, windows-sys 0.61,
  tray-icon 0.26. Ces API changent souvent : vérifier dans les sources (`~/.cargo/registry/src`)
  ou docs.rs, pas de mémoire.
- Chaque bloc `unsafe` a un commentaire `// SAFETY:` qui justifie pourquoi il est sûr.
- Commentaires et textes affichés en français ; identifiants en anglais.
- Animation façon anime : poses clés tenues un nombre entier d'images à 24 i/s, pas
  d'interpolation (voir `capri-core/src/anim.rs`).
- Elle tourne toute la journée : ne redessiner que quand la pose change, rester léger en CPU et
  en batterie.
- **Ne jamais copier de code** de mochiiwuff/capri-desktop ou CyberMoonie/capri-macos (licence
  PolyForm Noncommercial). S'inspirer des idées, réécrire.
- **Images** : jamais commitées (`assets/capri/` est ignoré). Elles viennent de
  `tools/fetch-assets.sh`, qui vérifie leurs empreintes. Le code les charge au lancement.
  Dans un worktree sans images, utiliser `CAPRI_ASSETS=/chemin/vers/assets`.
- Aucune télémétrie, rien n'envoyé sur le réseau sans action explicite de l'utilisateur.
- Pour le hook Claude Code (jalon 2) : socket Unix (Windows : named pipe) accessible au seul
  utilisateur, jamais un port TCP sans authentification.
