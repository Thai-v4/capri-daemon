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
./target/release/capri hook status         # où en sont le branchement, le miroir et le style
./target/release/capri hook uninstall      # le débranche
```

Le menu de Capri propose la même bascule (« Brancher / Débrancher Claude Code »), et sa fenêtre
de réglages aussi (onglet « Claude Code »). Les sessions déjà ouvertes prennent le changement en
compte toutes seules ; `/hooks` dans Claude Code liste les entrées `capri-hook`.

- **Aucun modèle n'est appelé, aucune clé n'est nécessaire** : tout vient des
  [hooks](https://code.claude.com/docs/en/hooks) de Claude Code, quel que soit ton abonnement.
- **Rien ne sort de ta machine** : le hook parle à Capri par une socket locale réservée à ton
  utilisateur. Il ne transmet ni tes prompts, ni le contenu de tes fichiers, ni la sortie de tes
  commandes : seulement le nom de l'outil et un résumé court (le nom d'un fichier, la description
  d'une commande). Une seule exception, si tu l'actives : le miroir, qui transmet la ligne que
  Claude écrit pour Capri, et rien d'autre de sa réponse ([plus bas](#capri-dans-claude-code)).
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

## Jalon 3 : elle s'assoit sur tes fenêtres

**Lâche-la doucement** les pieds près du haut d'une fenêtre (à moins de 45 points au-dessus ou en
dessous de son bord) : elle s'y pose, s'écrase un instant sur ses pieds, rebondit, puis reste
immobile (elle ne flotte plus ; seul son trait continue de « bouillir »). **Lance-la** : là où elle
s'arrête, c'est la même règle. Posée, elle **suit la fenêtre** quand tu la déplaces ou la
redimensionnes, à la même place le long du bord (tirée par un seul côté, la fenêtre ne l'emmène pas :
le bord sous ses pieds n'a pas bougé), sans quitter le bord ni l'écran. Elle **se remet à flotter
sur place**, avec un petit étirement, quand :

- sa fenêtre n'est plus à l'écran (fermée, réduite dans le Dock, app masquée, autre bureau) ;
- une autre fenêtre passe devant le morceau de bord sous ses pieds ;
- le bord sort de l'écran, ou ne lui laisse plus la place au-dessus de lui (plein écran) ;
- tu l'attrapes, ou tu décoches « S'asseoir sur les fenêtres ».

Il n'y a qu'un dessin d'elle, debout : « assise » veut dire posée sur le bord. Ne comptent que les
fenêtres d'app ordinaires (ni menus, ni Dock, ni panneaux flottants), à l'écran, opaques, d'au moins
90 × 45 points, sur son écran (celui qui porte la barre des menus), et pas les siennes ; et seulement
les parties visibles de leurs bords : une fenêtre devant cache le bord de celle de derrière.

**Ce qu'elle lit, et ce qu'elle ne lit jamais.** Aucune permission : ni Accessibilité, ni
Enregistrement de l'écran. Sur macOS, elle lit la liste des fenêtres de CoreGraphics
(`CGWindowListCopyWindowInfo`), et de chaque fenêtre seulement son numéro, son rectangle, sa couche,
son opacité, et si elle est à Capri. **Jamais le titre d'une fenêtre ni le nom d'une app** : ni lus,
ni gardés, ni écrits dans le journal, qui ne dit que des nombres. Elle ne garde que des numéros de
fenêtre, des rectangles et leur ordre, le temps de décider.

**Ce que ça coûte.** La liste n'est lue que quand elle sert : une fois quand elle s'arrête ; posée,
4 fois par seconde quand rien ne bouge, 24 fois par seconde pendant que sa fenêtre bouge ou qu'un
bouton de la souris est enfoncé. Jamais quand elle est cachée, tenue ou en vol, ni quand c'est
désactivé. Une lecture prend 0,16 à 0,17 ms en médiane et 0,25 à 0,4 ms en moyenne (mesuré sur un
MacBook Pro M5 Pro, avec 17 à 19 fenêtres à l'écran) : 1 à 1,6 ms par seconde posée au repos, 6 à
10 ms par seconde pendant un déplacement, soit au plus un centième d'un cœur.

**Le désactiver** : décoche « S'asseoir sur les fenêtres » dans son menu (clic droit sur elle, ou son
icône dans la barre des menus). C'est retenu d'un lancement à l'autre (`perch.json`, dans son
dossier) ; activé par défaut.

Limites connues :

- elle mesure 300 points : il lui faut 300 points de place au-dessus du bord. Une fenêtre agrandie,
  ou dont le haut est près de la barre des menus, ne peut pas la porter ;
- elle vit sur l'écran qui porte la barre des menus : les fenêtres d'un autre écran ne comptent pas,
  et elle repart quand sa fenêtre quitte son écran ;
- une fenêtre fermée, réduite ou passée devant elle au clavier se remarque au plus tard un quart de
  seconde après ; une fenêtre déplacée à la souris, elle la suit au rythme de ses dessins (24 fois
  par seconde), donc avec un léger retard ;
- une fenêtre que le système dit opaque mais qui est transparente par endroits compte comme pleine ;
- Windows : écrit et compilé, jamais exécuté (voir plus bas) ; Linux : elle flotte comme avant.

### Prêt, pas encore branché : une vraie pose assise

`tools/fetch-assets.sh` télécharge aussi, dans `assets/capri/perch/`, les planches de sa pose
assise dessinées par l'artiste du projet [Capri](https://github.com/lachydotmcg/capri-desktop)
(jambes pendantes, clignement, parole, musique, arrivée et départ ; voir `CREDITS.md`, usage non
commercial seulement). Leur description (cases, dessins tenus en deux, ligne du bord de la fenêtre)
est dans `capri_core::sheets`, leur chargement dans `assets::perch::load` : sans elles, elle se pose
debout comme aujourd'hui. **Rien ne les affiche encore.**

De même, sur macOS, elle sait déjà dire si **un son sort d'une autre app** (`capri_platform::music`,
décidé par `capri_core::music`), pour se balancer quand tu mets de la musique : **un oui ou un non,
rien d'autre**. Elle demande à CoreAudio quelles apps ont un flux de sortie en marche, sans aucune
permission et sans jamais lire le titre, l'artiste ni le nom de l'app. Comptent : un lecteur, une
vidéo, un jeu ; ne comptent pas : elle-même, une app d'appel qui a le micro ouvert, un son de moins
de 3 secondes ; une pause de moins de 8 secondes entre deux morceaux ne l'arrête pas. Une app qui
garde sa sortie ouverte sans rien jouer compte à tort, le volume coupé aussi. Une écoute coûte
0,07 à 0,2 ms en médiane quand aucun appareil audio ne tourne, 4,3 ms quand il faut interroger tous
les clients du son (mesuré sur un MacBook Pro M5 Pro, 42 clients) ; à une écoute par seconde, au
plus un demi-centième d'un cœur.
Windows et Linux : elle ne sait pas. **Pas encore branché** non plus.

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

**L'API d'Anthropic** (Claude comme cerveau) : elle n'est pas compatible OpenAI, Capri parle son
format à elle (en-tête `x-api-key`, version `2026-10-21`, corps et flux propres). Il faut une
**clé d'API**, créée dans la [Console d'Anthropic](https://platform.claude.com), facturée à
l'usage par Anthropic : Capri ne se sert jamais de ton abonnement Claude, ni de Claude Code, ni
d'un jeton de connexion, ni des variables `ANTHROPIC_API_KEY` ou `ANTHROPIC_AUTH_TOKEN`. La clé
se donne et se range comme les autres :

```bash
./target/release/capri chat key anthropic                 # rangée pour api.anthropic.com:443
./target/release/capri chat models anthropic <mot>        # les modèles que ta clé peut utiliser
./target/release/capri chat use anthropic --model <nom>   # aucun nom n'est écrit dans Capri
./target/release/capri chat test
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
| `anthropic`  | `https://api.anthropic.com/v1` | obligatoire (une clé d'API, pas l'abonnement) |
| `custom`     | à donner avec `--url`          | facultative                                 |

