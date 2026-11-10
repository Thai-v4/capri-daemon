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

## Jalon 4 : lui parler

**⌃⌥P** (Ctrl+Alt+P), ou « Parler à Capri » dans son menu, ouvre un champ de saisie sous ses
pieds. Entrée envoie, Échap ou un clic ailleurs referme, et le clavier revient tout seul à la
fenêtre où tu étais. Elle répond dans une bulle au-dessus d'elle, qui se remplit mot après mot et
tourne ses pages au rythme de la lecture. Le champ de saisie n'existe encore que sur macOS.

### Lui donner un cerveau

Sans réglage, elle n'a **pas de cerveau** : elle répète ce qu'on lui dit, et rien ne part sur le
réseau. Pour qu'elle réponde vraiment, donne-lui un modèle local servi par
[Ollama](https://ollama.com) : ni compte, ni clé, et la conversation ne quitte pas ta machine.

```bash
brew install ollama
ollama serve                              # ou, pour qu'il démarre tout seul : brew services start ollama
ollama pull <modèle>                      # à choisir sur ollama.com/library, selon ta mémoire
./target/release/capri chat use ollama    # prend le modèle installé (--model <nom> s'il y en a plusieurs)
./target/release/capri chat test          # lui parler depuis le terminal
./target/release/capri chat status        # quel cerveau elle utilise, et s'il répond
./target/release/capri chat off           # lui retirer son cerveau
```

Le choix vaut dès ta phrase suivante, sans relancer Capri. Dans son menu, « La faire taire » coupe
sa réponse (le modèle cesse d'écrire) et « Nouvelle conversation » lui fait oublier ce qui s'est
dit ; elle garde en mémoire les 24 dernières prises de parole, et rien n'est écrit sur le disque.

Sa personnalité tient en quelques lignes, qui sont les nôtres et pas une citation de la série.
Pour la changer, écris la tienne dans `persona.md`, dans le dossier de ses réglages (sur macOS,
`~/Library/Application Support/Capri/`) : `capri chat status` en donne le chemin.

Ce que Capri garantit :

- **seules sa personnalité et la conversation partent vers le modèle** : jamais ce que fait
  Claude Code, ni rien d'autre de ta machine ;
- **elle ne parle qu'à l'adresse choisie** : aucun proxy, même si l'environnement en déclare un,
  et aucune redirection suivie ;
- **en clair (`http://`), seulement vers cette machine ou un réseau privé** donné par son adresse
  IP, par exemple `--url http://192.168.1.20:11434/v1` pour un Ollama qui tourne ailleurs chez
  toi. Toute autre adresse en `http://` est refusée ;
- **tout est borné** : le temps de connexion, le silence du serveur (deux minutes), la longueur
  d'une réponse.

Ce qu'il faut savoir :

- pas encore de `https://` : les fournisseurs à clé (OpenRouter, DeepSeek, ProxyAPI, Anthropic)
  arrivent avec le lot suivant, leurs clés dans le trousseau du système ;
- Ollama décharge le modèle après quelques minutes sans rien lui demander : la réponse suivante
  met alors quelques secondes à commencer ;
- avec un modèle « à réflexion », la bulle reste sur « … » pendant qu'il réfléchit : il n'y a pas
  encore de réglage pour l'en empêcher ;
- si tu la fais taire avant son premier mot, la connexion n'est fermée qu'au premier signe du
  serveur ;
- un modèle écrit volontiers du Markdown : les signes sont retirés à l'affichage, les mots restent.

### Sa voix

Capri peut dire ses réponses à voix haute, avec une voix du système : rien n'est téléchargé, et
rien n'est envoyé nulle part. Chaque phrase est dite dès qu'elle est complète, pendant que le
modèle écrit la suite.

```bash
./target/release/capri voice on           # ou « Activer sa voix » dans son menu
./target/release/capri voice test         # l'écouter depuis le terminal
./target/release/capri voice list         # les voix françaises installées
./target/release/capri voice use <nom>    # en choisir une (auto : la meilleure installée)
./target/release/capri voice off
```

- « La faire taire », ou une nouvelle phrase, coupe sa voix en même temps que sa réponse ; la
  cacher (⌃⌥C) la met en pause ;
- les émojis ne sont pas lus, et une panne (modèle injoignable) reste écrite dans sa bulle ;
- par défaut, elle prend la meilleure voix française de femme installée. Un Mac n'est livré
  qu'avec des voix de base, assez synthétiques : de meilleures se téléchargent dans Réglages
  Système > Accessibilité > Lire et énoncer > Voix système (libellés de macOS 27) ;
- ce n'est **pas** la voix de la série, et ça ne le sera pas : pas de clonage de la voix d'une
  comédienne ;
- macOS seulement pour l'instant. La bulle et la voix avancent chacune à son rythme, et sa
  bouche ne bouge pas.

### Lui parler au micro

Maintiens **⌃⌥M** (Ctrl+Alt+M), parle, relâche : ce que tu as dit lui est envoyé comme si tu
l'avais tapé. Pendant que tu parles, une bulle bleue montre ce qu'elle comprend. Le menu propose
la même chose (« Lui parler au micro » : un clic pour commencer, un autre pour finir).

```bash
./tools/bundle-macos.sh                   # fabrique target/Capri.app
open target/Capri.app                     # quitte d'abord une Capri déjà lancée
./target/release/capri ear status         # si elle peut t'écouter, et sinon pourquoi
```

- **le son ne quitte pas ta machine** : la transcription est faite par la reconnaissance vocale
  de macOS, exigée sur l'appareil. Si le système ne sait pas transcrire le français sans passer
  par ses serveurs, Capri refuse d'écouter plutôt que d'envoyer ta voix ;
- **le micro n'est ouvert que pendant que tu maintiens le raccourci** (le témoin orange de macOS
  le montre), et rien n'est enregistré ;
- **il faut la lancer depuis `Capri.app`** : macOS n'accorde le micro et la reconnaissance vocale
  qu'à une app qui dit pourquoi elle les veut. À la première écoute, il te pose les deux
  questions ; réponds, puis recommence. Lancée nue (`cargo run`), Capri garde tout le reste et dit
  simplement qu'elle ne peut pas t'écouter ;
- l'app est signée pour ta machine seulement, et macOS rattache ses permissions à cette
  signature : après chaque reconstruction, il les redemande ;
- quand tu commences à parler, elle se tait. Le micro met un instant à s'ouvrir : laisse passer
  une demi-seconde avant ton premier mot ;
- pas encore de mot d'éveil, ni de détection de fin de phrase : c'est toi qui dis quand tu as
  fini, en relâchant. macOS seulement.

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
  capri-chat/      logique pure : conversation, format des fournisseurs, flux de réponse, règles
                   d'adresse, réglages du cerveau, personnalité, phrases à dire, dictée (testée)
  capri-net/       le seul crate qui parle au réseau : envoyer la conversation, lire la réponse
                   au fil de l'eau (testé contre un faux serveur local)
  capri-render/    rendu wgpu : Metal sur macOS, DirectX 12 + DirectComposition sur Windows
    sprite.wgsl    le shader : boil et copie violette
    banner.rs      le texte du bandeau et de la bulle, tracé avec les polices du système
  capri-platform/  la fenêtre « Overlay » : première des deux couches qui parlent au système
    macos.rs         AppKit via objc2 : un NSPanel non activant, sans fenêtre winit
    macos/input.rs   le champ de saisie : le seul panneau qui prend le clavier, sur demande
    voice.rs         sa voix ; macos/voice.rs : la synthèse vocale du système
    ear.rs           son oreille ; macos/ear.rs : le micro et la reconnaissance sur l'appareil
    winit_overlay.rs fenêtre winit, pour Windows et les autres systèmes
    windows.rs       Win32 via windows-sys
    other.rs         repli (Linux : affichage seul)
  capri-ipc/       le canal local : seconde couche système, minuscule
    unix.rs          socket Unix dans un dossier réservé à l'utilisateur
    windows.rs       tube nommé au nom aléatoire
  capri-hook/      le binaire appelé par les hooks de Claude Code
  capri-app/       l'application : boucle, souris, menus, raccourci, images, bandeau, bulle,
                   voix, écoute, et les commandes `capri hook`, `capri chat`, `capri voice`,
                   `capri ear`
tools/             récupération vérifiée des images, fabrication de Capri.app (macOS)
```

Règle : tout ce qui peut être testé sans écran va dans `capri-core`, `capri-claude` ou
`capri-chat`.

## État de validation

Vérifié automatiquement :

- formatage, `clippy -D warnings` et 210 tests sur **macOS Apple Silicon** : animation, lancer et
  rebonds, hit-test, clics, bandeau, parole en pages, bulle sur plusieurs lignes, conversation,
  **validation du shader** par le compilateur de wgpu, lecture
  des exemples de hooks de la documentation de Claude Code, suivi des sessions, aller-retour sur
  `settings.json`, socket (droits, connexions simultanées), et le binaire `capri-hook` lancé pour
  de vrai (sortie standard vide, code 0, durée bornée) ;
- le chat, sans sortir de la machine : le format des fournisseurs et le flux de réponse coupé à
  tous les endroits possibles, les adresses acceptées et refusées, le nettoyage du Markdown (ce
  qui est montré n'est jamais repris), et le client réseau contre un faux serveur local (requête
  envoyée, arrêt en cours de réponse, refus, redirection jamais suivie, serveur muet, réponse
  coupée ou sans fin) ;
- la voix, pour ce qui se teste sans haut-parleur : le découpage en phrases quel que soit le
  rythme d'arrivée du texte, ce qui se prononce ou non, et le choix de la voix ;
- l'écoute, pour ce qui se teste sans micro : le déroulé d'une prise de parole (texte provisoire,
  dernier mot attendu puis abandonné, silence, abandon) ;
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
12. avec un modèle local (Ollama), elle répond en français dans sa bulle, se souvient de la
    conversation, l'oublie avec « Nouvelle conversation », et « La faire taire » coupe sa réponse ;
13. sa voix dit la réponse phrase par phrase, se coupe net avec « La faire taire », se met en
    pause quand on cache Capri et reprend à son retour ; « Couper sa voix » la rend muette.

**Pas encore testé sur un vrai PC Windows** : y repasser les points 1 à 5, puis 8 à 10. Le tube
nommé de `capri-ipc` n'y a jamais tourné, le champ de saisie et la voix n'y existent pas encore,
et le chat n'y a jamais été lancé.

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
4. Le chat avec un modèle d'IA : ~~la saisie et la bulle~~, ~~un modèle local par Ollama~~, puis
   les fournisseurs à clé (OpenRouter, DeepSeek, ProxyAPI, Anthropic) avec leurs clés dans le
   trousseau, et Capri dans Claude Code pour qui a un abonnement. Sa voix : ~~ses réponses
   dites à voix haute~~, ~~lui parler au micro~~ (reconnaissance sur la machine), puis un mot
   d'éveil.
5. App signée et notarisée (`.app` / installeur), mises à jour.

## Licence

Licence du **code** à choisir (MIT ou Apache-2.0 par exemple). Les **images** n'en relèvent pas :
voir [CREDITS.md](CREDITS.md).
