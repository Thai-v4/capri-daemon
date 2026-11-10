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
réseau. Pour qu'elle réponde vraiment, donne-lui un modèle. Sur macOS, tout ce qui suit se règle
aussi sans terminal, dans sa [fenêtre de réglages](#ses-réglages-sans-terminal) ; les commandes
restent, et les deux disent et font la même chose.

**Sur ta machine**, servi par [Ollama](https://ollama.com) ou [LM Studio](https://lmstudio.ai) :
ni compte, ni clé, et la conversation ne quitte pas ta machine (sauf avec un modèle « cloud »
d'Ollama : voir plus bas).

```bash
brew install ollama
ollama serve                              # ou, pour qu'il démarre tout seul : brew services start ollama
ollama pull <modèle>                      # à choisir sur ollama.com/library, selon ta mémoire
./target/release/capri chat use ollama    # prend le modèle installé (--model <nom> s'il y en a plusieurs)
./target/release/capri chat use lmstudio  # ou LM Studio, son serveur local démarré
./target/release/capri chat test          # lui parler depuis le terminal
./target/release/capri chat status        # quel cerveau elle utilise, où part ce qu'on lui dit, et s'il répond
./target/release/capri chat off           # lui retirer son cerveau
```

**Chez un fournisseur, sur Internet** : [OpenRouter](https://openrouter.ai),
[DeepSeek](https://platform.deepseek.com), [ProxyAPI](https://proxyapi.ru),
[Ollama en ligne](https://docs.ollama.com/cloud) (les serveurs d'Ollama, joints directement, avec
une clé créée sur <https://ollama.com/settings/keys>), ou tout autre serveur qui parle le format
« chat completions » d'OpenAI. Il faut un compte chez lui et sa clé d'API, et
**ce que tu dis à Capri part alors chez lui**, avec sa personnalité, à chaque message :
`capri chat use` le rappelle.

```bash
./target/release/capri chat key openrouter                # colle la clé : elle ne s'affiche pas
pbpaste | ./target/release/capri chat key openrouter      # ou depuis le presse-papiers (macOS)
./target/release/capri chat models openrouter <mot>       # chercher le nom d'un modèle
./target/release/capri chat use openrouter --model <nom>  # le nom est obligatoire chez un fournisseur
./target/release/capri chat test
./target/release/capri chat key openrouter --forget       # retirer la clé du trousseau
```

Une clé est donnée **pour une adresse** : celle de `--url` (`capri chat key custom --url
https://…`), sinon celle du cerveau choisi s'il est de ce fournisseur, sinon son adresse habituelle.
La commande dit toujours pour laquelle elle la range, et un serveur personnalisé sans adresse
choisie demande `--url`.

| fournisseur  | adresse par défaut             | clé                                         |
| ------------ | ------------------------------ | ------------------------------------------- |
| `ollama`     | `http://localhost:11434/v1`    | aucune                                      |
| `lmstudio`   | `http://localhost:1234/v1`     | facultative (si tu en exiges une dans LM Studio) |
| `openrouter` | `https://openrouter.ai/api/v1` | obligatoire                                 |
| `deepseek`   | `https://api.deepseek.com`     | obligatoire                                 |
| `proxyapi`   | `https://api.proxyapi.ru/v1`   | obligatoire                                 |
| `ollama-cloud` | `https://ollama.com/v1`      | obligatoire                                 |
| `custom`     | à donner avec `--url`          | facultative                                 |

`--url` remplace l'adresse de n'importe lequel, par exemple `--url http://192.168.1.20:11434/v1`
pour un Ollama qui tourne ailleurs chez toi. La clé ne suit pas : celle de l'adresse habituelle
n'est jamais envoyée à une autre. Les adresses viennent de la documentation de chaque
fournisseur, relue le 10 novembre 2026.

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
  aucune redirection suivie, et aucun en-tête qui la présente au fournisseur (seul part celui de
  la bibliothèque HTTP, `User-Agent: ureq/<version>`, qui ne nomme pas Capri) ;
- **en clair (`http://`), seulement vers cette machine ou un réseau privé** donné par son adresse
  IP. Toute autre adresse en `http://` est refusée ;
- **en `https://`, le TLS du système** (Security.framework sur macOS, SChannel sur Windows) et les
  certificats auxquels il fait confiance : un certificat refusé arrête tout, avant le moindre mot ;
- **la clé ne vit que dans le trousseau du système** (le trousseau de session sur macOS, le
  Gestionnaire d'identification sur Windows) : jamais dans `chat.json` ni ailleurs sur le disque,
  jamais dans le journal, jamais à l'écran, jamais en argument d'une commande.
  `capri chat status` dit si elle est là, sans la montrer ;
- **une clé est liée à l'adresse pour laquelle elle a été donnée** : son nom dans le trousseau
  dit le fournisseur et l'adresse, hôte et port (`openrouter@openrouter.ai:443`), elle n'est lue
  que pour cette adresse, et `capri-net` refuse de la mettre dans une requête vers une autre.
  Choisir une autre adresse (`--url`, un autre port, `localhost` au lieu de `127.0.0.1`) ne
  l'emporte jamais : il faut en donner une pour celle-là ;
- **une clé ne part jamais en clair** : seulement en `https://`, ou vers cette machine même,
  jamais en `http://` sur le réseau, même privé. Un fournisseur qui n'en prend pas (l'Ollama de ta
  machine) n'en reçoit jamais ;
- si un fournisseur répète la clé dans un message d'erreur, elle y est masquée ; une clé refusée
  se dit dans sa bulle, avec la commande pour la remettre ;
- **tout est borné** : le temps de connexion, le silence du serveur (deux minutes), la longueur
  d'une réponse.

Ce qu'il faut savoir :

- **sur macOS, un élément du trousseau appartient à l'app qui l'a rangé**. Une autre app doit
  en demander l'accès, et macOS te pose la question : c'est ce qui devrait arriver quand
  `Capri.app` lit une clé rangée depuis le terminal, ou après une reconstruction. Réponds
  « Toujours autoriser ». Cette demande n'a pas encore été vue à la main ;
- une clé par fournisseur et par adresse : deux serveurs personnalisés, ou deux ports de la même
  machine, ont chacun la leur ;
- une clé rangée par une version précédente de Capri (sous le nom du fournisseur seul, sans
  adresse) n'est **jamais envoyée** : `capri chat status` la signale. Redonne-la
  (`capri chat key <fournisseur>`), ce qui retire l'ancienne ; `--forget` la retire aussi ;
- taper la clé au terminal passe, sur macOS, par `readpassphrase(3)` du système : un Ctrl+C rend
  l'écho du terminal avant d'arrêter la commande. Sur Windows, ce que laisse un Ctrl+C n'a pas
  été vérifié ;
- « La faire taire » ferme la connexion, mais un fournisseur peut compter la réponse entière quand
  même : OpenRouter le précise pour une partie des fournisseurs qu'il sert ;
- sous Linux, ni `https://` ni trousseau (il y faudrait OpenSSL et un autre trousseau) : seulement
  un modèle sur la machine ou le réseau privé ;
- **un modèle « cloud » d'Ollama ne reste pas sur ta machine**, même demandé à l'Ollama local :
  sa documentation les nomme `…:cloud` ou `…-cloud`, et l'Ollama connecté (`ollama signin`) les
  relaie vers ses serveurs. Capri reconnaît ces noms et le dit (`capri chat use`, `capri chat
  status`) au lieu de promettre que rien ne sort ; un modèle « cloud » nommé autrement ne serait pas
  reconnu ;
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

Maintiens **⌃⌥D** (Ctrl+Alt+D, comme « dicter »), parle, relâche : ce que tu as dit lui est envoyé comme si tu
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

## Ses réglages, sans terminal

Sur macOS, **« Réglages… »** dans son menu (icône de la barre des menus, ou clic droit sur elle)
ouvre la fenêtre « Réglages de Capri ». Elle prend le clavier, et le rend à la fenêtre où tu
étais quand tu la fermes (bouton rouge, ⌘W ou Échap) ; Capri continue de flotter pendant ce
temps. La rouvrir ramène la même fenêtre devant. ⌘V, ⌘C, ⌘X, ⌘A et ⌘Z marchent dans ses champs
(et désormais dans le champ où on lui parle). ⌘Q et ⌘H, dans cette fenêtre comme dans le champ où
on lui parle (⌃⌥P), ne ferment que celui-ci, comme ⌘W ou Échap : Capri ne se quitte que par
« Quitter » dans son menu. Ce qu'elle dit dans sa bulle, et ce que dit la fenêtre, ne renvoie
jamais à une commande de terminal : la bulle dit d'ouvrir « Réglages… » dans son menu, la fenêtre
dit quoi faire sur place (seules les commandes `capri chat` et `capri voice` gardent les leurs).

**Cerveau**

- le **fournisseur**, dans une liste (« Aucun » : elle répète ce qu'on lui dit), et son
  **adresse**, préremplie, modifiable (Entrée la valide) ;
- dès qu'un fournisseur est choisi dans la liste, avant qu'il devienne son cerveau, la phrase qui
  dit **où part ce que tu lui dis** (la même que `capri chat use`), en orange quand c'est sur
  Internet ;
- la **clé** : la fenêtre dit s'il y en a une pour l'adresse affichée (« Une clé est rangée pour
  openrouter.ai:443 », « Aucune clé pour cette adresse »), sans jamais la montrer. Colle-la dans
  le champ (rien ne s'affiche), puis « Enregistrer la clé » (ou Entrée) : elle va dans le
  trousseau, pour cette adresse seulement, et le champ se vide, avec ce que ⌘Z pourrait y
  remettre. « Retirer la clé » l'en retire. La clé reste dans ton presse-papiers, d'où tu l'as
  collée : Capri n'y touche pas. Mêmes règles qu'en ligne de commande : jamais en clair, jamais
  vers une autre adresse ;
- le **modèle** : la liste vient du serveur. Celle d'un serveur de ta machine est demandée
  d'elle-même ; ailleurs, seulement quand tu appuies sur « Demander la liste » (et jamais sans la
  clé qu'il exige). Chez un fournisseur qui en propose des centaines, un champ de recherche, et
  30 résultats au plus ;
- **Essayer** lui dit « Présente-toi en une phrase. » avec ce qui est affiché, et montre sa
  réponse, ou la panne, dans les mêmes mots que `capri chat test` mais avec le conseil de la
  fenêtre (« remets-la ci-dessus ») au lieu d'une commande ;
- **Utiliser ce cerveau** l'écrit dans `chat.json` : elle s'en sert dès ta phrase suivante.

**Voix** : la voix activée ou non, le choix parmi les voix françaises installées (avec leur
qualité ; « Automatique » : la meilleure), et « Écouter ». Mêmes réglages que `capri voice`
(`voice.json`).

La fenêtre relit les fichiers et le trousseau chaque fois qu'elle s'ouvre : un changement fait
en ligne de commande s'y voit à la réouverture.

Ce qui reste en ligne de commande (ou dans son menu) : brancher Claude Code (`capri hook`, ou
« Brancher Claude Code »), `capri ear status`, `capri chat status` (qui interroge aussi le
serveur), et sa personnalité (`persona.md`). Sous Windows, il n'y a pas encore de fenêtre de
réglages : « Réglages… » le dit, et tout passe par `capri chat` et `capri voice`.

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
  capri-core/      logique pure : animation, mouvement, lancer, hit-test, bandeau, parole, et la
                   description d'un formulaire de réglages (testée)
  capri-claude/    logique pure : hooks de Claude Code, protocole, sessions, settings.json (testée)
  capri-chat/      logique pure : conversation, fournisseurs et format « chat completions », flux
                   de réponse, règles d'adresse et de clé, réglages du cerveau, personnalité,
                   phrases à dire, dictée, et le modèle de la fenêtre de réglages (settings/ :
                   rubriques « Cerveau » et « Voix ») (testée)
  capri-net/       le seul crate qui parle au réseau : envoyer la conversation (en https par le TLS
                   du système), lire la réponse au fil de l'eau (testé contre un faux serveur local)
  capri-render/    rendu wgpu : Metal sur macOS, DirectX 12 + DirectComposition sur Windows
    sprite.wgsl    le shader : boil et copie violette
    banner.rs      le texte du bandeau et de la bulle, tracé avec les polices du système
  capri-platform/  la fenêtre « Overlay » : première des deux couches qui parlent au système
    macos.rs         AppKit via objc2 : un NSPanel non activant, sans fenêtre winit
    macos/input.rs   le champ de saisie : un des deux panneaux qui prennent le clavier, sur demande
    settings_window.rs la fenêtre de réglages, qui affiche un formulaire de capri-core ;
                     macos/settings_window.rs : un NSPanel à onglets (refus ailleurs)
    macos/edit_keys.rs ⌘X, ⌘C, ⌘V, ⌘A, ⌘Z dans les champs d'une app sans menu Édition
    voice.rs         sa voix ; macos/voice.rs : la synthèse vocale du système
    ear.rs           son oreille ; macos/ear.rs : le micro et la reconnaissance sur l'appareil
    keychain.rs      le trousseau des clés d'API ; macos/keychain.rs, windows/keychain.rs
    terminal.rs      taper une clé sans qu'elle s'affiche
    winit_overlay.rs fenêtre winit, pour Windows et les autres systèmes
    windows.rs       Win32 via windows-sys
    other.rs         repli (Linux : affichage seul)
  capri-ipc/       le canal local : seconde couche système, minuscule
    unix.rs          socket Unix dans un dossier réservé à l'utilisateur
    windows.rs       tube nommé au nom aléatoire
  capri-hook/      le binaire appelé par les hooks de Claude Code
  capri-app/       l'application : boucle, souris, menus, raccourci, images, bandeau, bulle,
                   voix, écoute, clés, la fenêtre de réglages (settings_view.rs : le modèle, la
                   fenêtre, et ses effets), et les commandes `capri hook`, `capri chat`,
                   `capri voice`, `capri ear`
tools/             récupération vérifiée des images, fabrication de Capri.app (macOS)
```

Règle : tout ce qui peut être testé sans écran va dans `capri-core`, `capri-claude` ou
`capri-chat`.

## État de validation

Vérifié automatiquement :

- formatage, `clippy -D warnings` et 303 tests sur **macOS Apple Silicon** (plus 3 qui ne
  tournent qu'à la main : ils écrivent des images, lisent un vrai `settings.json` ou touchent au
  vrai trousseau) : animation, lancer et rebonds, hit-test, clics, bandeau, parole en pages, bulle
  sur plusieurs lignes, conversation, **validation du shader** par le compilateur de wgpu, lecture
  des exemples de hooks de la documentation de Claude Code, suivi des sessions, aller-retour sur
  `settings.json`, socket (droits, connexions simultanées), et le binaire `capri-hook` lancé pour
  de vrai (sortie standard vide, code 0, durée bornée) ;
- le chat, sans sortir de la machine : le format des fournisseurs et le flux de réponse coupé à
  tous les endroits possibles, les adresses acceptées et refusées, le nettoyage du Markdown (ce
  qui est montré n'est jamais repris), et le client réseau contre un faux serveur local (requête
  envoyée, arrêt en cours de réponse, refus, redirection jamais suivie, serveur muet, réponse
  coupée ou sans fin) ;
- la clé, sans sortir de la machine : la règle « jamais en clair » adresse par adresse, son
  masquage dans un message (entière ou en morceaux), l'en-tête `Authorization: Bearer` tel que le
  voit un faux serveur local (présent une seule fois, jamais dans le corps, absent sans clé ou
  pour Ollama), aucun contact sans la clé exigée, les refus 401 et 403, et `https://` qui ne
  retombe jamais en clair face à un serveur qui répond en clair ; les commandes `capri chat` avec
  un trousseau en mémoire (la clé n'apparaît ni dans une sortie, ni dans une erreur, ni dans un
  fichier) ;
- la clé liée à son adresse : le nom de son compte (casse de l'hôte, port par défaut, IPv6, deux
  ports d'un même hôte), et, face à deux faux serveurs locaux qui ne diffèrent que par le port,
  une clé donnée pour l'un n'apparaît dans aucune requête vers l'autre (`use`, `models`, `test`,
  `status`, la conversation de l'app) ; `--url` sur un fournisseur nommé n'emporte pas la clé de
  son adresse habituelle ; une clé d'une version précédente, sans adresse, n'est jamais envoyée ;
- la voix, pour ce qui se teste sans haut-parleur : le découpage en phrases quel que soit le
  rythme d'arrivée du texte, ce qui se prononce ou non, et le choix de la voix ;
- l'écoute, pour ce qui se teste sans micro : le déroulé d'une prise de parole (texte provisoire,
  dernier mot attendu puis abandonné, silence, abandon) ;
- la fenêtre de réglages, sans fenêtre : son modèle seul (la destination dite dès qu'un
  fournisseur est choisi dans la liste, le champ secret jamais rempli, la clé ni dans le
  formulaire ni dans un effet affichable, liée à l'adresse affichée, jamais rangée pour une
  adresse jointe en clair ; aucun contact sans la clé exigée ; la liste demandée d'elle-même
  seulement à un serveur de cette machine ; des centaines de modèles cherchés et bornés à 30 ;
  un modèle « cloud » d'Ollama signalé ; les résultats périmés ignorés ; la voix), puis le modèle
  avec ce qui exécute ses effets, un trousseau en mémoire et un faux serveur local : une clé
  donnée dans la fenêtre est rangée pour l'adresse affichée et dans aucun fichier, ne part que
  vers elle, le cerveau choisi est celui que lit `capri chat`, l'essai dit les pannes de
  `capri chat test`, et la voix activée est celle que lit `capri voice` ;
- le fournisseur Ollama en ligne, et la phrase qui ne promet plus qu'un modèle « cloud » de
  l'Ollama local reste sur la machine ;
- `clippy -D warnings` en compilation croisée pour **Windows x64** et **Linux x64** (le code
  compile, il n'est pas exécuté).

**Vérifié en ligne de commande sur ce Mac** (le 10 novembre 2026, sans la clé d'un vrai
fournisseur) :

- `https://` par le TLS du système : la liste publique des modèles d'OpenRouter est lue ; des
  certificats expiré, auto-signé, à un autre nom et de racine inconnue (badssl.com) sont
  refusés ;
- une clé factice rangée dans le vrai trousseau (sous un service d'essai), tapée dans un
  pseudo-terminal sans s'afficher ou lue sur l'entrée standard, puis envoyée à un faux serveur
  local dans `Authorization: Bearer`, et retirée. Après la saisie, comme après un Ctrl+C pendant
  la saisie, l'écho du terminal est revenu ;
- la même clé, rangée pour un port : elle part vers ce port, et vers aucun autre (`use`,
  `models`, `test`, `status`) ; son compte s'appelle bien `custom@127.0.0.1:<port>` ; une clé
  rangée à l'ancienne, sans adresse, est signalée, jamais envoyée, et retirée par `--forget` ;
  une clé d'OpenRouter ne suit pas `--url` vers un autre serveur (refus, personne contacté) ;
- le test qui touche au vrai trousseau, lancé une fois à la main ;
- la fenêtre de réglages, sans la voir (binaire de débogage, `CAPRI_SETTINGS_REHEARSAL=1`, avec
  un dossier, un service de trousseau et une socket d'essai, quatre lancements de quelques
  secondes) : elle s'ouvre, elle est fenêtre clé, se refait pour chacun des sept fournisseurs et
  pour « Aucun » sans planter ; une clé factice collée dans son vrai champ secret est rangée dans
  le trousseau d'essai pour `openrouter@openrouter.ai:443`, relue (« Une clé est rangée pour
  openrouter.ai:443… »), puis retirée ; elle n'apparaît ni dans le journal ni dans le dossier, et
  le trousseau d'essai est vide après. Le processus quitte de lui-même (code 0).

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
14. depuis `Capri.app`, macOS demande le micro et la reconnaissance vocale à la première écoute ;
    en maintenant ⌃⌥D, la bulle bleue montre ce qu'elle comprend, la phrase part au relâchement,
    elle se tait quand on lui coupe la parole, et un appui sans rien dire ne lui envoie rien.
15. la fenêtre de réglages s'ouvre devant sans figer Capri ; ⌘V colle dans ses champs et dans le
    champ ⌃⌥P ; une clé enregistrée vide son champ et n'y revient pas avec ⌘Z ; l'avertissement
    d'envoi apparaît pour un fournisseur distant ; la liste de modèles se filtre ; « Essayer » et
    « Utiliser ce cerveau » fonctionnent ; le clavier revient à l'app d'avant à la fermeture.

**Pas encore vérifié** : une vraie conversation avec un fournisseur à clé (OpenRouter, DeepSeek,
ProxyAPI, Ollama en ligne) ; un vrai LM Studio ; la bulle avec un fournisseur distant ; la demande
d'accès au trousseau quand `Capri.app` lit une clé rangée depuis le terminal.

**À vérifier à la main : ⌘Q et ⌘H** (le choix du raccourci est testé, pas la fenêtre) :

1. fenêtre de réglages ouverte, ⌘Q : seule la fenêtre se ferme, Capri continue de flotter et le
   clavier revient à l'app d'avant ; idem pour ⌘H (Capri n'est pas masquée) ;
2. dans le champ ⌃⌥P, ⌘Q et ⌘H ferment seulement le champ, comme Échap ; Capri reste là ;
3. sans fenêtre ni champ ouverts, rien ne change : « Quitter » dans son menu quitte Capri ;
4. un message de panne dans la bulle (clé absente ou refusée) dit d'ouvrir « Réglages… », et le
   même dans la fenêtre dit « remets-la ci-dessus », sans commande.

**Pas encore testé sur un vrai PC Windows** : il n'y a pas encore de fenêtre de réglages. Le tube
nommé de `capri-ipc` n'y a jamais tourné, le champ de saisie et la voix n'y existent pas encore,
et le chat n'y a jamais été lancé : ni le https (SChannel), ni le Gestionnaire d'identification,
ni la saisie d'une clé sans écho n'y ont tourné.

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
4. Le chat avec un modèle d'IA : ~~la saisie et la bulle~~, ~~un modèle local par Ollama~~,
   ~~les fournisseurs à clé compatibles OpenAI (OpenRouter, DeepSeek, ProxyAPI, Ollama en ligne)
   avec leurs clés dans le trousseau~~, ~~une fenêtre de réglages (macOS)~~, puis l'API
   d'Anthropic, et Capri dans Claude Code pour qui a un
   abonnement. Sa voix : ~~ses réponses dites à voix haute~~, ~~lui parler au micro~~
   (reconnaissance sur la machine), puis un mot d'éveil.
5. App signée et notarisée (`.app` / installeur), mises à jour.

## Licence

Licence du **code** à choisir (MIT ou Apache-2.0 par exemple). Les **images** n'en relèvent pas :
voir [CREDITS.md](CREDITS.md).
