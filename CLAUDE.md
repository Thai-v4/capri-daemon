# Capri — consignes pour Claude Code

Compagne de bureau (Capri, *Cyberpunk: Edgerunners* 2) écrite en Rust, 100 % native, pour macOS
(cible principale, Apple Silicon) et Windows. Projet de fan non commercial. Le README décrit l'état
du projet et la feuille de route ; jalons 1 (elle flotte) et 2 (le bandeau Claude Code) faits,
jalon 4 (lui parler) en cours : saisie, bulle, modèle local (Ollama, LM Studio), fournisseurs
compatibles OpenAI avec leur clé dans le trousseau (OpenRouter, DeepSeek, ProxyAPI, Ollama en
ligne, serveur personnalisé), voix et écoute au micro en place ; API d'Anthropic et mot d'éveil à
venir. Sur macOS, une fenêtre de réglages (« Réglages… » dans son menu) règle son cerveau et sa
voix sans terminal.

## Architecture : où va quoi

- `capri-core` : logique pure, **aucune dépendance système ni GPU**. Tout ce qui peut être testé
  sans écran va ici, avec des tests. Dont `form` : la description d'un formulaire de réglages
  (rubriques, lignes, événements, `Secret` au `Debug` masqué), que le modèle des réglages produit
  et que la fenêtre de `capri-platform` affiche ; elle est ici parce que les deux doivent la voir
  et que `capri-chat` ne peut pas dépendre de `capri-platform`.
- `capri-claude` : tout ce que Capri sait de Claude Code, lui aussi **sans dépendance système** :
  lecture de l'entrée des hooks, protocole, suivi des sessions, édition de `settings.json` de
  texte à texte. Avec des tests.
- `capri-chat` : la conversation, **sans dépendance système ni réseau** : historique borné,
  format des fournisseurs (ce qu'on envoie, comment lire le flux), fournisseurs et besoin d'une
  clé (`brain`), règles d'adresse et règles de la clé (`endpoint`, `key` : jamais en clair, et
  liée à son adresse), réglages du cerveau, personnalité, nettoyage du Markdown. Avec des tests.
  Les phrases que disent à la fois `capri chat` et la fenêtre de réglages (où part ce qu'on lui
  dit, clé rangée ou retirée…) sont dans `phrases`, écrites une seule fois. Le modèle de la
  fenêtre de réglages est dans `settings` (voir plus bas).
- `capri-net` : **le seul crate qui parle au réseau** (ureq). Il se connecte et rien d'autre :
  quoi envoyer et comment lire la réponse est décidé par `capri-chat`. En `https://`, le TLS du
  système (fonction `native-tls-no-default` de ureq, `RootCerts::PlatformVerifier`), sur macOS et
  Windows seulement ; sous Linux, pas de TLS (il y faudrait OpenSSL) et `https://` est refusé.
  Testé contre un faux serveur local ; aucun test ne sort de la machine.
- `capri-render` : wgpu, et le tracé du texte du bandeau et de la bulle (cosmic-text, polices du
  système).
  Couleurs prémultipliées de bout en bout.
