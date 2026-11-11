# Capri — consignes pour Claude Code

Compagne de bureau (Capri, *Cyberpunk: Edgerunners* 2) écrite en Rust, 100 % native, pour macOS
(cible principale, Apple Silicon) et Windows. Projet de fan non commercial. Le README décrit l'état
du projet et la feuille de route ; jalons 1 (elle flotte) et 2 (le bandeau Claude Code) faits,
jalon 3 (elle s'assoit sur tes fenêtres) écrit et testé sans écran, pas encore regardé,
jalon 4 (lui parler) en cours : saisie, bulle, modèle local (Ollama, LM Studio), fournisseurs
compatibles OpenAI avec leur clé dans le trousseau (OpenRouter, DeepSeek, ProxyAPI, Ollama en
ligne, serveur personnalisé), l'API d'Anthropic avec sa clé d'API (son propre format ; testée
contre un faux serveur local seulement, pas encore essayée avec une vraie clé), voix et écoute au
micro en place, effets de voix, hauteur et débit écrits, vérifiés sans bruit et écoutés dans
l'ensemble ; Capri dans Claude Code (un style de sortie, et le miroir qui lui fait dire la ligne
que Claude écrit pour elle) en place, essayé dans une vraie session le 11 novembre 2026 (dans
l'ensemble, pas cas par cas) ; elle détecte seule la fin de la phrase (un appui bref sur le
raccourci suffit ; pas essayé avec une vraie voix) ; le mot d'éveil est écarté (il demanderait un
micro ouvert en permanence). Sur macOS, une fenêtre de réglages
(« Réglages… » dans son menu) règle son cerveau, sa voix et Claude Code sans terminal.

## Architecture : où va quoi