`--url` remplace l'adresse de n'importe lequel, par exemple `--url http://192.168.1.20:11434/v1`
pour un Ollama qui tourne ailleurs chez toi. La clé ne suit pas : celle de l'adresse habituelle
n'est jamais envoyée à une autre. Les adresses viennent de la documentation de chaque
fournisseur, relue le 10 novembre 2026 (le 11 pour Anthropic).

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
- un modèle écrit volontiers du Markdown : les signes sont retirés à l'affichage, les mots restent ;
- **avec Anthropic** : chaque message demande au plus 16 000 jetons de réponse (`max_tokens`,
  obligatoire chez lui : la réflexion du modèle compte dedans), sous le plafond de chacun des
  modèles d'après sa documentation (64 000 au moins) ; seuls les jetons produits sont facturés.
  Capri n'envoie ni `temperature`, ni réglage de la réflexion : les modèles récents réfléchissent
  d'eux-mêmes, sans rien en montrer, et la bulle reste sur « … » pendant ce temps. Si le modèle
  décline une demande, elle le dit (« Le modèle d'Anthropic a décliné cette demande. Essaie de la
  dire autrement. ») ; ce qu'il avait déjà écrit reste affiché, mais n'entre pas dans la
  conversation. Pas de repli vers un autre modèle : Anthropic ne le propose que pour certains
  modèles. Une réponse arrivée au plafond est gardée telle quelle, comme chez les autres.

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

#### Ses effets de voix

Capri est une construct, une conscience numérique : plutôt que d'imiter une voix, on le fait
entendre par le son. Un **effet** est un traitement appliqué à sa voix **après** la synthèse, sur
ta machine, avec les unités d'effet du système (réverbération, délai, distorsion, égaliseur
d'AVFAudio). Ce n'est ni une autre voix, ni une voix téléchargée, ni un modèle : la même voix du
système, passée dans une chaîne de traitements, en mémoire. Rien n'est enregistré ni envoyé.

```bash
./target/release/capri voice effect construct   # natural (aucun), construct, radio, hologram
./target/release/capri voice intensity 2        # 1 légère, 2 moyenne, 3 forte
./target/release/capri voice pitch -1           # sa hauteur, de -2 (plus grave) à 2 (plus aiguë)
./target/release/capri voice rate 1             # son débit, de -2 (lent) à 2 (rapide)
./target/release/capri voice status             # tout ce qui précède, en clair
./target/release/capri voice test               # l'écouter, effet compris
```

Ou, sans terminal, dans l'onglet « Voix » de sa fenêtre de réglages : quatre listes, et
« Écouter ». Les deux écrivent le même `voice.json`, avec les mêmes noms et les mêmes règles.
Choisi dans la fenêtre, le changement vaut dès sa phrase suivante, même au milieu d'une réponse ;
en ligne de commande, dès sa réponse suivante.

