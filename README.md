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

## Jalon 2 : elle te dit ce que fait Claude Code

Un bandeau apparaît au-dessus d'elle quand une session [Claude Code](https://code.claude.com)
travaille : « Claude lit main.rs », « Claude lance : Run the test suite », puis « Claude a fini ».
Il passe au **rouge** quand Claude attend quelque chose de toi (une permission, une réponse), et se
tait dès qu'il n'y a plus rien à dire.

```bash
cargo build --release                      # construit capri et capri-hook
./target/release/capri hook install        # branche Claude Code (--dry-run pour simuler)
./target/release/capri hook status         # où en est le branchement
./target/release/capri hook uninstall      # le débranche
```

Le menu de Capri propose la même bascule (« Brancher / Débrancher Claude Code »). Les sessions
déjà ouvertes prennent le changement en compte toutes seules ; `/hooks` dans Claude Code liste
les entrées `capri-hook`.

- **Aucun modèle n'est appelé, aucune clé n'est nécessaire** : tout vient des
  [hooks](https://code.claude.com/docs/en/hooks) de Claude Code, quel que soit ton abonnement.
- **Rien ne sort de ta machine** : le hook parle à Capri par une socket locale réservée à ton
  utilisateur. Il ne transmet ni tes prompts, ni le contenu de tes fichiers, ni la sortie de tes
  commandes : seulement le nom de l'outil et un résumé court (le nom d'un fichier, la description
  d'une commande).
- **Ton `settings.json` est respecté** : Capri ajoute ses propres groupes après les hooks
  existants, sans en modifier aucun, après une sauvegarde datée à côté du fichier. Elle refuse
  d'y toucher si elle ne sait pas le réécrire à l'identique, et le débranchement le rend tel
  qu'il était, à l'octet près.
- **Claude Code n'est jamais gêné** : les hooks sont asynchrones, `capri-hook` répond en quelques
  millisecondes, et si Capri est fermée (ou le binaire absent) il ne se passe rien.

Limites connues, qui viennent de ce que Claude Code signale ou non :

- après une **interruption** (Échap), Claude Code n'envoie aucun événement : le bandeau garde son
  dernier texte jusqu'au prochain prompt, ou se tait au bout de deux minutes sans nouvelles
  (dix si une commande était en cours) ;
- une **permission accordée** n'est pas signalée : « Claude attend ton accord » reste affiché
  jusqu'à la fin de l'outil en question ;
- dans un dossier que tu n'as pas encore approuvé, Claude Code retient tous les hooks.

## Jalon 4, premier lot : lui parler

**⌃⌥P** (Ctrl+Alt+P), ou « Parler à Capri » dans son menu, ouvre un champ de saisie sous ses
pieds. Entrée envoie, Échap ou un clic ailleurs referme, et le clavier revient tout seul à la
fenêtre où tu étais. Elle répond dans une bulle au-dessus d'elle, qui se remplit mot après mot et
tourne ses pages au rythme de la lecture.

Pour l'instant elle n'a **pas de cerveau** : elle répète ce qu'on lui dit. Ce lot met au point la
saisie et la bulle ; aucun modèle n'est appelé, rien ne part sur le réseau. Les lots suivants
brancheront un modèle local, puis des fournisseurs à clé. Le champ de saisie n'existe encore que
sur macOS.

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

Aucune permission système n'est nécessaire. Pour plus de détails dans le terminal :
`RUST_LOG=debug cargo run --release` ; pour comprendre ce que fait le hook :
`CAPRI_HOOK_DEBUG=1` (il écrit alors une ligne sur sa sortie d'erreur).

## Organisation

```
crates/
  capri-core/      logique pure : animation, mouvement, lancer, hit-test, bandeau, parole (testée)
  capri-claude/    logique pure : hooks de Claude Code, protocole, sessions, settings.json (testée)
  capri-chat/      logique pure : conversation, réponses en flux, cerveau provisoire (testée)
  capri-render/    rendu wgpu : Metal sur macOS, DirectX 12 + DirectComposition sur Windows
    sprite.wgsl    le shader : boil et copie violette
    banner.rs      le texte du bandeau et de la bulle, tracé avec les polices du système
  capri-platform/  la fenêtre « Overlay » : première des deux couches qui parlent au système
    macos.rs         AppKit via objc2 : un NSPanel non activant, sans fenêtre winit
    macos/input.rs   le champ de saisie : le seul panneau qui prend le clavier, sur demande
    winit_overlay.rs fenêtre winit, pour Windows et les autres systèmes
    windows.rs       Win32 via windows-sys
    other.rs         repli (Linux : affichage seul)
  capri-ipc/       le canal local : seconde couche système, minuscule
    unix.rs          socket Unix dans un dossier réservé à l'utilisateur
    windows.rs       tube nommé au nom aléatoire
  capri-hook/      le binaire appelé par les hooks de Claude Code
  capri-app/       l'application : boucle, souris, menus, raccourci, images, bandeau
tools/             récupération vérifiée des images
```

Règle : tout ce qui peut être testé sans écran va dans `capri-core`, `capri-claude` ou
`capri-chat`.

## État de validation

Vérifié automatiquement :

- formatage, `clippy -D warnings` et 131 tests sur **macOS Apple Silicon** : animation, lancer et
  rebonds, hit-test, clics, bandeau, parole en pages, bulle sur plusieurs lignes, conversation,
  **validation du shader** par le compilateur de wgpu, lecture
  des exemples de hooks de la documentation de Claude Code, suivi des sessions, aller-retour sur
  `settings.json`, socket (droits, connexions simultanées), et le binaire `capri-hook` lancé pour
  de vrai (sortie standard vide, code 0, durée bornée) ;
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
7. elle suit un changement de résolution de l'écran ;
8. le bandeau suit une vraie session Claude Code, passe au rouge pour une demande, disparaît
   après la fin du tour et accompagne Capri quand on la déplace ;
9. brancher puis débrancher Claude Code (au terminal et par le menu) : les hooks existants sont
   intacts, et le `settings.json` redevient identique à l'original, à l'octet près ;
10. Capri fermée, ou binaire `capri-hook` absent : aucune erreur ni ralentissement dans Claude Code ;
11. le champ de saisie prend le clavier (accents et émojis compris) et le rend à la fenêtre
    d'avant en se fermant ; la bulle répète la phrase, tourne ses pages, et s'affiche même quand
    Claude Code attend l'utilisateur.

**Pas encore testé sur un vrai PC Windows** : y repasser les points 1 à 5, puis 8 à 10. Le tube
nommé de `capri-ipc` n'y a jamais tourné, et le champ de saisie n'y existe pas encore.

Points à surveiller :

- **Windows** : la combinaison transparence (DirectComposition) + passage des clics est la partie
  la moins certaine. Si la fenêtre est invisible ou noire, regarder `capri-platform/src/windows.rs`.
- **macOS** : non testé avec plusieurs écrans, ni en branchant ou débranchant un écran externe
  (elle vit sur l'écran qui porte la barre des menus).
- **macOS** : au tout premier lancement, l'icône de la barre des menus n'a pas été vue ; elle
  était là à tous les lancements suivants. Non reproduit, cause inconnue.

## Feuille de route

1. ~~Elle flotte, on l'attrape, on la lance~~ ← jalon 1
2. ~~Le hook Claude Code : un petit binaire Rust, une socket Unix réservée à ton utilisateur, et
   un bandeau qui affiche ce que fait Claude~~ ← jalon 2
3. Elle s'assoit sur tes fenêtres (macOS : CoreGraphics + Accessibilité ; Windows : Win32).
4. Le chat avec un modèle d'IA : Claude par le binaire `claude` de ton propre abonnement (sans clé
   d'API, à vérifier contre les conditions d'Anthropic), Ollama, ou une API compatible OpenAI
   avec ses clés dans le trousseau.
5. App signée et notarisée (`.app` / installeur), mises à jour.

## Licence

Licence du **code** à choisir (MIT ou Apache-2.0 par exemple). Les **images** n'en relèvent pas :
voir [CREDITS.md](CREDITS.md).