- `capri-core` : logique pure, **aucune dépendance système ni GPU**. Tout ce qui peut être testé
  sans écran va ici, avec des tests. Dont `form` : la description d'un formulaire de réglages
  (rubriques, lignes, événements, `Secret` au `Debug` masqué), que le modèle des réglages produit
  et que la fenêtre de `capri-platform` affiche ; elle est ici parce que les deux doivent la voir
  et que `capri-chat` ne peut pas dépendre de `capri-platform`. Et `sound`, pour la même raison :
  la **description neutre** de comment dire une phrase (`Delivery` : voix, hauteur, débit, et une
  chaîne de traitements `Effect` — réverbération, délai, distorsion, égaliseur — avec leurs
  paramètres), ramenée dans les bornes documentées par Apple (`Delivery::bounded`), et la remise
  des échantillons du système en canaux séparés (`planar`). Elle ne sait rien de Capri.
  Et `sheets` : les planches de sa pose assise (cases, dessins tenus en deux, ligne du bord de la
  fenêtre), et dans `sheets::stand` celles de sa pose debout (sa hauteur, ses pieds, son échelle
  face à la pose assise ; la boucle ne flotte pas, le flottement reste au programme), mesurées sur
  les images ; `shown` serre les seuls dessins montrés (un sur deux pour les boucles et le
  clignement) dans les grilles confiées à la carte graphique.
  Et `look` : **son dessin**, quelle case montrer à chaque image (debout, la boucle et ses
  clignements aux tours de `BLINKS`, sa bouche posée par-dessus quand elle parle ; posée,
  l'arrivée, sa boucle assise, parlante ou en musique, le départ), où (sa ligne d'assise sur le
  bord), ce qui l'attrape (un masque par pose, l'union de sa boucle), ce qu'il lui faut sur un bord
  (`Look::stature`, que `perch` reçoit), et le repli sur son dessin d'avant quand des planches
  manquent (`Wardrobe`). `body` ne connaît que son image debout, le point posé sur le bord
  (`Capri::seat`), et combien d'images dure son départ.
  Et `scale` : **sa taille**, choisie par l'utilisateur, est une échelle `Scale` rapportée à sa
  hauteur **debout** d'origine (300 points pour sa silhouette, `stand::STATURE`, pas pour la case),
  de 40 % à 140 % (borne choisie pour son dessin d'avant, agrandi 2 fois à 140 % sur un écran
  Retina ; ses planches le sont 2,42 fois), jamais des dimensions en dur. Tous ses dessins se
  rapportent à sa hauteur debout à l'écran (`Look::set_height`) : un nouveau dessin n'ajoute pas
  sa propre taille.
  Et `perch` : où se poser sur les fenêtres des autres apps, d'après la description neutre qu'en
  donne `capri-platform` (`DeskWindow` : un numéro, un rectangle, ordinaire ou non, l'opacité, à
  elle ou non ; rien d'autre) : le passage du repère du système à celui de l'overlay (`DeskFrame`,
  AppKit compris), le tri (`skip`, `sift`), les morceaux visibles des bords (`ledges`),
  l'accroche (`landing`), le suivi et le départ (`follow`, `Leave`), la cadence de lecture
  (`Watch`, en images), d'après ce qu'il lui faut autour du point posé (`Stature` : sa hauteur
  debout, la place qu'elle occupe, le morceau de bord qui la porte). Le corps (`body`) a la
  posture `Perched` et ses deux gestes, ancrés à ses pieds (`AlphaMask::feet`).
- `capri-claude` : tout ce que Capri sait de Claude Code, lui aussi **sans dépendance système** :
  lecture de l'entrée des hooks, protocole, suivi des sessions, édition de `settings.json` de
  texte à texte, le style de sortie « Capri » (`style` : son texte, la signature qui reconnaît le
  nôtre), le miroir (`mirror` : son réglage, l'extraction de la ligne `[Capri]`, l'attente d'une
  ligne arrivée pendant qu'elle parle), et où vivent les réglages (`places` : le dossier de Capri
  et celui de Claude Code, calculés une seule fois pour le hook et pour l'app, l'environnement
  passé par l'appelant). Les phrases que disent à la fois `capri hook` et la fenêtre de réglages
  sur Claude Code y sont écrites une fois. Avec des tests.
- `capri-chat` : la conversation, **sans dépendance système ni réseau** : historique borné, format
  des fournisseurs (ce qu'on envoie, comment lire le flux : `wire::Wire` dit lequel parle chaque
  fournisseur, `openai` et `anthropic` les décrivent, chemins, en-têtes, corps, flux, liste des
  modèles), fournisseurs et besoin d'une clé (`brain`), règles d'adresse et règles de la clé
  (`endpoint`, `key` : jamais en clair, liée à son adresse, et l'en-tête qui la porte,
  `key::key_header` : son nom et sa valeur sont décidés ici), réglages du cerveau, personnalité,
  nettoyage du Markdown, et les effets de voix (`voice` : les préréglages `VoiceEffect` écrits
  pour Capri, l'intensité, la hauteur, le débit, `voice.json`, et `delivery()` qui en fait la
  description neutre). Avec des tests. Les phrases que disent à la fois `capri chat` et la fenêtre
  de réglages (où part ce qu'on lui dit, clé rangée ou retirée…) sont dans `phrases`, écrites une
  seule fois. Le modèle de la fenêtre de réglages est dans `settings` (voir plus bas) ; sa
  rubrique « Claude Code » dépend de `capri-claude` (pur lui aussi) pour ses états et ses phrases.
- `capri-net` : **le seul crate qui parle au réseau** (ureq). Il se connecte et rien d'autre :
  quoi envoyer et comment lire la réponse est décidé par `capri-chat` (`Wire`, `key_header`, qu'il
  pose tels quels). En `https://`, le TLS du système (fonction `native-tls-no-default` de ureq,
  `RootCerts::PlatformVerifier`), sur macOS et Windows seulement ; sous Linux, pas de TLS (il y
  faudrait OpenSSL) et `https://` est refusé. Testé contre un faux serveur local ; aucun test ne
  sort de la machine.
- `capri-render` : wgpu, et le tracé du texte du bandeau et de la bulle (cosmic-text, polices du
  système). Ses images sont confiées une fois (`Renderer::add_image`, qui compte ce qu'elles
  pèsent) ; chaque dessin en prend une part (`Crop` : une case de grille), lue à un demi-texel des
  bords de la case. `Offscreen` fait le même rendu dans une image en mémoire, pour les essais.
  Couleurs prémultipliées de bout en bout.
- Les appels système et l'`unsafe` lié à l'OS vivent dans **deux crates, et nulle part
  ailleurs** : `capri-platform` (la fenêtre, la voix, le micro, le trousseau, la liste des
  fenêtres) et `capri-ipc` (le canal local).
- `capri-ipc` : socket Unix ou tube nommé Windows, réservés à l'utilisateur. À part de
  `capri-platform` parce que `capri-hook` le charge à chaque événement de Claude Code : ni winit
  ni AppKit ici.
- `capri-hook` : le binaire que les hooks appellent. Deux dépendances (`capri-claude`,
  `capri-ipc`), rien d'autre. À chaque appel, il relit le réglage du miroir dans le dossier de
  Capri (`capri_claude::places`, le même que l'app) ; désactivé ou douteux, il ne lit pas la
  réponse de Claude.
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
  Sa voix vit ici aussi (`voice.rs`, `macos/voice.rs`, `macos/voice_stage.rs`) ; ce qu'elle dit,
  avec quelle voix et quels effets est décidé par `capri_chat::voice`, qui est pur, et reçu sous la
  forme neutre de `capri_core::sound::Delivery`. Deux chemins, une seule file qui garde l'ordre
  entre eux : sans traitement, le **chemin direct** (`speakUtterance`), celui d'avant les effets,
  inchangé — à hauteur et débit normaux, l'énoncé ne reçoit que sa voix (même une valeur égale à
  la normale change le rendu de certaines voix) ; avec, le **chemin traité** : un second
  synthétiseur rend la phrase en mémoire (`writeUtterance:toBufferCallback:`), un `AVAudioEngine`
  la joue (lecteur au format de la voix → mélangeur vers 48 kHz stéréo → unités d'effet →
  sortie). **Règle du repli** : au moindre problème du chemin traité (chaîne impossible, format
  illisible, moteur qui ne démarre pas ou s'arrête, son qui ne sort pas, rien de rendu), ce qui
  n'a pas été joué jusqu'au bout est redit par le chemin direct, sans effet, avec une ligne
  `warn` dans le journal ; les effets reviennent au prochain `stop`. Une panne des effets ne doit
  jamais la rendre muette ni la faire planter. Les rappels du système (paquets, fin d'énoncé,
  minuteur) ne portent qu'un numéro et retrouvent la voix dans un registre propre au thread
  principal ; seuls les blocs de fin de paquet, appelés ailleurs, ne touchent qu'à des atomiques.
  Le moteur ne tourne que pendant qu'elle parle (plus 1,5 s de queue), le minuteur aussi.
  Son oreille de même (`ear.rs`, `macos/ear.rs` : `AVAudioEngine` et le framework Speech) ; le
  déroulé d'une prise de parole est dans `capri_chat::dictation`, pur lui aussi.
  Le trousseau aussi (`keychain.rs` ; `macos/keychain.rs` : les `SecItem` de
  `security-framework`, déjà tiré par native-tls ; `windows/keychain.rs` : `CredWriteW`,
  `CredReadW`, `CredDeleteW` ; ailleurs : refus) et la saisie d'une clé sans écho
  (`terminal.rs` : `readpassphrase(3)` sur macOS, qui rend l'écho même sur un signal ; mode de la
  console sous Windows ; ailleurs : refus).
  La liste des fenêtres des autres apps aussi (`desk.rs` ; `macos/desk.rs` :
  `CGWindowListCopyWindowInfo` de CoreGraphics, dont le tableau « Copy » est libéré par
  `CFRetained` ; `windows/desk.rs` : `EnumWindows` et DWM, jamais exécuté ; ailleurs : refus), rendue
  sous la forme neutre de `capri_core::perch::DeskWindow`, dans le repère du système ;
  `Overlay::desk_frame` dit où est l'overlay dans ce repère.
  Si un son sort d'une autre app aussi (`music.rs` ; `macos/music.rs` : deux fonctions de
  CoreAudio déclarées à la main d'après `AudioHardware.h` ; ailleurs : refus), décidé par
  `capri_core::music` (horloge passée en paramètre).
- `capri-app` : assemble le tout. Pas d'appel système direct ici (sauf menus via `tray-icon`).
  La fenêtre passe toujours par `capri_platform::Overlay`, jamais par `winit::window::Window`.
  Les clés passent par `keys` (le trait `KeyStore` : le trousseau du système, ou un trousseau en
  mémoire pour les tests) ; les commandes `capri chat` par la structure `Cli` de `brain_setup`,
  qui reçoit son dossier, son trousseau, sa saisie et sa sortie : c'est ce qui les rend testables.
  `settings_view` relie le modèle des réglages à la fenêtre et exécute ses effets avec les
  fonctions de la ligne de commande (`brain_setup::load`, `save`, `list_models`, `talk`,
  `keys::store`, `keys::forget`, `voice_setup`, `hook_setup::install`, `uninstall`,
  `load_mirror`, `save_mirror`, `style_setup`) : la fenêtre et `capri chat` / `capri voice` /
  `capri hook` restent d'accord parce qu'elles passent par le même code. Trousseau et réseau sur
  un thread à part, résultats relevés à chaque pas de la boucle. `style_setup` écrit le style de
  sortie dans `output-styles/` du dossier de Claude Code ; `claude::Feed` reçoit la ligne du
  miroir, que `banner_view` fait dire par `chat::Chat::relay` quand Capri est libre.
  Sa taille est retenue par `size_setup` (`size.json` du dossier de Capri, rien d'autre) et
  appliquée par `Running::resize` (menus « Taille », et `OverlayEvent::Zoom` : la molette sur elle
  avec `capri_platform::ZOOM_MODIFIER` tenue, ⌥ sur macOS, Ctrl sous Windows).
  `voice_setup::delivery` fait la description de la voix pour la conversation, « Écouter » et
  `capri voice test` ; un changement fait dans la fenêtre est relu par `Chat::refresh_voice` dès
  la phrase suivante. `voice_rehearsal` (version de débogage seulement) est `capri voice
  rehearse` : la vraie voix, avec et sans effets, exercée sans un bruit (ordre, fin, pause, arrêt,
  repli sur quatre pannes simulées, rendu hors ligne de chaque préréglage).
  `perch_view` lit la liste des fenêtres quand elle sert et fait faire à Capri ce que décide
  `capri_core::perch` ; `perch_setup` lit et écrit `perch.json` (la case « S'asseoir sur les
  fenêtres » de son menu). `perch_rehearsal` (version de débogage seulement) est `capri perch
  rehearse` : la vraie liste, lue et triée sans rien afficher, qui n'imprime que des nombres.
  `figure` confie ses images à la carte graphique (les planches lues par `assets`, puis lâchées),
  fait les masques de clic, et traduit ce que décide `capri_core::look` en dessins pour le rendu ;
  `portraits` (essais ignorés) rend de vraies images de chaque pose par ce même code, pour les
  regarder : à relancer après tout changement de ce qui se voit,
  `CAPRI_PORTRAITS_DIR=… cargo test --release -p capri-app portraits -- --ignored`.
  `music_view` décide quand écouter si un son joue (sur son thread « capri-musique ») et ce
  qu'elle en conclut ; `music_setup` lit et écrit `music.json` (la case « Se balancer sur la
  musique » de son menu). `music_rehearsal` est `capri music rehearse` (version de débogage).

## La fenêtre de réglages : ajouter une rubrique

Le modèle (`capri_chat::settings`) est pur et testé sans écran : à partir des réglages lus, des
`FormEvent` de l'utilisateur et des `Outcome` des opérations lancées, il produit le `Form` à
afficher et des `Effect` à exécuter. Chaque onglet est une rubrique, qui implémente le trait
`Rubric` (`brain.rs` : « Cerveau », `voice.rs` : « Voix », `claude.rs` : « Claude Code »).

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
le vrai champ secret, et quitte ; le journal dit ce qu'elle affiche, jamais la clé. Avec aussi
`CLAUDE_CONFIG_DIR` d'essai (et seulement alors), elle installe et retire le style, branche et
débranche Claude Code par les boutons de la vraie fenêtre, puis coche et décoche le miroir.

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
  fonction C du système déclarée à la main (`readpassphrase`) se vérifie dans l'en-tête du SDK ;
  de même une méthode qu'objc2-avf-audio ne lie pas (`renderOffline:toBuffer:error:`, appelée par
  `msg_send!` dans les essais seulement). Les bornes et le comportement des unités d'effet se
  lisent dans les en-têtes d'AVFAudio du SDK (`AVAudioUnitReverb.h`, `…Delay.h`,
  `…Distortion.h`, `…EQ.h`, `AVAudioPlayerNode.h`, `AVSpeechSynthesis.h`). Ce que la
  documentation ne dit pas et qui a été **mesuré** sur ce Mac (à revérifier si le système
  change) : les paquets de `writeUtterance` arrivent sur le thread principal par sa boucle ; deux
  énoncés confiés à la suite sont rendus en même temps et peuvent finir dans le désordre (n'en
  confier qu'un à la fois) ; un paquet vide n'est pas la fin (seul `didFinishSpeechUtterance` du
  délégué l'est) ; `isSpeaking` reste faux pendant un tel rendu et juste après `speakUtterance`.
- **Exceptions Objective-C** : AVFAudio en lève sur un format ou un branchement invalide, et Rust
  ne sait pas les attraper (le processus s'arrête). La fonction `exception` d'objc2 le permettrait
  mais tire `objc2-exception-helper`, qui compile du C : pas ajoutée. Tout appel qui pourrait en
  lever est donc précédé de la vérification de ses conditions documentées (formats créés par nous,
  nœuds attachés avant d'être branchés, paquets au format même du branchement, `play` moteur
  démarré). Ne pas appeler une API d'AVFAudio sans relire ce qu'elle exige.
  Ces API changent souvent : vérifier dans les sources
  (`~/.cargo/registry/src`) ou docs.rs, pas de mémoire. Même règle pour Claude Code : le format
  des hooks se vérifie dans sa documentation (<https://code.claude.com/docs/en/hooks>).
- Chaque bloc `unsafe` a un commentaire `// SAFETY:` qui justifie pourquoi il est sûr.
- Commentaires et textes affichés en français ; identifiants en anglais.
- Animation façon anime : poses clés tenues un nombre entier d'images à 24 i/s, pas
  d'interpolation (voir `capri-core/src/anim.rs`). Cela vaut pour ses **dessins**, pas pour sa
  **position** : ce qui la porte (le curseur qui la tient, la fenêtre où elle est posée, son vol)
  est suivi à la cadence de l'écran (`capri_core::pace`, `perch::Watch`, en secondes et non en
  images d'animation).
- Elle tourne toute la journée : ne redessiner que quand la pose ou la position change, rester
  léger en CPU et en batterie. Au repos, la boucle se réveille à 60 Hz (`pace::REST`) ; pressée
  seulement (tenue, en vol, ou sa fenêtre qui bouge ou un bouton de la souris enfoncé, plus une
  demi-seconde), elle suit l'écran : `Overlay::follow_refresh` (sur macOS 14 et suivants, un
  `CADisplayLink` de la vue, en pause le reste du temps ; ailleurs, refus, et un `pace::Clock` à
  la fréquence de `Overlay::refresh_rate`), et seuls les réveils de l'écran
  (`Overlay::refreshed`) refont le pas. Mesurer avant de changer la cadence :
  `CAPRI_PERCH_REHEARSAL=1` (version de débogage ; voir le README).
- **Ne jamais copier de code** de mochiiwuff/capri-desktop, CyberMoonie/capri-macos ou
  lachydotmcg/capri-desktop (licence PolyForm Noncommercial). S'inspirer des idées, réécrire.
- **Images** : jamais commitées (`assets/capri/` est ignoré). Elles viennent de
  `tools/fetch-assets.sh` (et `.ps1`), qui les prend dans leur dépôt d'origine à une révision
  figée et vérifie leurs empreintes. Le code les charge au lancement. Sa pose assise
  (`assets/capri/perch/`) vient de lachydotmcg/capri-desktop : seulement la famille « perch »
  créditée à l'artiste du projet, jamais une image créditée à quelqu'un d'autre, et ses mentions
  obligatoires recopiées dans `CREDITS.md`. Son chargement est tolérant : sans elles, elle se pose
  debout. Sa pose debout (`assets/capri/stand/`) en vient aussi, aux mêmes conditions (la boucle
  `idle`, son clignement et sa bouche ; pas l'ancien jeu de cases de 240 px) ; sans elle, elle
  garde `capri-cutout.png`. **Le repli** : seul `capri-cutout.png` est indispensable ; une planche
  absente, illisible ou refusée par la carte graphique ne fait jamais planter, sa pose revient à
  son dessin d'avant (`look::Wardrobe`), et chaque combinaison (debout seule, assise seule, aucune)
  est testée.
  Dans un worktree sans images, utiliser `CAPRI_ASSETS=/chemin/vers/assets`.
- Aucune télémétrie, rien n'envoyé sur le réseau sans action explicite de l'utilisateur. La
  fenêtre de réglages ne demande d'elle-même la liste des modèles qu'à un serveur de cette
  machine ; à tout autre, seulement quand on appuie sur « Demander la liste ».
- Le chat : seules la personnalité et la conversation partent vers le modèle, et seulement vers
  l'adresse que l'utilisateur a choisie. Jamais l'activité de Claude Code, ni la ligne du miroir
  (`Chat::relay` ne l'ajoute pas à la conversation). Pas de proxy, aucune
  redirection suivie. `http://` seulement vers la machine ou un réseau privé : toute adresse
  passe par `capri_chat::Endpoint`. Ce qui se dit n'est jamais écrit dans le journal.
- Sa voix : des voix existantes (celles du système pour l'instant), **jamais un clone de la voix
  de la comédienne de la série**, aucun modèle de voix téléchargé. Les effets sont un traitement
  du son appliqué après la synthèse, sur la machine, avec des préréglages écrits par nous (l'idée
  vient de CapriChooms/CapriCore : ni son code ni ses valeurs). « Naturelle » (le défaut) garde le
  chemin direct d'avant, inchangé. Rien n'est enregistré ni envoyé. Désactivée par défaut. Les
  tests ne font aucun bruit et ne créent aucune vraie voix ; la seule exception est le test ignoré
  `crates/capri-app/tests/voix_en_silence.rs`, qui lance `capri voice rehearse` (version de
  débogage) : de vraies voix et un vrai moteur, à volume nul ou rendus hors ligne, sans lire aucun
  réglage. Le relancer après tout changement de la voix :
  `cargo test -p capri-app --test voix_en_silence -- --ignored`. Ce qu'il ne prouve pas : que
  c'est agréable à entendre ; cela se vérifie à l'oreille (liste dans le README).
- La musique : savoir si un son joue, c'est **un oui ou un non, rien d'autre**. Jamais le titre,
  l'artiste, l'album ni le nom de l'app : ni lus (pas de `kAudioProcessPropertyBundleID`, pas de
  notification de lecteur qui les transporte), ni gardés, ni écrits dans le journal, ni affichés,
  ni envoyés à son cerveau. **Aucune permission** (ni AppleScript, ni Automatisation, ni capture
  du son) et **aucune API privée** (pas de MediaRemote). Rien ne part sur le réseau. L'écoute ne
  tourne que **quand elle sert** : assise (avec ses planches), visible, et permise par la case de
  son menu (`music.json`) ; une fois par seconde, sur son propre thread (la première écoute d'un
  processus prend environ 70 ms : jamais sur le chemin d'une image) ; oubliée dès qu'elle ne sert
  plus. Sa propre voix ne compte pas (mesuré : le système l'attribue à son processus, que
  l'écoute écarte) ; à revérifier après tout changement de la voix ou de l'écoute avec
  `capri music rehearse`, qui le compte sans un bruit. Debout, la musique ne change rien.
- Son oreille : le micro ne s'ouvre que sur un **geste de l'utilisateur** (le raccourci, ou le
  menu), jamais autrement : pas d'écoute permanente, pas de mot d'éveil, pas de réouverture
  automatique après une réponse. Il se ferme toujours tout seul : au relâchement (touche maintenue),
  à un nouvel appui, à la fin de la phrase (écoute libre : un appui bref ou le menu ;
  `capri_chat::dictation` la décide d'après l'arrivée des textes provisoires et une horloge passée
  en paramètre, sans mesurer le son : plus de texte nouveau après des mots, ou rien du tout au
  bout d'un délai),
  ou à la **borne dure** de 55 s, sous la minute qu'Apple documente (`SFSpeechRecognizer.h`),
  pour les deux gestes. Ces délais sont des constantes nommées, pas des réglages. La
  transcription est **exigée sur l'appareil** (`setRequiresOnDeviceRecognition(true)` : jamais
  de repli sur un serveur), rien n'est enregistré, et ce qui se dit n'est jamais écrit dans le
  journal. Capri ne demande une permission que si son
  `Info.plist` l'explique (`tools/macos/Info.plist`) : sinon macOS tue le processus.
- Raccourcis globaux : une touche y est désignée par sa place, pas par sa lettre. Ne choisir que
  des lettres à la même place en AZERTY et en QWERTY (ni A, Q, W, Z, M, ni la ponctuation).
- **Les fenêtres des autres apps** : **aucune permission** (ni Accessibilité, ni Enregistrement de
  l'écran, ni rien qui les demanderait), et **jamais le titre d'une fenêtre ni le nom d'une app**,
  ni lus, ni gardés, ni écrits dans le journal : sur macOS, seules les clés que `CGWindow.h`
  promet dans tout dictionnaire de fenêtre (numéro, couche, rectangle, opacité, processus,
  comparé au nôtre puis oublié) et `kCGWindowIsOnscreen`, jamais `kCGWindowName` ni
  `kCGWindowOwnerName`. Capri ne garde que des numéros, des rectangles et leur ordre ; le journal
  et `capri perch rehearse` ne disent que des nombres (le test ignoré `fenetres_du_bureau` le
  vérifie : à relancer après tout changement de la liste,
  `cargo test -p capri-app --test fenetres_du_bureau -- --ignored`). La liste n'est lue que
  quand elle sert (au lâcher, puis posée, à la cadence de `perch::Watch`), jamais quand elle est
  cachée ; son coût se mesure avec le test ignoré `liste_des_fenetres` de `capri-platform`, en
  version optimisée, avant de changer une cadence.
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
  et son `Debug` la masque ; son texte ne sort que pour le trousseau et pour l'en-tête qui la
  porte (`key::KeyHeader`, au `Debug` masqué lui aussi : `Authorization: Bearer` pour le format
  d'OpenAI, `x-api-key` pour Anthropic).
  Une clé ne part qu'en `https://`, ou en `http://` vers cette machine même
  (`Endpoint::may_carry_secret`, vérifié par `capri-net` avant toute connexion) ; jamais vers un
  fournisseur qui n'en prend pas. Un message de fournisseur est masqué (`ApiKey::redact`) avant
  d'être cité. Celui d'un refus de la clé (401) est cité aussi, parce que lui seul dit pourquoi
  (clé inconnue, mal formée, révoquée), mais masqué jusqu'au moindre morceau
  (`redact_any_trace`) : des fournisseurs y rappellent le début ou la fin de la clé. Le journal
  de l'app ne contient jamais les traces
  `trace` de ureq (`NoWireLog` dans `main.rs`), qui écrivent les octets échangés sans masque.
- Trousseau et tests : un test qui touche au vrai trousseau est `#[ignore]`, n'utilise qu'un
  service réservé aux tests et nettoie derrière lui. Pour des essais en ligne de commande :
  `CAPRI_CONFIG_DIR=<dossier temporaire>` et `CAPRI_KEYCHAIN_SERVICE=<service d'essai>`, et ne
  jamais lire d'autre élément du trousseau. Aucun appel authentifié à un vrai fournisseur sans
  la clé et l'accord de l'utilisateur.
- Le canal du hook : socket Unix (Windows : named pipe) accessible au seul utilisateur, jamais
  un port TCP sans authentification.
- `capri-hook` ne doit **jamais gêner Claude Code** : rien sur la sortie standard, toujours le
  code de sortie 0, une durée de vie bornée, miroir activé ou non, réglage lisible ou non, quelle
  que soit la taille de la réponse. Les hooks sont déclarés `async`.
- **Capri n'utilise jamais l'abonnement de l'utilisateur** : elle ne lance jamais Claude Code (ni
  `claude`, ni `claude -p`, ni une session), et aucun code ne doit le faire. Le cerveau Anthropic
  ne prend qu'une **clé d'API**, donnée par l'utilisateur et rangée dans le trousseau comme les
  autres : jamais un jeton OAuth, l'outil `ant`, `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN` ni
  les identifiants de Claude Code. Son format (version `anthropic-version`, `max_tokens`, ce qu'on
  n'envoie pas : `temperature`, `top_p`, `top_k`, `thinking`, `fallbacks`) se vérifie dans la
  documentation d'Anthropic, citée dans `capri_chat::anthropic`, pas de mémoire. Aucun test ne
  contacte `api.anthropic.com`. C'est l'utilisateur qui parle à Claude Code dans ses propres
  sessions ; Capri y prend part par un style de sortie qu'il choisit lui-même, et par le miroir.
- Le style de sortie (`capri_claude::style`) ne s'écrit **que sur demande** (`capri hook style
  install`, ou le bouton de la fenêtre), dans `output-styles/` du dossier de Claude Code
  (`CLAUDE_CONFIG_DIR` respectée), d'un seul coup. On n'écrase ni ne retire jamais un fichier qui
  ne porte pas notre signature (le commentaire `# capri-managed` de son en-tête), ni un lien, ni
  ce qui n'est pas un fichier ordinaire. **Capri n'active pas le style** et ne touche pas à
  `settings.json` pour cela : elle dit quoi taper dans Claude Code (`/output-style Capri`, vérifié
  dans <https://code.claude.com/docs/en/output-styles>). Son texte est le nôtre, pas une citation
  de la série, et garde `keep-coding-instructions: true`.
- Le bandeau ne transmet ni n'affiche jamais le texte d'un prompt, le contenu d'un fichier ou la
  sortie d'une commande. Dans le doute, il se tait plutôt que d'affirmer quelque chose de périmé.
  **Une seule exception, explicite et étroite : le miroir.** Activé par l'utilisateur seulement
  (désactivé par défaut), il ne transmet que la ligne que Claude a écrite pour Capri (la dernière
  qui commence par `[Capri]` dans `last_assistant_message`, sur le `Stop` de la session
  principale, hors des blocs de code, bornée à 240 caractères), par le canal local réservé à
  l'utilisateur ; rien n'est gardé ni envoyé ailleurs, et le journal n'en écrit que la longueur
  (`protocol::Said` masque son `Debug`). Elle n'entre pas dans la conversation envoyée au cerveau
  de Capri. Ce qu'on ne peut pas garantir : cette ligne est écrite par Claude, qui pourrait y citer
  un nom de fichier ou un bout de résultat malgré les consignes du style. Miroir désactivé, le hook
  ne lit même pas la réponse, et la ligne envoyée est la même qu'avant le miroir.
- Le protocole du hook n'évolue que par des champs facultatifs, sans changer de version : une
  Capri ancienne et un hook récent, ou l'inverse, se comprennent (testé avec une copie de
  l'ancien protocole).
- Au-dessus de sa tête, une seule chose à la fois : ce qu'on lui dit au micro d'abord, puis sa
  parole (on vient de lui parler, ou elle dit la ligne que Claude a écrite pour elle), puis un
  message de l'app, puis l'activité de Claude Code. La ligne de Claude passe après ce que
  l'utilisateur lui dit et ce qu'elle lui répond : arrivée pendant qu'elle parle, elle attend
  qu'elle ait fini (voix comprise) ; seule la plus récente attend, deux minutes au plus, et elle
  est oubliée si sa session repart, si on fait taire Capri, ou si le miroir est désactivé.
- **Ce qui s'affiche dans la bulle ou dans la fenêtre de réglages ne renvoie jamais à une commande
  de terminal** (`capri chat …`, `capri voice …`) : le but de la fenêtre est que personne n'ait à
  ouvrir un terminal. La bulle renvoie à « Réglages… » dans son menu, la fenêtre dit quoi faire
  sur place (« remets-la ci-dessus »). Seul un terminal garde ses commandes. Une phrase s'écrit
  une fois ; le conseil qui la termine se choisit par `capri_chat::Place` (`Terminal`, `Bubble`,
  `Window`), jamais en dupliquant la phrase. Ce qui se tape **dans Claude Code**
  (`/output-style Capri`, `/hooks`) n'est pas une commande de terminal : la fenêtre peut le dire. Là où la fenêtre ne sait pas encore faire la chose
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
