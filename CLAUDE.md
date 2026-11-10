# Capri — consignes pour Claude Code

Compagne de bureau (Capri, *Cyberpunk: Edgerunners* 2) écrite en Rust, 100 % native, pour macOS
(cible principale, Apple Silicon) et Windows. Projet de fan non commercial. Le README décrit l'état
du projet et la feuille de route ; jalons 1 (elle flotte) et 2 (le bandeau Claude Code) faits,
jalon 4 (lui parler) en cours : saisie, bulle, modèle local (Ollama, LM Studio), fournisseurs
compatibles OpenAI avec leur clé dans le trousseau (OpenRouter, DeepSeek, ProxyAPI, serveur
personnalisé), voix et écoute au micro en place ; API d'Anthropic et mot d'éveil à venir.

## Architecture : où va quoi

- `capri-core` : logique pure, **aucune dépendance système ni GPU**. Tout ce qui peut être testé
  sans écran va ici, avec des tests.
- `capri-claude` : tout ce que Capri sait de Claude Code, lui aussi **sans dépendance système** :
  lecture de l'entrée des hooks, protocole, suivi des sessions, édition de `settings.json` de
  texte à texte. Avec des tests.
- `capri-chat` : la conversation, **sans dépendance système ni réseau** : historique borné,
  format des fournisseurs (ce qu'on envoie, comment lire le flux), fournisseurs et besoin d'une
  clé (`brain`), règles d'adresse et règles de la clé (`endpoint`, `key` : jamais en clair, et
  liée à son adresse), réglages du cerveau, personnalité, nettoyage du Markdown. Avec des tests.
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
  Seul le champ de saisie (`macos/input.rs`) prend le clavier, parce qu'on le lui demande, et il
  le rend à la fenêtre d'avant en se fermant.
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
- Aucune télémétrie, rien n'envoyé sur le réseau sans action explicite de l'utilisateur.
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
  commande, jamais en argument de ligne de commande. `capri_chat::ApiKey` n'a pas de `Display`
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
- Le `settings.json` de Claude Code appartient à l'utilisateur et à d'autres outils : toute
  modification passe par `capri_claude::settings` (réécriture à l'octet près, vérification) et
  par `hook_setup` (sauvegarde, écriture atomique). Jamais d'édition directe.
- CapriChooms/CapriCore (GPL-3.0) : on s'inspire de ses idées, on ne copie pas son code.
