# Capri — consignes pour Claude Code

Compagne de bureau (Capri, *Cyberpunk: Edgerunners* 2) écrite en Rust, 100 % native, pour macOS
(cible principale, Apple Silicon) et Windows. Projet de fan non commercial. Le README décrit l'état
du projet et la feuille de route ; jalons 1 (elle flotte) et 2 (le bandeau Claude Code) faits,
jalon 4 (lui parler) en cours : saisie, bulle et modèle local (Ollama) en place ; fournisseurs à
clé, puis voix, à venir.

## Architecture : où va quoi

- `capri-core` : logique pure, **aucune dépendance système ni GPU**. Tout ce qui peut être testé
  sans écran va ici, avec des tests.
- `capri-claude` : tout ce que Capri sait de Claude Code, lui aussi **sans dépendance système** :
  lecture de l'entrée des hooks, protocole, suivi des sessions, édition de `settings.json` de
  texte à texte. Avec des tests.
- `capri-chat` : la conversation, **sans dépendance système ni réseau** : historique borné,
  format des fournisseurs (ce qu'on envoie, comment lire le flux), règles d'adresse, réglages du
  cerveau, personnalité, nettoyage du Markdown. Avec des tests.
- `capri-net` : **le seul crate qui parle au réseau** (ureq). Il se connecte et rien d'autre :
  quoi envoyer et comment lire la réponse est décidé par `capri-chat`. Testé contre un faux
  serveur local ; aucun test ne sort de la machine.
- `capri-render` : wgpu, et le tracé du texte du bandeau et de la bulle (cosmic-text, polices du
  système).
  Couleurs prémultipliées de bout en bout.
- Les appels système et l'`unsafe` lié à l'OS vivent dans **deux crates, et nulle part
  ailleurs** : `capri-platform` (la fenêtre) et `capri-ipc` (le canal local).
- `capri-ipc` : socket Unix ou tube nommé Windows, réservés à l'utilisateur. À part de
  `capri-platform` parce que `capri-hook` le charge à chaque événement de Claude Code : ni winit
  ni AppKit ici.
- `capri-hook` : le binaire que les hooks appellent. Deux dépendances (`capri-claude`,
  `capri-ipc`), rien d'autre.
- `capri-platform` : la fenêtre et son `unsafe`.
  L'API publique est dans `lib.rs` (le type `Overlay` et ses événements), avec une
  implémentation par plateforme : `macos.rs` (un `NSPanel` non activant, sans fenêtre winit :
  cliquer sur Capri ne doit jamais prendre le focus) et `winit_overlay.rs` (fenêtre winit, réglée
  par `windows.rs` ou `other.rs`). Toute nouvelle fonction doit exister partout, au besoin sous
  la forme d'un refus explicite (le champ de saisie n'existe encore que sur macOS).
  Seul le champ de saisie (`macos/input.rs`) prend le clavier, parce qu'on le lui demande, et il
  le rend à la fenêtre d'avant en se fermant.
- `capri-app` : assemble le tout. Pas d'appel système direct ici (sauf menus via `tray-icon`).
  La fenêtre passe toujours par `capri_platform::Overlay`, jamais par `winit::window::Window`.

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
  tray-icon 0.26, cosmic-text 0.19, ureq 3.5 (sans TLS pour l'instant). Ces API changent souvent : vérifier dans les sources
  (`~/.cargo/registry/src`) ou docs.rs, pas de mémoire. Même règle pour Claude Code : le format
  des hooks se vérifie dans sa documentation (<https://code.claude.com/docs/en/hooks>).
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
- Le chat : seules la personnalité et la conversation partent vers le modèle, et seulement vers
  l'adresse que l'utilisateur a choisie. Jamais l'activité de Claude Code. Pas de proxy, aucune
  redirection suivie. `http://` seulement vers la machine ou un réseau privé : toute adresse
  passe par `capri_chat::Endpoint`. Ce qui se dit n'est jamais écrit dans le journal.
- Aucun nom de modèle écrit en dur : il vient de l'utilisateur ou de la liste que donne le
  serveur. Même règle que pour les API : format et adresses d'un fournisseur se vérifient dans sa
  documentation, pas de mémoire.
- Le canal du hook : socket Unix (Windows : named pipe) accessible au seul utilisateur, jamais
  un port TCP sans authentification.
- `capri-hook` ne doit **jamais gêner Claude Code** : rien sur la sortie standard, toujours le
  code de sortie 0, une durée de vie bornée. Les hooks sont déclarés `async`.
- Le bandeau ne transmet ni n'affiche jamais le texte d'un prompt, le contenu d'un fichier ou la
  sortie d'une commande. Dans le doute, il se tait plutôt que d'affirmer quelque chose de périmé.
- Au-dessus de sa tête, une seule chose à la fois : sa parole d'abord (on vient de lui parler),
  puis un message de l'app, puis l'activité de Claude Code.
- Le `settings.json` de Claude Code appartient à l'utilisateur et à d'autres outils : toute
  modification passe par `capri_claude::settings` (réécriture à l'octet près, vérification) et
  par `hook_setup` (sauvegarde, écriture atomique). Jamais d'édition directe.
- CapriChooms/CapriCore (GPL-3.0) : on s'inspire de ses idées, on ne copie pas son code.