| effet | ce qui le compose | ce qu'on devrait entendre |
| ----- | ----------------- | ------------------------- |
| **Naturelle** (par défaut) | rien | la voix du système, exactement comme avant les effets |
| **Construct** | un peu de son « décimé » (moins de bits, moins d'échantillons), une bosse de présence vers 2,8 kHz, une petite salle | un léger grain électronique et un peu d'espace, la voix restant nette |
| **Liaison radio** | une légère saturation, une bande étroite (environ 400 Hz à 3,4 kHz en intensité moyenne, plus étroite si elle monte), une bosse vers 1,7 kHz | une voix serrée et un peu saturée, comme dans un casque de liaison ; un peu moins forte que les autres |
| **Hologramme** | du son décimé, des aigus relevés, des échos très rapprochés (22 ms) et réinjectés, une réverbération de plaque | plus marqué : un timbre métallique, en peigne, du grain et une réverbération brillante |

L'**intensité** (légère, moyenne, forte) dose ce qui est mélangé au son d'origine (×0,6, ×1, ×1,5)
et, pour la radio, resserre la bande. Elle n'agit pas sur « Naturelle ». La **hauteur** et le
**débit** ont cinq crans chacun, normaux par défaut ; ils valent avec ou sans effet. Les valeurs
viennent de mesures sur les voix de ce Mac (sans les écouter : fréquence fondamentale et durée
d'une même phrase) : le système monte la hauteur fidèlement mais la descend moins qu'annoncé sur
les voix de base (« plus grave » la baisse d'environ 12 %, « plus aiguë » la monte de 14 %), et
son débit n'est pas linéaire (« lent » : environ ×0,8, « rapide » : environ ×1,25). Une voix
téléchargée peut y répondre autrement.

Les préréglages sont les nôtres. L'idée d'effets de lecture vient de CapriChooms/CapriCore
(GPL-3.0) : ni son code ni ses valeurs ne sont repris.

Ce que Capri garantit :

- **« Naturelle » sonne comme avant** : la phrase suit exactement le même chemin qu'avant les
  effets. À la hauteur et au débit normaux, rien d'autre que la voix n'est demandé au système
  (même une valeur égale à la normale change le rendu de certaines voix : mesuré) ;
- **une panne des effets ne la rend jamais muette** : si la chaîne ne peut pas être montée, si le
  son de la voix est dans un format inattendu, si le moteur audio ne démarre pas ou s'arrête (un
  casque qu'on branche, par exemple), si le son ne sort pas ou si le système ne rend rien, ce qui
  n'a pas été joué jusqu'au bout est redit sans effet, avec une ligne dans le journal. Les effets
  sont retentés à sa réponse suivante ;
- tout ce que fait sa voix marche aussi avec effets : les phrases s'enchaînent dans l'ordre pendant
  que la réponse arrive, « La faire taire » coupe net (la queue de réverbération comprise), la
  cacher met en pause et la montrer reprend, et la ligne de Claude Code attend qu'elle ait fini ;
- le moteur audio ne tourne que pendant qu'elle parle, plus une seconde et demie pour la queue
  d'une réverbération.

Limites :

- **personne n'a encore entendu ces effets** : ils ont été composés et mesurés sans être écoutés
  (voir plus bas ce qu'il faut vérifier à l'oreille) ;
- avec un effet, une phrase est d'abord rendue en mémoire, puis jouée : le début peut arriver une
  fraction de seconde plus tard que sans effet. Le passage de la voix mono au moteur stéréo baisse
  chaque canal d'environ 1,4 dB par rapport au son rendu (mesuré hors ligne) ; comparé au chemin
  direct, le niveau n'a pas été mesuré ;
- si le moteur s'arrête au milieu d'une phrase, cette phrase est redite depuis son début, sans
  effet ;
- Rust ne sait pas attraper une exception Objective-C, que lève AVFAudio sur un format ou un
  branchement invalide : chaque condition documentée est vérifiée avant l'appel, mais un
  changement de sortie audio à l'instant exact où le lecteur démarre reste un risque (non
  observé) ;
- macOS seulement, comme sa voix.

### Lui parler au micro

Deux façons de lui parler avec **⌃⌥D** (Ctrl+Alt+D, comme « dicter »). Ce que tu as dit lui est
envoyé comme si tu l'avais tapé, et une bulle bleue montre ce qu'elle comprend pendant que tu
parles.

- **un appui bref** (relâché en moins d'un tiers de seconde), puis tu parles : le micro reste ouvert, et c'est
  elle qui comprend que tu as fini et l'envoie. Tu peux aussi finir toi-même, en appuyant une
  seconde fois : elle envoie tout de suite ce qu'elle a compris ;
- **maintenir** la touche, parler, relâcher : comme avant, c'est le relâchement qui termine.

Le menu (« Lui parler au micro ») fait comme l'appui bref : un clic pour commencer, un second pour
finir tout de suite.

```bash
./tools/bundle-macos.sh                   # fabrique target/Capri.app
open target/Capri.app                     # quitte d'abord une Capri déjà lancée
./target/release/capri ear status         # si elle peut t'écouter, et sinon pourquoi
```

- **le son ne quitte pas ta machine** : la transcription est faite par la reconnaissance vocale
  de macOS, exigée sur l'appareil. Si le système ne sait pas transcrire le français sans passer
  par ses serveurs, Capri refuse d'écouter plutôt que d'envoyer ta voix ;
- **le micro ne s'ouvre que sur un geste de ta part** (le raccourci ou le menu), **et il se
  referme toujours tout seul** (le témoin orange de macOS le montre), et rien n'est enregistré.
  Il se ferme : quand tu relâches la touche maintenue ; quand tu appuies une seconde fois ; en
  écoute libre, quand elle comprend que tu as fini (**1,5 s** sans nouveau texte après des mots
  compris) ou qu'il n'y a rien à entendre (**7 s** sans un seul mot : rien n'est envoyé) ; et, dans
  tous les cas, au bout de **55 secondes** au plus (Apple limite une reconnaissance à une minute).
  Elle n'écoute jamais en permanence, n'a pas de mot d'éveil, et ne rouvre jamais le micro d'elle-même,
  pas même après avoir répondu : il faut un nouveau geste ;
- **elle ne mesure pas le son** pour détecter la fin : elle regarde quand le texte compris a cessé
  de changer. Si tu fais de longues pauses en parlant, elle peut t'envoyer avant la fin de ta
  phrase : maintiens alors la touche, qui ne coupe jamais sur un silence. Ce que tu dis n'est
  jamais écrit dans le journal ;
- **il faut la lancer depuis `Capri.app`** : macOS n'accorde le micro et la reconnaissance vocale
  qu'à une app qui dit pourquoi elle les veut. À la première écoute, il te pose les deux
  questions ; réponds, puis recommence. Lancée nue (`cargo run`), Capri garde tout le reste et dit
  simplement qu'elle ne peut pas t'écouter ;
- l'app est signée pour ta machine seulement, et macOS rattache ses permissions à cette
  signature : après chaque reconstruction, il les redemande ;
- quand tu commences à parler, elle se tait. Le micro met un instant à s'ouvrir : laisse passer
  une demi-seconde avant ton premier mot ;
- les délais (appui bref, silence, attente d'un premier mot, durée maximale) sont des constantes
  de `capri_chat::dictation`, pas des réglages. macOS seulement.

### Capri dans Claude Code

Pour qui a un abonnement Claude : **Capri ne lance jamais Claude Code et ne se sert jamais de ton
abonnement** (Anthropic n'autorise pas une app tierce à le faire ; pour lui donner Claude comme
cerveau, il faut une clé d'API : voir [plus haut](#lui-donner-un-cerveau)). C'est l'inverse : tu
parles à Claude Code dans tes propres sessions, comme d'habitude ; il y prend la personnalité de
Capri (un [style de sortie](https://code.claude.com/docs/en/output-styles)) et finit chaque
réponse par une ligne pour elle ; Capri, sur ton bureau, la dit dans sa bulle, et à voix haute si
sa voix est activée (le **miroir**).

```bash
./target/release/capri hook install          # le bandeau, s'il n'est pas déjà branché
./target/release/capri hook style install    # le style, dans ~/.claude/output-styles/capri.md
./target/release/capri hook mirror on        # le miroir, désactivé par défaut
./target/release/capri hook status           # où en sont les trois
```

Puis, **dans Claude Code** : `/output-style Capri` (ou `/config`, puis *Output style*). Tout cela
se fait aussi sans terminal, dans l'onglet « Claude Code » de sa fenêtre de réglages.

**Le style.** Ses consignes sont les nôtres, pas une citation de la série : Claude Code reste un
outil de travail (`keep-coding-instructions: true` garde ses consignes de développement), tutoie,
parle comme Capri, et termine chaque réponse par une ligne seule qui commence par `[Capri]`, une
ou deux phrases courtes en français, faites pour être dites, sans nom de fichier, commande ni
résultat. Pourquoi `[Capri]` : un mot entre crochets en tête de ligne est simple à écrire pour un
modèle et n'ouvre jamais par hasard une ligne de prose, de liste ou de titre ; « Capri : »
pourrait commencer n'importe quelle phrase d'une session qui travaille justement sur Capri.

- Capri n'écrit le style que si tu le demandes, dans le dossier de Claude Code (celui de
  `CLAUDE_CONFIG_DIR` si tu l'as définie), d'un seul coup. Elle ne touche jamais à un fichier
  `capri.md` qui n'est pas le sien (le sien porte une ligne `# capri-managed` dans son en-tête ;
  retire-la pour garder le fichier à toi). `capri hook style uninstall` ne retire que le sien.
- **Capri ne l'active pas**, et ne touche pas à ton `settings.json` pour cela : c'est toi qui le
  choisis. D'après la documentation de Claude Code, ce choix vaut pour le projet où tu le fais
  (il est rangé dans son `.claude/settings.local.json`), `/output-style default` revient au style
  habituel, et une session ouverte avant l'installation ne voit le style qu'une fois relancée.
  Pour l'avoir dans tous tes projets, c'est `"outputStyle": "Capri"` dans ton
  `~/.claude/settings.json`, à écrire toi-même.
- Un style est une consigne, pas une garantie (la documentation le dit) : si Claude oublie sa
  ligne, Capri ne dit rien. Ses consignes partent avec chaque requête (le cache des prompts en
  réduit le coût). `keep-coding-instructions` n'agit qu'avec le prompt système complet de Claude
  Code ; selon la documentation, `CLAUDE_CODE_SIMPLE_SYSTEM_PROMPT=0` l'impose.

**Le miroir** est désactivé par défaut. Activé, à la fin de chaque réponse de Claude Code (dans la
session principale, pas dans un sous-agent), le hook cherche la dernière ligne qui commence par
`[Capri]` (hors des blocs de code) et ne transmet qu'elle, bornée à 240 caractères, par le même
canal local que le bandeau. C'est la seule exception à « le hook ne transmet rien de ce
qu'écrit Claude », et elle est étroite :

- activé par toi seulement (`capri hook mirror on`, ou la case de la fenêtre) ; désactivé, le hook
  ne lit même pas la réponse de Claude, et ce qu'il envoie est exactement ce qu'il envoyait avant ;
- une seule ligne, aucune autre partie de la réponse ; rien n'est gardé, ni envoyé ailleurs, ni
  écrit dans le journal (sa longueur seulement) ; elle n'entre pas dans la conversation que Capri
  envoie à son propre cerveau ;
- **ce qu'on ne peut pas garantir** : cette ligne est écrite par Claude, qui pourrait y citer un
  nom de fichier ou un bout de résultat malgré les consignes du style. Elle peut être lue à voix
  haute.

Capri la dit comme une réponse : dans sa bulle, à voix haute si sa voix est activée, et « La
faire taire » la coupe. Ce que tu lui dis (au clavier ou au micro) et ce qu'elle te répond passent
avant : une ligne arrivée pendant qu'elle parle attend qu'elle ait fini. Seule la plus récente
attend, deux minutes au plus, et elle est oubliée si tu envoies un nouveau prompt dans la même
session, si tu la fais taire, ou si tu désactives le miroir. Cachée, elle ne la dit pas. Sans
ligne, le bandeau « Claude a fini » ne change pas.

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
qualité ; « Automatique » : la meilleure), l'effet (chacun avec ce qu'on devrait entendre), son
intensité (éteinte pour « Naturelle »), la hauteur et le débit, une note qui dit ce que sont les
effets et ce qu'ils ne sont pas, et « Écouter », qui fait entendre ce qui est choisi. Mêmes
réglages que `capri voice` (`voice.json`) ; un changement vaut dès sa phrase suivante.

**Claude Code** : où en est le branchement du hook (dans quel `settings.json`, combien
d'événements, un binaire `capri-hook` introuvable, `disableAllHooks`), avec « Brancher » et
« Débrancher », qui font exactement ce que font `capri hook install` et `capri hook uninstall`
(sauvegarde datée, réécriture à l'octet près ou refus) ; le style de sortie, avec « Installer le
style Capri » et « Retirer », et ce qu'il faut taper dans Claude Code pour le choisir ; la case du
miroir, avec ce qu'elle fait, dit avant qu'on la coche, et ce qui manque encore pour qu'une ligne
arrive (le hook pas branché, le style pas installé). Le menu et la fenêtre restent d'accord.

La fenêtre relit les fichiers et le trousseau chaque fois qu'elle s'ouvre : un changement fait
en ligne de commande s'y voit à la réouverture.

Ce qui reste en ligne de commande : `capri ear status`, `capri chat status` (qui interroge aussi
le serveur), et sa personnalité (`persona.md`). Sous Windows, il n'y a pas encore de fenêtre de
réglages : « Réglages… » le dit, et tout passe par `capri chat`, `capri voice` et `capri hook`.

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
  capri-core/      logique pure : animation, mouvement, lancer, hit-test, bandeau, parole, la
                   description d'un formulaire de réglages, et celle de comment dire une phrase
                   (sound.rs : voix, hauteur, débit, traitements du son, leurs bornes), et où se
                   poser sur les fenêtres (perch.rs : repères, bords visibles, accroche, suivi,
                   départ, cadence de lecture) (testée)
  capri-claude/    logique pure : hooks de Claude Code, protocole, sessions, settings.json, style
                   de sortie « Capri », miroir, dossiers de Capri et de Claude Code (testée)
  capri-chat/      logique pure : conversation, fournisseurs et format « chat completions », flux
                   de réponse, règles d'adresse et de clé, réglages du cerveau, personnalité,
                   phrases à dire, dictée, les effets de voix (voice.rs : préréglages,
                   intensité, hauteur, débit, voice.json), et le modèle de la fenêtre de
                   réglages (settings/ : rubriques « Cerveau », « Voix » et « Claude Code »)
                   (testée)
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
    voice.rs         sa voix ; macos/voice.rs : la synthèse vocale du système, chemin direct et
                     chemin traité dans une seule file, et le repli ;
                     macos/voice_stage.rs : la chaîne d'effets (AVAudioEngine)
    ear.rs           son oreille ; macos/ear.rs : le micro et la reconnaissance sur l'appareil
    desk.rs          la liste des fenêtres des autres apps, sans permission ; macos/desk.rs
                     (CoreGraphics), windows/desk.rs (Win32 et DWM, jamais exécuté)
    keychain.rs      le trousseau des clés d'API ; macos/keychain.rs, windows/keychain.rs
    terminal.rs      taper une clé sans qu'elle s'affiche
    winit_overlay.rs fenêtre winit, pour Windows et les autres systèmes
    windows.rs       Win32 via windows-sys
    other.rs         repli (Linux : affichage seul)
  capri-ipc/       le canal local : seconde couche système, minuscule
    unix.rs          socket Unix dans un dossier réservé à l'utilisateur
    windows.rs       tube nommé au nom aléatoire
  capri-hook/      le binaire appelé par les hooks de Claude Code (et qui relit le réglage du
                   miroir)
  capri-app/       l'application : boucle, souris, menus, raccourci, images, bandeau, bulle,
                   voix, écoute, clés, la fenêtre de réglages (settings_view.rs : le modèle, la
                   fenêtre, et ses effets), le style de sortie (style_setup.rs), et les commandes
                   `capri hook` (branchement, miroir, style), `capri chat`, `capri voice`,
                   `capri ear` ; en version de débogage, `capri voice rehearse`
                   (voice_rehearsal.rs : la vraie voix exercée sans bruit, lancée par
                   tests/voix_en_silence.rs) ; s'asseoir sur les fenêtres (perch_view.rs, et
                   perch_setup.rs pour perch.json), et en version de débogage `capri perch
                   rehearse` (perch_rehearsal.rs : la vraie liste des fenêtres, sans rien
                   afficher, lancée par tests/fenetres_du_bureau.rs)
tools/             récupération vérifiée des images, fabrication de Capri.app (macOS)
```

Règle : tout ce qui peut être testé sans écran va dans `capri-core`, `capri-claude` ou
`capri-chat`.

## État de validation

Vérifié automatiquement :

- formatage, `clippy -D warnings` et 423 tests sur **macOS Apple Silicon** (plus 6 qui ne
  tournent qu'à la main : ils écrivent des images, lisent un vrai `settings.json`, touchent au
  vrai trousseau, créent de vraies voix et un vrai moteur audio, sans bruit, ou lisent la vraie
  liste des fenêtres) : animation, lancer et rebonds, hit-test, clics, bandeau, parole en pages, bulle
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
- les effets de voix, pour ce qui se teste sans haut-parleur : la description neutre (chaque
  réglage ramené dans les bornes d'Apple, ce qui n'est pas un nombre rendu neutre, traitements et
  bandes en nombre limité) ; les échantillons du système remis en canaux séparés (entiers ou
  flottants, entrelacés ou non, plus de deux canaux ramenés à un) ; les préréglages (« Naturelle »
  sans aucun traitement, les autres dans leurs bornes, plus d'effet à mesure que l'intensité
  monte, aucun identique à un autre) ; `voice.json` (un fichier d'avant les effets donne la voix
  d'avant, un nom inconnu ou un cran hors bornes ne casse rien) ; la rubrique « Voix » et
  `capri voice` (mêmes noms, même fichier, chaque choix garde les autres, « Écouter » fait entendre
  ce qui est choisi) ; un changement fait dans la fenêtre pris en compte dès la phrase suivante ;
- l'écoute, pour ce qui se teste sans micro : le déroulé d'une prise de parole (texte provisoire,
  dernier mot attendu puis abandonné, silence, abandon), et sur une horloge factice le geste bref
  contre le geste maintenu, la fin de phrase après un silence, l'attente d'un premier mot, la
  durée maximale pour les deux gestes ;
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
- l'API d'Anthropic, **contre un faux serveur local seulement** (aucun appel à Anthropic, avec ou
  sans clé) : la requête (`POST /v1/messages`, `x-api-key` une seule fois et jamais dans le corps,
  pas d'`Authorization`, `anthropic-version: 2026-10-21`, `system` à la racine, `max_tokens`, ni
  `temperature` ni `thinking`), la conversation toujours valable (rôles fusionnés, pas de réponse
  en tête ni en dernier, sans rien changer de ce qui part vers les autres fournisseurs) ; le flux
  (texte des blocs `text` seulement ; réflexion, `ping` et événements inconnus ignorés ; un `error`
  en plein flux expliqué et masqué ; un refus dit par sa phrase ; une réponse au plafond gardée,
  ou « n'a rien répondu » si la réflexion a tout pris) ; les refus 401 (le moindre morceau de la
  clé masqué), 403, 404, 429, 529 ; la liste des modèles page par page, avec sa clé ; sa clé ne part
  ni vers une autre adresse ni à un autre fournisseur ; `capri chat key`, `use`, `test`, `models`,
  la bulle et la fenêtre (rien demandé d'elle-même à `api.anthropic.com`) ;
- Capri dans Claude Code, sans Claude Code : le style (son en-tête, `keep-coding-instructions:
  true`, sa signature reconnue dans l'en-tête seulement ; installé, mis à jour, retiré, jamais
  par-dessus un fichier qui n'est pas le nôtre, un dossier ou un lien) ; l'extraction de la ligne
  `[Capri]` (la dernière, hors des blocs de code, pas un lien `[Capri](…)`, bornée en caractères
  et en octets sur une frontière de caractère, dans une réponse de plusieurs Mio) ; le hook lancé
  pour de vrai, miroir désactivé (ligne identique à celle d'avant le miroir), activé (seule la
  ligne marquée de la session principale part ; ni `SubagentStop`, ni un `Stop` de sous-agent),
  réglage cassé, trop gros, dossier ou tube nommé à sa place, entrée jamais fermée : toujours
  muet, code 0, borné ; le protocole lu par une copie de l'ancien, et l'inverse ; la phrase dite
  sans entrer dans la conversation envoyée à son cerveau (faux serveur local), la plus récente
  seulement, oubliée quand sa session repart ; l'onglet « Claude Code » de la fenêtre, modèle
  seul puis avec ses effets (le même `settings.json` que `capri hook`, rendu à l'octet près, la
  même sauvegarde, le même fichier du miroir) ;
- s'asseoir sur les fenêtres, sans écran : le passage du repère des fenêtres à celui de l'overlay
  (AppKit depuis le bas, CoreGraphics depuis le haut, un écran au-dessus de l'écran principal, les
  pixels de Windows) ; le tri (à elle, pas ordinaire, peu opaque, ce qui n'est pas un nombre, trop
  petite, sur un autre écran) ; les bords visibles (cachés par une fenêtre devant, pas par une
  fenêtre derrière, ni par une qui le touche par en dessous ; coupés au bord de l'écran) ;
  l'accroche (au-dessus et en dessous du bord, trop loin, le bord le plus proche, un morceau caché
  ou trop court, ses pieds rentrés loin des coins et dans l'écran, la place de ses pieds dans son
  image) ; le suivi (immobile, déplacée, tirée par la gauche, rétrécie par la droite, changée des
  deux côtés, à moitié hors de l'écran) ; le départ (fenêtre disparue,
  bord recouvert sous ses pieds ou à moitié, plein écran, trop haute, sortie par le bas ou sur un
  autre écran, une place qui n'est pas un nombre) ; la cadence de lecture ; sa pose (l'écrasement
  et le rebond tenus 3 et 2 images, ses pieds immobiles, puis plus aucun flottement ; l'étirement
  au départ ; attrapée, elle quitte le bord) ; `perch.json` (activé sans fichier, retenu, un
  fichier étrange signalé) ;
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

**Vérifié sur ce Mac, pour l'API d'Anthropic** (le 11 novembre 2026, sans clé et sans contacter
Anthropic) : la répétition des réglages (binaire de débogage, `CAPRI_SETTINGS_REHEARSAL=1`, un
dossier, un service de trousseau et des réglages de Claude Code d'essai) se refait pour chacun des
huit fournisseurs et pour « Aucun » sans planter ; une clé factice collée dans le vrai champ secret
est rangée dans le trousseau d'essai pour `anthropic@api.anthropic.com:443` (« Une clé est rangée
pour api.anthropic.com:443… »), puis retirée ; elle n'apparaît ni dans le journal ni dans le
dossier, et le trousseau d'essai est vide après. Le processus quitte de lui-même (code 0).

**Vérifié en ligne de commande sur ce Mac, pour Capri dans Claude Code** (le 10 et le 11 novembre
2026, avec des dossiers d'essai, sans jamais lancer Claude Code) :

- `capri hook status`, `capri hook style install | uninstall` et `capri hook mirror on | off` :
  le style écrit puis retiré, un `capri.md` étranger refusé et laissé intact, le miroir écrit dans
  `claude-code.json` du dossier de Capri, et ce qui manque encore dit ;
- le binaire livré `target/release/capri-hook`, une fin de tour avec une ligne `[Capri]` sur son
  entrée : Capri absente, il part en 14 ms ; avec un écouteur d'essai, en 8 à 9 ms, sortie
  standard vide, code 0 ; la ligne reçue est celle d'avant le miroir quand il est désactivé ou
  illisible, et ne porte que `"said":"**Tout** passe, on peut souffler."` quand il est activé ;
- la répétition des réglages (binaire de débogage, `CAPRI_SETTINGS_REHEARSAL=1`, tous les
  dossiers d'essai) : l'onglet « Claude Code » s'affiche ; « Installer le style Capri »,
  « Retirer », « Brancher », « Débrancher » et la case du miroir, appuyés dans la vraie fenêtre,
  font ce qu'ils disent ; une vraie fin de tour envoyée pendant ce temps par `capri-hook` à cette
  Capri d'essai arrive (« Claude Code lui fait dire 33 caractères ») ; le journal ne contient
  jamais le texte de la réponse.

**Vérifié sans un bruit sur ce Mac, pour les effets de voix** (le 11 novembre 2026, sans rien
écouter) :

- d'abord, dans un projet d'essai à part, ce que fait vraiment le système, et dont tout dépend :
  `writeUtterance:toBufferCallback:` rend une phrase en paquets sur le thread principal, par sa
  boucle (sans elle, rien n'arrive), en 20 à 50 ms pour une phrase de deux secondes ; en flottants,
  mono, à 22,05 kHz (voix « compactes ») ou 16 kHz (« eloquence ») ; deux énoncés confiés à la
  suite sont rendus en même temps et finissent parfois dans le désordre ; un paquet vide apparaît
  au milieu d'un long énoncé de plusieurs phrases, et seul `didFinishSpeechUtterance` marque la
  fin ; `isSpeaking` reste faux pendant un tel rendu, et juste après `speakUtterance` (une dizaine
  de millisecondes) ; un débit de 0,5 écrit (la valeur par défaut) change le rendu des voix
  « eloquence » ; la hauteur et le débit agissent sur le rendu en mémoire (durée d'une même
  phrase, fréquence fondamentale par autocorrélation) ;
- `capri voice rehearse` (version de débogage), lancé à la main et par le test ignoré
  `voix_en_silence`, avec des voix d'essai muettes (énoncés à volume nul, mélangeur principal à
  zéro) ou rendues hors ligne (sans périphérique), sans lire aucun réglage : « Naturelle » passe
  par le chemin direct sans demander de hauteur ni de débit ; avec chacun des trois effets, trois
  phrases confiées d'un coup sont rendues puis jouées dans l'ordre, en temps réel, et le moteur
  s'arrête après la queue ; la pause fige le son et la reprise le relance sans rien perdre ;
  l'arrêt coupe net, et plus rien n'est rendu ni joué ; une autre voix (16 kHz après 22,05 kHz)
  puis d'autres effets remontent la chaîne entre deux phrases, dans l'ordre ; une phrase sans effet
  entre deux phrases avec attend la précédente ; quatre pannes simulées (chaîne impossible à
  monter, format illisible, moteur qui s'arrête seul, rien de rendu) font redire tout le reste
  sans effet, dans l'ordre, puis les effets reviennent après un arrêt ; hors ligne, chaque
  préréglage à chaque intensité change le son (écart de 0,85 à 1,26 fois le son sec), sans
  saturer (crête de 0,83 à 0,93), à un niveau de -4 à 0 dB du son sec ;
- la répétition des réglages (`CAPRI_SETTINGS_REHEARSAL=1`, dossiers d'essai) : l'onglet « Voix »
  s'affiche avec ses nouvelles listes ; un effet, une intensité, une hauteur et un débit choisis
  dans la vraie fenêtre sont écrits dans `voice.json`, puis « Naturelle » de nouveau. « Écouter »
  n'y est jamais pressé.

**Vérifié sur ce Mac, pour le jalon 3** (le 11 novembre 2026, sans rien regarder à l'écran) :

- la vraie liste des fenêtres, par le test ignoré `liste_des_fenetres` (version optimisée,
  `cargo test --release -p capri-platform --test liste_des_fenetres -- --ignored --nocapture`) :
  17 à 19 fenêtres à l'écran ; six séries de 500 lectures : 0,14 ms au mieux, 0,16 à 0,17 ms en
  médiane, 95 % sous 0,23 à 0,46 ms, 0,25 à 0,40 ms en moyenne, et quelques pointes de 4 à 7 ms ;
  savoir si un bouton de la souris est enfoncé coûte 0,1 µs (médiane) ;
- `capri perch rehearse` (version de débogage), lancé à la main et par le test ignoré
  `fenetres_du_bureau` : l'écran principal vu comme l'overlay le voit (3024 × 1964 px, 2 px par
  point, origine en (0, 0)), 15 fenêtres à l'écran dont 9 comptées (6 écartées parce qu'elles ne
  sont pas de la couche 0), les bords visibles, et aucun ne lui laissait la place ce
  jour-là (des fenêtres agrandies, la plus en avant à 33 points du haut) : « aucun bord ne lui
  laisse la place », comme il se doit. Sa sortie ne contient que des nombres et des mots de son
  propre code (le test le vérifie), et aucun réglage n'est écrit. Attention : lancé depuis un
  terminal qui a lui-même l'Enregistrement de l'écran, le processus en héritait (« accordé, rien
  n'a été demandé ») ; le code n'en dépend pas (il ne lit que des clés présentes sans elle), mais
  c'est depuis `Capri.app` qu'il faut le constater (liste ci-dessous) ;
- la vraie app en version de débogage, cinq secondes, avec un dossier, une socket et un trousseau
  d'essai, à côté d'une Capri déjà lancée et sans y toucher : elle démarre, son menu se construit
  avec la nouvelle case, rien ne plante, rien n'est écrit. Elle n'a été ni attrapée ni posée
  (impossible sans souris).

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
    (Le geste bref et la détection de fin de phrase sont décrits plus bas : ils n'ont jamais été
    essayés avec une vraie voix.)
15. la fenêtre de réglages s'ouvre devant sans figer Capri ; ⌘V colle dans ses champs et dans le
    champ ⌃⌥P ; une clé enregistrée vide son champ et n'y revient pas avec ⌘Z ; l'avertissement
    d'envoi apparaît pour un fournisseur distant ; la liste de modèles se filtre ; « Essayer » et
    « Utiliser ce cerveau » fonctionnent ; le clavier revient à l'app d'avant à la fermeture.

16. avec une vraie clé d'OpenRouter rangée depuis la fenêtre : une clé refusée s'affiche en rouge
    (401), un modèle gratuit saturé aussi (429), et « Essayer » avec un autre modèle gratuit
    obtient une vraie réponse, en français, en moins de trois secondes. C'est la première
    conversation aboutie avec un fournisseur à clé. Avec une clé d'Ollama en ligne, seule la
    liste des modèles a été reçue : elle est publique, ce qui ne prouve pas que la clé est
    acceptée.

17. avec OpenRouter choisi comme cerveau, elle répond dans sa bulle ; fenêtre de réglages
    ouverte, ⌘Q et ⌘H ne ferment que la fenêtre, et Capri reste là ; dans le champ ⌃⌥P, ils ne
    ferment que le champ ;
18. sans clé, la bulle le dit et renvoie à « Réglages… », sans commande de terminal ; une clé
    refusée (401) cite la raison que donne le fournisseur.

19. Capri dans Claude Code, dans une vraie session (le 11 novembre 2026, style installé depuis
    l'onglet « Claude Code », `/output-style Capri`, miroir activé) : « J'ai vérifié le jalon 4,
    cela fonctionne très bien », sans retour cas par cas. Le style s'installe depuis l'onglet,
    Claude Code le propose et, une fois choisi, répond avec son ton et termine par la ligne
    `[Capri] …` ; miroir activé, Capri dit cette ligne sur le bureau (voir plus bas les cas qui
    n'ont pas été confirmés un par un).

20. Les effets de voix, écoutés le 11 novembre 2026 depuis `Capri.app` : « cela fonctionne bien »,
    sans retour cas par cas (voir plus bas ceux qui n'ont pas été confirmés un par un).

**Pas encore vérifié** : une vraie conversation avec DeepSeek, ProxyAPI ou Ollama en ligne (ils
passent par le même code qu'OpenRouter, mais n'ont pas été essayés) ; rien avec le vrai service
d'Anthropic (voir la liste plus bas) ; un vrai LM Studio ; la
demande d'accès au trousseau quand `Capri.app` lit une clé rangée depuis le terminal. Capri dans
une vraie session de Claude Code a été essayée dans l'ensemble (point 19), pas cas par cas.

**L'API d'Anthropic : à vérifier à la main, avec ta propre clé** (rien n'a encore parlé au vrai
service ; sur ton Mac, `Capri.app` reconstruite) :

1. onglet « Cerveau », « Anthropic » : l'avertissement orange dit que la conversation part chez
   Anthropic (api.anthropic.com:443) ; colle ta clé, « Enregistrer la clé » : « Une clé est rangée
   pour api.anthropic.com:443 » ;
2. « Demander la liste » : les modèles que ta clé peut utiliser arrivent (aucun nom n'est écrit
   dans Capri), le champ de recherche les filtre ; rien n'est demandé avant d'appuyer ;
3. un modèle choisi, « Essayer » : une vraie réponse, en français, sans Markdown, sans rien de sa
   réflexion ; puis « Utiliser ce cerveau », et une vraie conversation dans sa bulle (⌃⌥P), sur
   plusieurs messages, et à voix haute si sa voix est activée ; « La faire taire » la coupe ;
4. une clé refusée (une clé révoquée, ou une clé valable dont on a changé un caractère) : le
   message est en rouge, dit 401 et cite la raison d'Anthropic, sans le moindre morceau de la clé
   (attention : un mot qui ressemble à un morceau de la clé, comme « -api » dans « x-api-key »,
   peut être masqué lui aussi), et dit de la remettre ci-dessus ;
5. si l'occasion se présente : un 429 ou un 529 dit en clair, et un refus du modèle dit par sa
   phrase plutôt qu'un silence ;
6. `capri chat key anthropic`, `capri chat models anthropic`, `capri chat test` au terminal :
   mêmes résultats ;
7. la demande d'accès au trousseau de macOS, si la clé a été rangée depuis le terminal puis lue
   par `Capri.app`.

**Effets de voix, cas pas encore confirmés un par un** (écoutés dans l'ensemble, voir le
point 20 ; sa voix activée, depuis `Capri.app` ou `cargo run --release`) :

1. « Naturelle » (le défaut) : elle sonne exactement comme avant ce changement ;
2. dans l'onglet « Voix », chaque effet avec « Écouter » : Construct, un léger grain et une petite
   salle, la voix restant claire ; Liaison radio, une voix serrée et un peu saturée, plus discrète ;
   Hologramme, un timbre métallique et une réverbération brillante, plus marqué. Aucun ne doit
   grésiller fort, saturer ni rendre les mots incompréhensibles ;
3. l'intensité, de « légère » à « forte » : l'effet doit s'entendre davantage, sans devenir
   pénible ;
4. la hauteur et le débit, aux deux extrêmes, avec et sans effet : un écart net mais naturel, sans
   voix de dessin animé ;
5. une vraie réponse avec un effet : les phrases s'enchaînent dans l'ordre, sans trou gênant entre
   elles, et le début n'arrive pas nettement plus tard qu'en « Naturelle » ;
6. « La faire taire » pendant qu'elle parle avec un effet : silence immédiat, sans traîne de
   réverbération ;
7. la cacher (⌃⌥C) au milieu d'une phrase traitée, attendre, la montrer : la phrase reprend là
   où elle s'était arrêtée ;
8. changer d'effet dans la fenêtre pendant une longue réponse : la phrase suivante prend le nouvel
   effet ;
9. le niveau : avec un effet, elle n'est pas nettement moins forte qu'en « Naturelle » ;
10. brancher ou débrancher un casque pendant qu'elle parle avec un effet : elle continue, sans
    effet jusqu'à la réponse suivante (une ligne « voix : effets abandonnés » dans le journal) ;
11. `capri voice test` au terminal, avec un effet : on entend l'effet, et la commande attend la fin
    de la queue avant de rendre la main.

**Le geste bref et la fin de phrase détectée seule : à essayer avec ta propre voix** (aucun test
ne peut ouvrir le micro ; seule la logique de décision, sur une horloge factice, est testée) :

1. un appui bref sur ⌃⌥D, puis une phrase : la bulle bleue suit ce que tu dis, et la phrase part
   seule environ une seconde et demie après ta dernière parole ; le témoin orange s'éteint ;
2. maintenir ⌃⌥D, parler, relâcher : exactement comme avant, et même après une longue pause
   pendant que la touche est enfoncée, elle n'envoie pas avant le relâchement ;
3. un appui bref puis un second appui au milieu de la phrase : elle envoie tout de suite ce qu'elle
   a compris ;
4. un appui bref puis rien : au bout de 7 s environ, le micro se ferme (le témoin orange s'éteint),
   « Je n'ai rien entendu », et rien n'est envoyé ;
5. le même geste par le menu (un clic commence, un second termine tout de suite) ;
6. une pause naturelle au milieu d'une phrase (réfléchir une seconde) : elle ne coupe pas ; si elle
   coupe trop souvent, ou si elle met trop de temps à envoyer, dis-le : les délais sont des
   constantes ;
7. parler plus d'une minute (en maintenant, puis en écoute libre) : le micro se ferme vers 55 s, ce
   qui a été compris part, et le témoin orange s'éteint ;
8. un appui long mais lent à relâcher, ou un appui très court ressenti comme maintenu : le seuil
   (350 ms) tombe-t-il où tu l'attends ?
9. après sa réponse, le micro ne se rouvre pas tout seul ;
10. la première fois, sans permission accordée : elle dit « Autorise le micro et la dictée »,
    puis « Appuie sur ⌃⌥D pour me parler » une fois accordées.

**Capri dans Claude Code, cas plus fins pas encore regardés un par un** :

1. une réponse dans un sous-agent ne la fait pas parler ; un nouveau prompt envoyé avant qu'elle
   ait parlé l'empêche de dire une ligne périmée ;
2. pendant qu'elle répond à ce que tu lui as écrit (⌃⌥P), ou pendant que tu lui parles au micro (⌃⌥D), une
   fin de tour de Claude Code attend : elle dit sa ligne après, pas en même temps ;
3. « Brancher » et « Débrancher » de l'onglet, comme ceux du menu : `/hooks` montre les entrées,
   le menu et l'onglet restent d'accord, et `settings.json` redevient identique à l'octet près ;
4. un `capri.md` à toi dans `output-styles/` n'est jamais touché.

**Jalon 3 : à regarder à la main** (rien n'a encore été vu ; depuis `Capri.app` reconstruite, ou
`RUST_LOG=debug cargo run --release` pour lire le journal) :

1. **se poser** : une fenêtre ordinaire (Finder, TextEdit) dont le haut est vers le milieu de
   l'écran ; lâche Capri doucement, les pieds un peu au-dessus du bord : elle descend s'y poser, ses
   pieds sur le bord (ni vide dessous, ni pieds enfoncés dans la barre de titre), s'écrase un
   instant, rebondit, puis ne bouge plus (plus de flottement). Lâchée les pieds dans la barre de
   titre : elle remonte sur le bord. Lâchée nettement plus haut (au-delà d'environ 45 points) :
   elle flotte, comme avant ;
2. près d'un coin arrondi : ses pieds rentrent un peu sur le bord, elle ne se tient pas dans le
   vide ; contre le bord de l'écran, elle reste entière à l'écran ;
3. **la lancer** vers une fenêtre : si elle s'arrête près d'un bord, elle s'y pose ; sinon elle
   flotte. Les rebonds sur les bords de l'écran sont ceux d'avant ;
4. **suivre** : déplace sa fenêtre lentement puis vite : elle suit, reste sur le bord et dans
   l'écran, et part avec la fenêtre dès le début du déplacement (un léger retard, au rythme de ses
   dessins, est normal). Redimensionne-la par la gauche : elle ne bouge pas ; par la droite,
   jusqu'à plus étroite que l'endroit où elle était : elle glisse au bout du bord, sans le quitter ;
   par un coin du haut : elle monte ou descend avec le bord ;
5. **fenêtre fermée** (⌘W), réduite dans le Dock (⌘M), app masquée (⌘H) : elle se remet à
   flotter sur place en un quart de seconde au plus, avec un petit étirement, sans sauter ailleurs ;
6. **bord recouvert** : clique sur une autre fenêtre qui chevauche le bord sous ses pieds, elle
   repart ; une fenêtre devant mais à côté de ses pieds : elle reste ;
7. **plein écran** : sa fenêtre en plein écran (bouton vert), elle repart ; une fenêtre agrandie
   jusqu'en haut de l'écran ne peut pas la porter (pas la place) ;
8. **second écran** (si tu en as un) : sa fenêtre glissée vers l'autre écran, elle la suit
   jusqu'au bord de son écran puis repart ; les fenêtres de l'autre écran ne la portent pas ;
9. **changement de bureau** (⌃→ puis ⌃←) : elle flotte sur l'autre bureau, et au retour elle
   n'est plus posée ;
10. **⌃⌥C** : posée, cache-la, déplace sa fenêtre, montre-la : elle rejoint la fenêtre aussitôt
    (ou flotte si la fenêtre a disparu entre-temps) ;
11. **le menu** : « S'asseoir sur les fenêtres » est coché, dans le menu de l'icône comme au clic
    droit ; décoche-le : posée, elle repart, et lâchée près d'un bord elle flotte ; les deux menus
    restent d'accord ; quitte et relance Capri : toujours décoché ; recoche-le ;
12. **le processeur au repos**, dans le Moniteur d'activité (% processeur de Capri) : flottante, puis
    posée sans que rien ne bouge (presque pareil), pendant qu'on déplace sa fenêtre (un peu plus),
    puis cachée (comme cachée avant ce jalon) ;
13. aucune demande de permission n'apparaît (ni Accessibilité, ni Enregistrement de l'écran), et
    elle se pose quand même ;
14. avec `RUST_LOG=debug`, les lignes « posée sur un bord », « aucun bord sous ses pieds » et
    « quitte le bord : … » ne contiennent que des nombres ;
15. si tu utilises Stage Manager ou Mission Control : rien n'a été prévu pour eux, regarder
    seulement que rien ne casse.

**Pas encore testé sur un vrai PC Windows** : il n'y a pas encore de fenêtre de réglages. Le tube
nommé de `capri-ipc` n'y a jamais tourné, le champ de saisie et la voix n'y existent pas encore,
et le chat n'y a jamais été lancé : ni le https (SChannel), ni le Gestionnaire d'identification,
ni la saisie d'une clé sans écho n'y ont tourné. Le style de sortie (dans
`%USERPROFILE%\.claude\output-styles`) et le miroir (dans `%LOCALAPPDATA%\Capri`) n'y ont pas
tourné non plus. La liste des fenêtres (jalon 3 : `EnumWindows`, le cadre et l'état « masquée »
de DWM, l'état des boutons de la souris) est écrite et compilée, jamais exécutée : sur un vrai PC,
`cargo test --release -p capri-platform --test liste_des_fenetres -- --ignored --nocapture`, puis
les points du jalon 3 ci-dessus (les fenêtres d'un autre bureau virtuel et les fenêtres réduites
ne doivent pas la porter).

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
3. ~~Elle s'assoit sur tes fenêtres~~ ← jalon 3 (macOS : CoreGraphics, sans aucune permission ;
   Windows : Win32) : écrit et testé sans écran ; reste à le regarder sur un vrai Mac, et à
   l'exécuter sur un vrai PC Windows.
4. Le chat avec un modèle d'IA : ~~la saisie et la bulle~~, ~~un modèle local par Ollama~~,
   ~~les fournisseurs à clé compatibles OpenAI (OpenRouter, DeepSeek, ProxyAPI, Ollama en ligne)
   avec leurs clés dans le trousseau~~, ~~une fenêtre de réglages (macOS)~~, ~~Capri dans Claude
   Code pour qui a un abonnement (un style de sortie et le miroir)~~ (écrit, testé sans Claude
   Code, puis essayé dans une vraie session le 11 novembre 2026), ~~l'API d'Anthropic~~ (écrite et
   testée contre un faux serveur local ; reste à l'essayer avec une vraie clé). Sa voix : ~~ses
   réponses dites à voix haute~~, ~~lui parler au micro~~ (reconnaissance sur la machine), ~~des
   effets de voix, la hauteur et le débit~~ (écrits, vérifiés sans bruit, puis écoutés le 11 novembre 2026),
   ~~la fin de la phrase détectée seule~~ (écrite et testée sans micro ; reste à l'essayer avec ta
   voix). Le mot d'éveil est écarté : il demanderait un micro ouvert en permanence, et Capri ne
   l'ouvre que sur un geste.
5. App signée et notarisée (`.app` / installeur), mises à jour.

## Licence

Licence du **code** à choisir (MIT ou Apache-2.0 par exemple). Les **images** n'en relèvent pas :
voir [CREDITS.md](CREDITS.md).