- Les appels système et l'`unsafe` lié à l'OS vivent dans **deux crates, et nulle part
  ailleurs** : `capri-platform` (la fenêtre, la voix, le micro, le trousseau) et `capri-ipc` (le
  canal local).
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
  **Deux fenêtres seulement prennent le clavier**, parce qu'on le leur demande, et elles le
  rendent à la fenêtre d'avant en se fermant : le champ de saisie (`macos/input.rs`) et la
  fenêtre de réglages (`settings_window.rs`, `macos/settings_window.rs`). Ce sont les seules
  exceptions voulues à « Capri ne prend jamais le focus » : toutes deux sont des `NSPanel` non
  activants (elles deviennent fenêtre clé sans faire passer l'app au premier plan), et aucune ne
  s'ouvre toute seule. Pas de boucle modale.
  Capri n'a ni barre des menus ni menu Édition (app « accessoire ») : ⌘X, ⌘C, ⌘V, ⌘A, ⌘Z et ⇧⌘Z
  ne passeraient pas aux champs. La vue de contenu de ces deux panneaux (`macos/edit_keys.rs`)
  les reconnaît dans `performKeyEquivalent:` et envoie l'action d'édition au premier répondeur.
  Toute nouvelle fenêtre avec des champs doit l'utiliser.
  La fenêtre de réglages affiche un `capri_core::form::Form` et ne sait rien de Capri ; elle
  garde d'un formulaire à l'autre les contrôles de même identifiant et de même sorte, ne réécrit
  jamais un champ en cours de saisie, valide la saisie en cours avant toute action, et ne lit un
  champ secret qu'au moment de l'appui qui l'emporte, avant de le vider. Ailleurs que sur macOS :
  refus explicite (`SettingsWindowError::Unavailable`).
  Sa voix vit ici aussi (`voice.rs`, `macos/voice.rs` : `AVSpeechSynthesizer`) ; ce qu'elle dit
  et avec quelle voix est décidé par `capri_chat::voice`, qui est pur.
  Son oreille de même (`ear.rs`, `macos/ear.rs` : `AVAudioEngine` et le framework Speech) ; le
  déroulé d'une prise de parole est dans `capri_chat::dictation`, pur lui aussi.
  Le trousseau aussi (`keychain.rs` ; `macos/keychain.rs` : les `SecItem` de
  `security-framework`, déjà tiré par native-tls ; `windows/keychain.rs` : `CredWriteW`,
  `CredReadW`, `CredDeleteW` ; ailleurs : refus) et la saisie d'une clé sans écho
  (`terminal.rs` : `readpassphrase(3)` sur macOS, qui rend l'écho même sur un signal ; mode de la
  console sous Windows ; ailleurs : refus).
- `capri-app` : assemble le tout. Pas d'appel système direct ici (sauf menus via `tray-icon`).
  La fenêtre passe toujours par `capri_platform::Overlay`, jamais par `winit::window::Window`.
  Les clés passent par `keys` (le trait `KeyStore` : le trousseau du système, ou un trousseau en
  mémoire pour les tests) ; les commandes `capri chat` par la structure `Cli` de `brain_setup`,
  qui reçoit son dossier, son trousseau, sa saisie et sa sortie : c'est ce qui les rend testables.
  `settings_view` relie le modèle des réglages à la fenêtre et exécute ses effets avec les
  fonctions de la ligne de commande (`brain_setup::load`, `save`, `list_models`, `talk`,
  `keys::store`, `keys::forget`, `voice_setup`) : la fenêtre et `capri chat` / `capri voice`
  restent d'accord parce qu'elles passent par le même code. Trousseau et réseau sur un thread à
  part, résultats relevés à chaque pas de la boucle.

## La fenêtre de réglages : ajouter une rubrique

Le modèle (`capri_chat::settings`) est pur et testé sans écran : à partir des réglages lus, des
`FormEvent` de l'utilisateur et des `Outcome` des opérations lancées, il produit le `Form` à
afficher et des `Effect` à exécuter. Chaque onglet est une rubrique, qui implémente le trait
`Rubric` (`brain.rs` : « Cerveau », `voice.rs` : « Voix »).

Pour en ajouter une :

1. un module pur dans `crates/capri-chat/src/settings/` qui implémente `Rubric` : son
   identifiant (qui commence celui de chacune de ses lignes : `micro.status`), son titre, ses
   lignes, ce qu'elle relit à l'ouverture (`open`), ce qu'elle fait d'un événement et d'un
   résultat ; les règles et les textes vivent là, avec ses tests, et réutilisent les phrases et
   les règles de la ligne de commande plutôt que de les réécrire ;
2. son inscription dans `Settings::new` ;
3. seulement si elle demande au système quelque chose qu'aucun `Effect` ne sait faire : la
   variante d'`Effect` et son `Outcome`, et leur exécution dans `capri-app/src/settings_view.rs`
   (`Effects::run`), en passant par les mêmes fonctions que la commande correspondante.

La fenêtre (`capri-platform`) n'a pas à changer tant que les contrôles de `capri_core::form`
suffisent. Pour vérifier qu'elle s'ouvre sans planter, en version de débogage :
`CAPRI_SETTINGS_REHEARSAL=1` (avec `CAPRI_CONFIG_DIR`, `CAPRI_KEYCHAIN_SERVICE` et `CAPRI_SOCKET`
d'essai) l'ouvre au lancement, y choisit chaque fournisseur, range et retire une clé factice par
le vrai champ secret, et quitte ; le journal dit ce qu'elle affiche, jamais la clé.

## Vérifier avant de dire « c'est fait »

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings
cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings
cargo test --workspace
```

Le code Windows ne se teste pas depuis un Mac : la CI GitHub Actions le compile et le teste.
Les deux clippy croisés tiennent parce qu'aucune dépendance ne compile de C (sous Linux,
wayland-backend tire `cc`, mais ne s'en sert qu'avec sa fonction `log`, qui n'est pas active) :
pas de rustls (son fournisseur par défaut dans ureq, ring, compile du C, et son script de
construction échoue vers Windows depuis un Mac), pas d'OpenSSL. Ne pas en ajouter sans revérifier
les trois.
Tout comportement visible (transparence, clics, animation) se vérifie à la main : dire
précisément quoi vérifier plutôt que de l'affirmer.

## Règles

- **API des crates** : wgpu 30, winit 0.30, objc2 0.6 / objc2-app-kit 0.3 / objc2-avf-audio 0.3 /
  objc2-speech 0.3 / block2 0.6, windows-sys 0.61, tray-icon 0.26, cosmic-text 0.19, ureq 3.5
  (avec `native-tls-no-default` sur macOS et Windows seulement), security-framework 3.7. Une
  fonction C du système déclarée à la main (`readpassphrase`) se vérifie dans l'en-tête du SDK.
  Ces API changent souvent : vérifier dans les sources
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
- Aucune télémétrie, rien n'envoyé sur le réseau sans action explicite de l'utilisateur. La
  fenêtre de réglages ne demande d'elle-même la liste des modèles qu'à un serveur de cette
  machine ; à tout autre, seulement quand on appuie sur « Demander la liste ».
- Le chat : seules la personnalité et la conversation partent vers le modèle, et seulement vers
  l'adresse que l'utilisateur a choisie. Jamais l'activité de Claude Code. Pas de proxy, aucune
  redirection suivie. `http://` seulement vers la machine ou un réseau privé : toute adresse
  passe par `capri_chat::Endpoint`. Ce qui se dit n'est jamais écrit dans le journal.
- Sa voix : des voix existantes (celles du système pour l'instant), **jamais un clone de la voix
  de la comédienne de la série**. Rien n'est enregistré ni envoyé. Désactivée par défaut. Les
  tests ne font aucun bruit : pas de test qui crée une vraie voix.
- Son oreille : le micro n'est ouvert que pendant que l'utilisateur maintient le raccourci, la
  transcription est **exigée sur l'appareil** (`setRequiresOnDeviceRecognition(true)` : jamais
  de repli sur un serveur), rien n'est enregistré. Capri ne demande une permission que si son
  `Info.plist` l'explique (`tools/macos/Info.plist`) : sinon macOS tue le processus.
- Raccourcis globaux : une touche y est désignée par sa place, pas par sa lettre. Ne choisir que
  des lettres à la même place en AZERTY et en QWERTY (ni A, Q, W, Z, M, ni la ponctuation).
- Aucun nom de modèle écrit en dur : il vient de l'utilisateur ou de la liste que donne le
  serveur. Même règle que pour les API : format et adresses d'un fournisseur se vérifient dans sa
  documentation, pas de mémoire (les sources de chaque adresse sont citées dans
  `capri_chat::brain`). Aucun en-tête qui présente l'app à un fournisseur (`HTTP-Referer`,
  `X-OpenRouter-Title`…).
- **Une clé est liée à l'adresse pour laquelle elle a été donnée.** Son compte dans le trousseau
  nomme le fournisseur et l'adresse, hôte et port (`openrouter@openrouter.ai:443` :
  `capri_chat::KeyAccount`, à ne jamais changer sans perdre les clés rangées). Elle n'est lue que
  par `keys::key_for(trousseau, fournisseur, adresse jointe)`, revient liée à son compte
  (`BoundKey`), et `capri_chat::key::authorization` refuse d'en faire un en-tête pour une autre
  adresse : `capri-net` la traite alors comme absente. Une clé rangée sans adresse par une version
  précédente (`key::legacy_account`) n'est jamais lue pour être envoyée : on la signale, on la
  retire ; pas de migration.
- **Les clés d'API** : uniquement dans le trousseau du système (service `Capri`, un compte par
  fournisseur et par adresse), par `capri_platform::keychain` et `keys`. Jamais dans `chat.json`
  ni aucun fichier, jamais dans le journal, jamais dans un message d'erreur ou une sortie de
  commande, jamais en argument de ligne de commande, jamais dans un champ de la fenêtre de
  réglages (son champ secret n'est jamais prérempli ; elle dit seulement s'il y a une clé pour
  l'adresse affichée). Ce qui y est tapé n'en sort que dans un `capri_core::form::Secret`, aussi
  masqué que `ApiKey`. `capri_chat::ApiKey` n'a pas de `Display`
  et son `Debug` la masque ; son texte ne sort que pour le trousseau et pour l'en-tête
  `Authorization`.
  Une clé ne part qu'en `https://`, ou en `http://` vers cette machine même
  (`Endpoint::may_carry_secret`, vérifié par `capri-net` avant toute connexion) ; jamais vers un
  fournisseur qui n'en prend pas. Un message de fournisseur est masqué (`ApiKey::redact`) avant
  d'être cité, et un 401 n'est jamais cité. Le journal de l'app ne contient jamais les traces
  `trace` de ureq (`NoWireLog` dans `main.rs`), qui écrivent les octets échangés sans masque.
- Trousseau et tests : un test qui touche au vrai trousseau est `#[ignore]`, n'utilise qu'un
  service réservé aux tests et nettoie derrière lui. Pour des essais en ligne de commande :
  `CAPRI_CONFIG_DIR=<dossier temporaire>` et `CAPRI_KEYCHAIN_SERVICE=<service d'essai>`, et ne
  jamais lire d'autre élément du trousseau. Aucun appel authentifié à un vrai fournisseur sans
  la clé et l'accord de l'utilisateur.
- Le canal du hook : socket Unix (Windows : named pipe) accessible au seul utilisateur, jamais
  un port TCP sans authentification.
- `capri-hook` ne doit **jamais gêner Claude Code** : rien sur la sortie standard, toujours le
  code de sortie 0, une durée de vie bornée. Les hooks sont déclarés `async`.
- Le bandeau ne transmet ni n'affiche jamais le texte d'un prompt, le contenu d'un fichier ou la
  sortie d'une commande. Dans le doute, il se tait plutôt que d'affirmer quelque chose de périmé.
- Au-dessus de sa tête, une seule chose à la fois : ce qu'on lui dit au micro d'abord, puis sa
  parole (on vient de lui parler), puis un message de l'app, puis l'activité de Claude Code.
- **Ce qui s'affiche dans la bulle ou dans la fenêtre de réglages ne renvoie jamais à une commande
  de terminal** (`capri chat …`, `capri voice …`) : le but de la fenêtre est que personne n'ait à
  ouvrir un terminal. La bulle renvoie à « Réglages… » dans son menu, la fenêtre dit quoi faire
  sur place (« remets-la ci-dessus »). Seul un terminal garde ses commandes. Une phrase s'écrit
  une fois ; le conseil qui la termine se choisit par `capri_chat::Place` (`Terminal`, `Bubble`,
  `Window`), jamais en dupliquant la phrase. Là où la fenêtre ne sait pas encore faire la chose
  (personnalité `persona.md`…), le message peut citer le fichier ou la commande : ne pas promettre
  une rubrique qui n'existe pas.
- **⌘Q et ⌘H ne touchent jamais à Capri tant qu'un de ses panneaux est la fenêtre clé** : dans la
  fenêtre de réglages et dans le champ de saisie, ils ferment seulement ce panneau (comme ⌘W et
  Échap), parce que `EditKeysView` (`macos/edit_keys.rs`) les prend avant le menu par défaut de
  winit (Quitter, Masquer). Capri ne se quitte que par « Quitter » dans son propre menu, et ne se
  masque que par son propre raccourci. Tout nouveau panneau qui prend le clavier porte cette vue.
- Le `settings.json` de Claude Code appartient à l'utilisateur et à d'autres outils : toute
  modification passe par `capri_claude::settings` (réécriture à l'octet près, vérification) et
  par `hook_setup` (sauvegarde, écriture atomique). Jamais d'édition directe.
- CapriChooms/CapriCore (GPL-3.0) : on s'inspire de ses idées, on ne copie pas son code.
