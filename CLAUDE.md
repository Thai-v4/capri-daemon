# Capri — consignes pour Claude Code

Compagne de bureau (Capri, *Cyberpunk: Edgerunners* 2) écrite en Rust, 100 % native, pour macOS
(cible principale, Apple Silicon) et Windows. Projet de fan non commercial. Le README décrit l'état
du projet et la feuille de route ; jalons 1 (elle flotte) et 2 (le bandeau Claude Code) faits,
jalon 3 (elle s'assoit sur tes fenêtres) écrit et testé sans écran, puis regardé sur un vrai Mac
le 12 novembre 2026 dans l'ensemble, pas point par point (sa boucle debout et son flottement vont
bien, elle s'assoit bien sur les fenêtres ; avec Spotify, elle se balance à la lecture et s'arrête
à la pause ; l'échelle de 1,20 entre ses deux poses pas encore jugée en mouvement, Musique jamais
essayée), jalon 4 (lui parler) en cours : saisie, bulle, modèle local (Ollama, LM Studio),
fournisseurs compatibles OpenAI avec leur clé dans le trousseau (OpenRouter, DeepSeek, ProxyAPI,
Ollama en ligne, serveur personnalisé), l'API d'Anthropic avec sa clé d'API (son propre format ; testée
contre un faux serveur local seulement, pas encore essayée avec une vraie clé), voix et écoute au
micro en place, effets de voix, hauteur et débit écrits, vérifiés sans bruit et écoutés dans
l'ensemble ; Capri dans Claude Code (un style de sortie, et le miroir qui lui fait dire la ligne
que Claude écrit pour elle) en place, essayé dans une vraie session le 11 novembre 2026 (dans
l'ensemble, pas cas par cas) ; elle détecte seule la fin de la phrase (un appui bref sur le
raccourci suffit ; pas essayé avec une vraie voix) ; le mot d'éveil est écarté (il demanderait un
micro ouvert en permanence). Sur macOS, une fenêtre de réglages
(« Réglages… » dans son menu) règle son cerveau, sa voix et Claude Code sans terminal. Sur
macOS toujours, « Se lancer à l'ouverture de session » (case de son menu, décochée par défaut)
l'inscrit par un agent de lancement et, cochée, lui fait lancer Ollama à chaque lancement s'il est
son cerveau sur cette machine et que rien n'y répond (écrit et testé sans ouverture de session ;
le 12 novembre 2026, case cochée dans le vrai menu et agent chargé à la main par `launchctl
bootstrap` : Capri lancée, et Ollama lancé par elle ; pas encore essayé à une vraie ouverture de
session). Jalon 5, sa première moitié : une `Capri.app` qui emporte ses images, installée dans
`/Applications` par `tools/install-macos.sh` (écrit, essayé vers un dossier temporaire, puis
installée pour de bon le 12 novembre 2026 : images lues dans l'app, inscription à l'ouverture de
session réparée, hook de Claude Code qui suit ; ni ce qui se voit, ni le micro, ni une vraie
ouverture de session vérifiés depuis). Sa signature : « ad hoc » par défaut, ou par l'identité
« Developer ID Application » du trousseau s'il y en a une (écrit, et essayé le 23 novembre 2026
vers un dossier temporaire avec la vraie identité de l'utilisateur, puis installée ainsi dans
`/Applications` ; le même jour, d'après l'utilisateur, macOS n'a pas redemandé le micro après la
mise à jour vers la 0.2.0, signée par la même identité : rapporté dans l'ensemble, le journal ne
disant pas si le micro a été ouvert depuis, une écoute réussie n'y écrivant rien). Pas de
notarisation ; ses mises à jour sont écrites, et essayées pour de bon sur macOS (voir plus bas).
Son journal, lancée comme app à fenêtre, s'écrit aussi dans un fichier borné
(`~/Library/Logs/Capri/capri.log` sur macOS), que « Montrer le journal » de son menu montre dans
le Finder (écrit et testé sans écran le 15 novembre 2026 ; le même jour, l'app réinstallée et
lancée depuis `/Applications` a écrit le fichier, dossier et fichier réservés à l'utilisateur, et
« Montrer le journal » a été essayé par l'utilisateur, dans l'ensemble, pas point par point).

Jalon 5, sa seconde moitié : des mises à jour à la manière de Droppy, en trois lots (publier les
versions ; savoir qu'une version existe ; la télécharger, la vérifier et l'installer, sur un
geste). Le premier est écrit le 23 novembre 2026 (`.github/workflows/release.yml`,
`tools/release-manifest.sh` essayé avec de faux fichiers) et lancé sur GitHub le même jour, en
répétition puis pour de bon : la version 0.2.0 est publiée (tag `v0.2.0`, six fichiers, tailles et
empreintes du manifeste contrôlées à part). Le deuxième est écrit et testé sans réseau le même jour : la case « Chercher les
mises à jour » de son menu (cochée par défaut), une vérification par jour sur un thread à elle,
ce qu'elle en dit, et `capri update check` ; répété dans la vraie app contre un faux serveur de
cette machine, puis, avec l'accord de l'utilisateur, contre GitHub par `capri update check` de
l'app installée (la 0.2.0 trouvée depuis la 0.1.0, par les vraies redirections), puis par la
veille de l'app elle-même (son journal : « version 0.2.0 disponible » une minute après son
lancement, et « à jour » une minute après la mise à jour) ; pas exécuté sous Windows. Le
troisième est écrit et testé sans écran le 23 novembre 2026 (macOS seulement) : sur un geste, la
ligne « Installer la version … » de son menu la télécharge, la vérifie, refait son app à côté de
l'installée et la signe comme elle ; « Redémarrer pour passer à la … » la met en place par le
programme de fin de la nouvelle app. Répété de bout en bout depuis une `Capri.app` d'essai signée
« ad hoc », contre un faux serveur de cette machine, puis, le même jour, depuis la même app
d'essai signée par la vraie identité de l'utilisateur (la nouvelle signée par elle aussi, son nom
écrit nulle part). Puis essayé pour de bon le même jour, par l'utilisateur, sur son Mac : la Capri
installée en 0.1.0 dans `/Applications`, signée par son identité, a téléchargé la 0.2.0 de GitHub
sur son geste (les trois fichiers, tailles et empreintes vérifiées, en une seconde), l'a refaite
et signée par la même identité, puis, au redémarrage, le programme de fin l'a mise en place et
relancée par `open` (tout le relais dans la même seconde, d'après son journal) ; ensuite
`/Applications/Capri.app` est en 0.2.0, sa signature se vérifie, rien n'est resté à côté. D'après
l'utilisateur : aucune fenêtre du trousseau, elle est revenue seule, macOS n'a pas redemandé le
micro ; ce qui se voit est rapporté dans l'ensemble, pas étape par étape. Pas essayé : un échec
en vrai (réseau coupé, relais manqué), une app « ad hoc » mise à jour pour de bon, une identité
qui n'est plus dans le trousseau.

Avant l'installeur, « lui donner vie », demandé par l'utilisateur le 23 novembre 2026, en cinq
lots : elle apparaît et s'efface ; une intro en plein écran au démarrage ; elle va s'asseoir
seule sur les fenêtres ; elle bouge et réagit ; elle se repose. Le premier est écrit et testé
sans écran le même jour (ses planches `arrive` et `leave`, au lancement et quand on la masque ou
la montre ; portraits rendus et regardés) ; pas encore regardé en mouvement.

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
  clignement) dans les grilles confiées à la carte graphique. Et dans `sheets::life` les planches
  de sa vie : des cases de 240 px d'un dessin plus ancien d'elle (de face, un bras tendu), pour
  l'instant `arrive` et `leave`, à la même échelle que sa pose debout et alignées sur elle par
  ses pieds et sa hauteur (`STAND_LEFT` : la case de sa boucle commence 73 px plus à droite).
  Une planche de plus s'y ajoute sans rien refaire (sa doc dit comment).
  Et `look` : **son dessin**, quelle case montrer à chaque image (debout, la boucle et ses
  clignements aux tours de `BLINKS`, sa bouche posée par-dessus quand elle parle ; posée,
  l'arrivée, sa boucle assise, parlante ou en musique, le départ), où (sa ligne d'assise sur le
  bord), ce qui l'attrape (un masque par pose, l'union de sa boucle), ce qu'il lui faut sur un bord
  (`Look::stature`, que `perch` reçoit), et le repli sur son dessin d'avant quand des planches
  manquent (`Wardrobe`). Et son apparition et son départ (`Look::enter` au lancement, `appear`,
  `vanish` ; `is_here`, `is_away`, `is_gone`) : `arrive` puis sa boucle, par une **coupe franche**
  (ce ne sont pas le même dessin : ni fondu ni interpolation), `leave` avant que la fenêtre ne se
  cache ; montrée pendant qu'elle s'efface, ou cachée pendant qu'elle apparaît, l'animation est
  rejouée à l'envers depuis le dessin où elle en est ; assise, `perch_leave` et `perch_arrive` ;
  sans ses planches de vie, tout est immédiat, comme avant. `body` ne connaît que son image
  debout, le point posé sur le bord (`Capri::seat`), combien d'images dure son départ, et le tour
  que partagent son flottement et sa boucle debout (`Capri::tour`, `still`, `unstill` : arrêté
  pendant qu'elle apparaît ou s'efface, il reprend où il s'était arrêté).
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
  Et `music` : de la musique joue-t-elle, d'après l'état que les lecteurs connus ont annoncé
  (`PlayerState`, lu de la seule valeur de `STATE_KEY` : trois valeurs exactes, tout le reste vaut
  « ne joue pas ») ; sans délai (une lecture compte tout de suite, une pause l'arrête tout de
  suite ; `look` ne prend et ne quitte la musique qu'au début d'un tour, ce qui suffit contre le
  clignotement) ; et quand vérifier qu'un lecteur cru en lecture tourne encore (`Alive`, toutes
  les `ALIVE_EVERY`, horloge passée en paramètre). Il ne sait pas de quel lecteur il s'agit.
  Et `login` : le texte de son **inscription à l'ouverture de session** (un agent de lancement
  `launchd.plist(5)` : `LABEL`, `FILE_NAME`, `/usr/bin/open -g -a <Capri.app>`, `RunAtLoad`, session
  `Aqua`, `AssociatedBundleIdentifiers`), la reconnaissance du nôtre (`LoginState::of` : le
  commentaire `MARK` avant `<plist>`), l'app qu'il lance (`registered_app`), sa réparation quand
  l'app a changé de place (`repair`), et l'app autour d'un programme (`app_bundle`).
  Et `journal` : quand son **journal en fichier** passe au fichier suivant (`must_rotate` : avant
  la ligne qui ferait dépasser `MAX_BYTES`, et au lancement s'il la dépasse déjà) et la ligne trop
  longue rognée sans couper un caractère (`clip`). L'écriture est dans `capri-app` (`journal.rs`).
  Et `bundle` : ce qu'une mise à jour remplace dans `Capri.app` (`PROGRAM`, `HOOK_PROGRAM`,
  `BUNDLE_ID` repris de `login`), la lecture stricte d'un `Info.plist` en XML (`info_strings`,
  `info_string` : les chaînes du dictionnaire de premier niveau, les cinq entités de XML et
  rien d'autre, une clé en double refusée), les noms de ce qu'une mise à jour fabrique à côté de
  l'app (`staging_name`, `previous_name`, `leftover`, qui ne reconnaît qu'eux, au chiffre près ;
  `MARKER`, `MARKER_TEXT`, `NEXT`), et le protocole du relais entre l'ancienne Capri et le
  programme de fin (`HANDOVER_READY`, `HANDOVER_GO`). Et `codesign` : la signature de l'app lue
  dans ce qu'écrivent `codesign -d --verbose=2` (`signature` : `Signature=adhoc`, ou le premier
  `Authority=`) et `security find-identity -v -p codesigning` (`identities`), le choix de
  l'identité (`choose` : « ad hoc » comme l'installée, la seule identité valide du même nom, aucune
  et « ad hoc », plusieurs et refus ; le trousseau n'est pas lu pour une app « ad hoc »), et les
  arguments de `codesign` (`sign_args`, toujours `--timestamp=none`). `IdentityName` et
  `IdentityHash` n'ont pas de `Display`, et leur `Debug` les masque.
- `capri-claude` : tout ce que Capri sait de Claude Code, lui aussi **sans dépendance système** :
  lecture de l'entrée des hooks, protocole (et la forme pour le journal d'un message,
  `Message::for_log`), suivi des sessions, édition de `settings.json` de
  texte à texte, le style de sortie « Capri » (`style` : son texte, la signature qui reconnaît le
  nôtre), le miroir (`mirror` : son réglage, l'extraction de la ligne `[Capri]`, l'attente d'une
  ligne arrivée pendant qu'elle parle), et où vivent les réglages (`places` : le dossier de Capri
  et celui de Claude Code, calculés une seule fois pour le hook et pour l'app, l'environnement
  passé par l'appelant ; et le fichier du journal de l'app, `log_file`, à côté de `capri_dir`
  parce qu'il suit la même règle de `CAPRI_CONFIG_DIR` : le hook ne s'en sert pas ; de même le
  dossier où elle s'inscrit à l'ouverture de session, `login_agents` : le vrai, ou
  `<CAPRI_CONFIG_DIR>/LaunchAgents` dès que la variable est posée). Les phrases
  que disent à la fois `capri hook` et la fenêtre de réglages
  sur Claude Code y sont écrites une fois. Avec des tests.
- `capri-chat` : la conversation, **sans dépendance système ni réseau** : historique borné, format
  des fournisseurs (ce qu'on envoie, comment lire le flux : `wire::Wire` dit lequel parle chaque
  fournisseur, `openai` et `anthropic` les décrivent, chemins, en-têtes, corps, flux, liste des
  modèles), fournisseurs et besoin d'une clé (`brain`), règles d'adresse et règles de la clé
  (`endpoint`, `key` : jamais en clair, liée à son adresse, et l'en-tête qui la porte,
  `key::key_header` : son nom et sa valeur sont décidés ici), réglages du cerveau, personnalité,
  nettoyage du Markdown, et les effets de voix (`voice` : les préréglages `VoiceEffect` écrits
  pour Capri, l'intensité, la hauteur, le débit, `voice.json`, et `delivery()` qui en fait la
  description neutre), et `startup` : faut-il lancer Ollama au lancement (`plan` : case cochée,
  fournisseur Ollama, adresse de cette machine, en `http`), où le chercher (`candidates` :
  emplacements fixes, l'app seulement pour l'adresse par défaut), `OLLAMA_HOST`, le déroulé (`Wake` :
  un délai de grâce, un seul lancement, l'attente qu'il réponde, horloge passée en paramètre) et ce
  qu'elle en dit (`say`). Avec des tests. Les phrases que disent à la fois `capri chat` et la fenêtre
  de réglages (où part ce qu'on lui dit, clé rangée ou retirée…) sont dans `phrases`, écrites une
  seule fois. Le modèle de la fenêtre de réglages est dans `settings` (voir plus bas) ; sa
  rubrique « Claude Code » dépend de `capri-claude` (pur lui aussi) pour ses états et ses phrases.
  Et `update` : savoir qu'une version plus récente existe, sans réseau. `Version`
  (`MAJEUR.MINEUR.CORRECTIF`, écriture canonique, car l'étiquette `v<version>` en est calculée) ;
  la lecture stricte du manifeste `latest.json` (`manifest` : champs inconnus ignorés à tous les
  niveaux, tout le reste refusé en entier ; un `format` inconnu n'est lu que pour sa version) ;
  le verdict (`verdict` : à jour, installable depuis l'app, ou pas, avec son `Obstacle` : `format`
  inconnu, `bundle` différent, cible absente) ; l'adresse du dépôt, écrite une fois
  (`REPOSITORY`), d'où sont calculées celle du manifeste et celle d'un fichier (`asset_url`,
  jamais lue dans le manifeste) ; la règle des redirections (`route::Route`, prise en paramètre
  par `capri-net` : `Route::github` en vrai, `Route::this_machine` dans les essais) ; quand
  regarder (`schedule`, horloge passée en paramètre) ; où regarder (`source` : jamais GitHub
  depuis un essai) ; ce qu'elle en dit (`say`, et `Failure`, les causes d'un échec). Ici plutôt
  que dans `capri-core` parce que `serde_json` y est déjà, et que `capri-net`, qui doit recevoir
  la règle, ne dépend que de ce crate. Et `update::digest` : l'empreinte SHA-256, écrite d'après
  FIPS 180-4, sans dépendance (essayée contre ses constantes recalculées en nombres entiers, les
  exemples de la norme et des empreintes de `shasum`). Et `update::install`, sur un geste : le
  plan (`plan` : une version `Installable` dont la cible macOS donne exactement `capri`,
  `capri-hook` et `Info.plist`, chacun à sa place, `Slot`), l'`Info.plist` reçu vérifié
  (`check_info` : notre identifiant de bundle, la version du manifeste deux fois, notre
  programme), les refus et leurs phrases (`Refusal`, `say` et `for_log`), et le déroulé, une
  machine à états (`Installation` : la ligne choisie, les étapes, l'app prête, le redémarrage,
  et la ligne du menu qui en découle). L'adresse d'un fichier se calcule par la source
  (`Source::asset_url` : `asset_url` chez GitHub, la même forme sous le faux serveur d'essai).
- `capri-net` : **le seul crate qui parle au réseau** (ureq). Il se connecte et rien d'autre :
  quoi envoyer et comment lire la réponse est décidé par `capri-chat` (`Wire`, `key_header`, qu'il
  pose tels quels). Et `knock` : quelque chose répond-il à une adresse **de cette machine
  seulement** (une connexion TCP ouverte et refermée, rien d'envoyé ; `localhost` vaut `127.0.0.1`
  puis `::1`, sans résolution ; toute autre adresse, ou `https`, refusée avant de se connecter).
  En `https://`, le TLS du système (fonction `native-tls-no-default` de ureq,
  `RootCerts::PlatformVerifier`), sur macOS et Windows seulement ; sous Linux, pas de TLS (il y
  faudrait OpenSSL) et `https://` est refusé. Testé contre un faux serveur local ; aucun test ne
  sort de la machine.
  Et `update::fetch` : le manifeste des versions, par une entrée **à part** de la conversation
  (aucun paramètre de clé, aucun `Ask`, aucun corps) : un `GET` qui suit lui-même, une à une,
  les redirections que la règle passée en paramètre permet (ureq à zéro redirection), sans proxy,
  sans `User-Agent` ni `Accept` (retirés : ureq les enverrait de lui-même), sans connexion
  gardée ouverte, borné en durée et en taille (64 Kio). Avec `update::download`, sur un geste
  seulement, les deux seules entrées qui suivent des redirections : le fichier d'une version, par
  la même règle et le même agent, écrit au fil de l'eau dans un fichier que l'appelant a créé,
  **exactement** à la taille du manifeste (refusé dès un `Content-Length` différent), son
  empreinte calculée pendant la lecture, borné en durée (connexion, silence à chaque lecture,
  échange entier) et par un arrêt demandé ; refusé, le fichier est retiré.
- `capri-render` : wgpu, et le tracé du texte du bandeau et de la bulle (cosmic-text, polices du
  système). Ses images sont confiées une fois (`Renderer::add_image`, qui compte ce qu'elles
  pèsent) ; chaque dessin en prend une part (`Crop` : une case de grille), lue à un demi-texel des
  bords de la case. `Offscreen` fait le même rendu dans une image en mémoire, pour les essais.
  Couleurs prémultipliées de bout en bout.
- Les appels système et l'`unsafe` lié à l'OS vivent dans **deux crates, et nulle part
  ailleurs** : `capri-platform` (la fenêtre, la voix, le micro, le trousseau, la liste des
  fenêtres, l'inscription à l'ouverture de session, le lancement d'Ollama, un fichier montré dans
  le Finder, la signature de l'app qu'une mise à jour refait et le relais vers son programme de
  fin) et `capri-ipc` (le canal local).
- `capri-ipc` : socket Unix ou tube nommé Windows, réservés à l'utilisateur. À part de
  `capri-platform` parce que `capri-hook` le charge à chaque événement de Claude Code : ni winit
  ni AppKit ici. Sous Windows, un tube n'a pas la file d'attente d'une socket : un thread
  (« capri-tube ») le sert sans arrêt et garde toujours une instance à l'écoute, pour qu'un hook
  arrivé pendant que Capri fait autre chose ne soit pas perdu (`windows.rs`, qui cite la
  documentation de Microsoft ; exécuté par la CI seulement, jamais sur un vrai PC).
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
  Le guetteur des annonces des lecteurs de musique connus aussi (`music.rs` : `Announcements`,
  les constantes `SPOTIFY`, `MUSIC` et `KNOWN` ; `macos/music.rs` :
  `NSDistributedNotificationCenter`, abonné avec `NSNotificationSuspensionBehaviorDeliverImmediately`
  par un observateur déclaré (`define_class!`, sélecteur `announced:`), qui ne vit que sur le
  thread principal, où l'en-tête dit que les annonces arrivent, et que `Drop` désabonne avant de
  le lâcher, le centre ne le retenant pas ; `+[NSRunningApplication
  runningApplicationsWithBundleIdentifier:]` pour un lecteur disparu ; ailleurs : refus,
  `MusicError::Unavailable`), rendu sous la forme neutre de `capri_core::music::PlayerState`. En
  version de débogage seulement : le lecteur d'essai (`Player::Rehearsal`, nom et identifiant
  donnés par l'appelant), l'abonnement ordinaire pour comparer, le poste d'une annonce d'essai,
  la suspension à la main.
  L'inscription à l'ouverture de session aussi (`login.rs` : `status`, `register`, `unregister`
  dans un dossier **passé en paramètre**, écriture d'un seul coup en `0644` par un fichier
  temporaire qui ne finit pas par `.plist`, jamais par-dessus ni à la place d'un fichier qui n'est
  pas le nôtre, d'un lien ou d'un dossier, aucun appel à `launchctl` ; seul `user_agents_dir` désigne
  le vrai `~/Library/LaunchAgents`, et ailleurs que sur macOS il refuse : `LoginError::Unavailable`),
  et le lancement d'Ollama (`launch.rs` : `start_program`, un programme par son chemin absolu,
  détaché dans son propre groupe de processus, entrée et sorties fermées, sa fin attendue par un
  thread « capri-lance » ; `open_app`, une app par `/usr/bin/open -g -a` ; `is_program`, `is_app` ;
  ailleurs : `LaunchError::Unavailable`). Ni l'un ni l'autre ne sait ce qu'est Ollama : c'est
  `capri_chat::startup` qui décide quoi lancer. Et `reveal` : un fichier montré dans le Finder
  par `/usr/bin/open -R`, chemin absolu seulement, `open` attendu par un thread « capri-montre »
  et jamais par la boucle (ailleurs : `LaunchError::Unavailable`). Et `hand_over`, le relais
  d'une mise à jour : le `capri` de la nouvelle app, détaché (son propre groupe), son entrée sur
  un tube (`std::io::pipe`, dont les deux bouts sont fermés à l'exécution d'un autre programme :
  seul le sien lui passe) ; sa première ligne lue par un thread « capri-relais » (`Relay::ready`),
  puis « go » écrit (`Relay::go`) et le bout du tube gardé jusqu'à la sortie
  (`Gone::keep_until_exit`). Rien d'autre n'est lu de lui ; aucun processus n'est signalé.
  La signature de l'app qu'une mise à jour refait aussi (`codesign.rs` : `describe`, `sign`,
  `verify` par `/usr/bin/codesign`, avec les arguments de `capri_core::codesign` ; `identities`
  par `/usr/bin/security find-identity -v -p codesigning`, appelée seulement pour une app
  installée signée par une identité ; leur sortie n'est jamais recopiée ; ailleurs :
  `CodesignError::Unavailable`).
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
  `music_view` tient l'abonnement aux annonces tant que la case est cochée (dès le lancement,
  assise ou non ; sur le thread principal, aucun thread à elle), vérifie qu'un lecteur cru en
  lecture tourne encore et dit si elle se balance ; `music_setup` lit et écrit `music.json` (la
  case « Se balancer sur la musique » de son menu). `music_rehearsal` (version de débogage
  seulement) est `capri music rehearse` (une annonce d'essai, postée par un autre processus,
  `capri music announce`, reçue et son état lu) et `CAPRI_MUSIC_REHEARSAL=1` au lancement de la
  vraie app (reçue alors que l'app n'est pas active, et livraison suspendue à la main), lancés
  par le test ignoré `musique_sans_titre`.
  `login_setup` tient la case « Se lancer à l'ouverture de session » (l'état lu dans le système
  par `capri_platform::login::status`, aucun fichier de réglages à part ; la réparation au
  lancement par `capri_core::login::repair` ; la bascule, qui n'inscrit que l'app d'où elle
  tourne, `this_app`, jamais un binaire nu ; le dossier, `agents_dir` et `checked_at_launch`,
  d'après `capri_claude::places::login_agents` : le vrai, ou le dossier d'essai que donne
  `CAPRI_CONFIG_DIR`, que la case reflète alors). `ollama_view` veille sur Ollama au lancement, case
  cochée, sur un thread « capri-ollama » (le cerveau lu là, `startup::plan`, `capri_net::knock`,
  `capri_platform::launch`), derrière un trait `Machine` que les essais remplacent par une fausse
  machine à l'horloge factice ; ce qu'elle en dit est relevé par la boucle (`App::handle_ollama`).
  `journal` installe son journal : `start`, appelé par `main` pour l'app à fenêtre seulement (ses
  commandes de terminal sont servies avant et n'écrivent aucun fichier ; l'essai
  `journal_des_commandes` les lance pour de vrai) ; `NoWireLog`, qui enveloppe deux journaux
  d'`env_logger` réglés par le même `RUST_LOG`, la sortie d'erreur (inchangée) et le fichier
  (`LogFile`, sans couleurs) ; et `show`, ce que fait « Montrer le journal » de ses menus (le
  fichier montré par `capri_platform::launch::reveal` s'il est là, sinon une phrase pour le
  bandeau).
  `update_setup` tient la case « Chercher les mises à jour » (`update.json`, cochée sans
  fichier, décochée s'il est illisible), ce qui tourne ici (`installed` : la version du
  `Cargo.toml`, `tools/macos/bundle-format` lu par `include_str!`, `capri_chat::update::TARGET`),
  où regarder (`source` : `CAPRI_CONFIG_DIR` ou une des `REHEARSALS`, nulle part ;
  `CAPRI_UPDATE_URL` en version de débogage seulement) et `capri update check`. `update_view`
  regarde sur un thread « capri-maj », derrière un trait `Line` que les essais remplacent par un
  faux monde à l'horloge factice ; la lâcher l'arrête, et rien ne l'attend en quittant. Ce
  qu'elle trouve est relevé par la boucle (`App::handle_updates`) : dit une fois au-dessus de sa
  tête par `BannerView::announce` (un message de l'app qui attend qu'elle soit visible et libre),
  et rappelé par une ligne glissée sous la case, dans ses deux menus (`update_view::Found` : le
  verdict et le manifeste dont il vient). Sur macOS, cette ligne est le geste qui installe
  (« Installer la version … »), puis celui qui redémarre (« Redémarrer pour passer à la … ») ;
  ailleurs, ou pour une version qui ne s'installe pas depuis l'app, « Version … disponible », qui
  dit pourquoi. `install_view` exécute ce qu'ordonne `capri_chat::update::install::Installation`
  (`App::choose_update`, `App::handle_install`, `App::exiting` qui abandonne tout en quittant).
  `update_install` (macOS seulement) : la préparation sur un thread « capri-installe »
  (`Preparation` ; `prepare`, derrière un trait `Workshop` que les essais remplacent : le dossier
  caché `Staging` en `0700`, sa marque verrouillée par `File::try_lock`, les fichiers créés par
  `create_new`, les dossiers par `create_dir` sans jamais recréer un parent retiré,
  `Contents/Resources` de l'installée recopié octet par octet, fichiers et dossiers seulement),
  l'arrêt borné (`Preparation::stop`, `STOP_WAIT`), les restes au lancement
  (`clear_leftovers`, appelé par `main` pour l'app à fenêtre), et le relais (`start_relay`,
  `complete_relay`, `HANDOVER_WAIT`). `update_finish` : `capri update finish` (le programme de fin)
  et, en version de débogage, `capri update rehearse`.
- `tools/` : `fetch-assets.sh` (et `.ps1`), qui télécharge les images et vérifie leurs
  empreintes ; `bundle-macos.sh`, qui fabrique `target/Capri.app` : les deux binaires, l'`Info.plist`
  (`tools/macos/Info.plist`), les images que lit le code et seulement elles (sa liste `IMAGES`, prise
  dans `assets/` ou dans le dossier de `CAPRI_ASSETS` ; l'essai
  `the_app_bundle_carries_exactly_the_images_the_code_reads` de `assets.rs` vérifie qu'elle suit
  `assets::SPRITE`, `capri_core::sheets`, `sheets::stand` et `sheets::life` ; une liste qui change
  change aussi le numéro de `tools/macos/bundle-format`, 2 depuis ses planches de vie, et une
  version d'un autre numéro ne s'installe pas depuis l'app : il faut `tools/install-macos.sh`)
  dans `Contents/Resources/assets/`, et
  `CREDITS.md` dans `Contents/Resources/`, et son icône `Contents/Resources/Capri.icns` (nommée par
  `CFBundleIconFile`), fabriquée sur place à partir de `capri-face.png` du même dossier d'images
  (`sips` puis `iconutil`, de 16 à 256 px : l'image mesure 256 px, jamais agrandie ; fichiers de
  travail dans un dossier temporaire retiré à la sortie). Cette image n'est **pas** dans `IMAGES`
  (le code ne la lit pas) : sans elle, l'app est fabriquée sans icône, avec une ligne qui le dit
  (l'essai `the_app_icon_is_named_alike…` vérifie que l'`Info.plist` et le script nomment le même
  fichier). Signée **après** la copie des ressources et la fabrication de l'icône, que la
  signature scelle : par `CAPRI_SIGN_IDENTITY` si elle est posée (un nom ou une empreinte, ou `-`
  pour « ad hoc »), sinon par la seule identité « Developer ID Application » valide du trousseau
  (`security find-identity -v -p codesigning`, dont il ne lit que les noms ; plusieurs : il
  s'arrête avant de compiler), sinon « ad hoc ». Toujours `--timestamp=none`, le second programme
  d'abord ; une signature qui échoue retire l'app à moitié faite (l'essai
  `signing_the_app_bundle_never_leaves_the_machine` vérifie l'horodatage refusé et l'absence
  d'outil de notarisation dans les deux scripts). Il ne télécharge rien : sans les images, il
  s'arrête et dit quoi lancer.
  L'app cherche ses images dans cet ordre (`assets::candidates`, essayé) : `CAPRI_ASSETS`, à côté de
  l'exécutable, `Contents/Resources/assets` de son app, puis le dépôt.
  `install-macos.sh [--no-hook] [dossier]` l'installe (par défaut dans `/Applications`) : refus si
  une Capri tourne depuis `target/Capri.app` ou depuis la destination (regardé avant de fabriquer,
  puis avant de mettre en place ; `pgrep`, `lsof`, `ps`, rien d'autre), `bundle-macos.sh`, copie
  complète sous un nom caché à côté et signature vérifiée, puis renommage (une installation
  précédente, seulement si c'est une Capri, mise de côté puis retirée, ou remise en place en cas
  d'échec ; `touch` de l'app installée ensuite, pour que le Finder relise son icône), le hook de
  Claude Code qui suit s'il était branché (`capri hook install
  --if-plugged` de l'app installée : `hook_setup::follow`, `capri_claude::settings::follow`), puis
  `target/Capri.app` retirée, pour que l'agent d'ouverture de session se répare au lancement suivant
  (`login_setup::at_launch`, `capri_core::login::repair` : l'app inscrite n'existe plus).

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

Toute répétition de la vraie app (celle-ci, `CAPRI_PERCH_REHEARSAL`, `CAPRI_MUSIC_REHEARSAL`)
pose les quatre lieux d'essai : `CAPRI_CONFIG_DIR` (ses réglages, son journal, et son dossier
d'ouverture de session, `<CAPRI_CONFIG_DIR>/LaunchAgents` : rien n'est lu ni écrit dans le vrai
`~/Library/LaunchAgents` ; vide, la case est décochée et Ollama n'est pas veillé),
`CAPRI_KEYCHAIN_SERVICE`, `CAPRI_SOCKET` et `CLAUDE_CONFIG_DIR` (sans elle, elle lit le vrai
`~/.claude/settings.json`). Ce qu'elle touche encore hors de ces lieux : le système (la liste des
fenêtres, les annonces des vrais lecteurs si la musique est permise, ses raccourcis globaux, son
icône dans la barre des menus), ses images (`CAPRI_ASSETS`), et, dans la fenêtre de réglages,
la liste des modèles qu'elle demande d'elle-même à un serveur de cette machine.

Pendant une répétition (ou avec `CAPRI_CONFIG_DIR`), elle ne regarde jamais les versions : ni
l'app, ni `capri update check`. Une nouvelle répétition de la vraie app s'inscrit dans
`update_setup::REHEARSALS` (l'essai `every_rehearsal_the_app_reads_is_known_here` le vérifie), et
tout essai qui lance `capri` pose `CAPRI_CONFIG_DIR` (`every_test_that_launches_capri_sets_a_test_folder`).
Pour répéter la vérification : `CAPRI_UPDATE_URL=http://127.0.0.1:<port>/…` (version de débogage),
un faux serveur de cette machine et rien d'autre, regardé 3 s après le lancement
(`CAPRI_ASSETS=… cargo test -p capri-app --test mises_a_jour -- --ignored`).
Pour répéter l'installation : `capri update rehearse` (version de débogage, macOS), lancé depuis
une `Capri.app`, avec `CAPRI_CONFIG_DIR` et un `CAPRI_UPDATE_URL` de la forme de GitHub
(`http://127.0.0.1:<port>/…/releases/latest/download/latest.json` : ses fichiers sont cherchés sous
`…/releases/download/v<version>/`), refusé sans eux. L'essai ignoré
`cargo test -p capri-app --test mise_a_jour_installee -- --ignored --test-threads=1` le lance
depuis une petite `Capri.app` d'essai d'un dossier temporaire, signée « ad hoc » par le vrai
`codesign` (le vrai `capri` de débogage, un faux second programme, de fausses ressources), contre
un faux serveur qui redirige comme GitHub vers des adresses « signées » : l'app refaite, signée,
mise en place par le programme de fin, vérifiée ; et un fichier abîmé, qui n'installe rien. Il ne
relance pas l'app (avec `CAPRI_CONFIG_DIR`, le programme de fin saute la relance) et n'écrit ni
dans `/Applications`, ni dans `~/Library`, ni dans `~/.claude`. À relancer après tout changement
de l'installation. La signature par une identité s'y essaie à part, à la main, et **seulement
après l'avoir dit à l'utilisateur** : `CAPRI_SIGN_IDENTITY=<nom ou empreinte>` devant la commande
signe la petite app d'essai avec elle (la nouvelle doit alors l'être aussi, et son nom n'être
écrit nulle part), et macOS peut afficher sur son écran une fenêtre du trousseau qui demande
d'autoriser `codesign` (c'est arrivé le 23 novembre 2026).

## Vérifier avant de dire « c'est fait »

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings
cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings
cargo clippy --release --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --release --workspace
```

Les deux en version optimisée parce que du code n'existe qu'en version de débogage
(`debug_assertions`) : le 12 novembre 2026, deux défauts n'apparaissaient qu'ainsi (`1272f8a` :
un essai qui appelait une fonction de débogage seulement, et un `return` que clippy refuse
quand le bloc de débogage qui le suit disparaît).

Les essais tournent aussi sous Windows, dans la CI : un essai ne suppose pas des chemins à la
façon d'Unix. Les deux cas rencontrés (`6f8b876`) :
- un chemin assemblé par `join` et comparé comme du texte (sous Windows, `join` met `\`) :
  comparer des chemins, pas du texte ; et `is_absolute`, qui y exige une lettre de lecteur, sur
  un chemin écrit à la façon d'Unix : `has_root` ;
- un vrai dossier temporaire, dont le chemin porte une lettre de lecteur sous Windows, donné à du
  code qui exige à raison un chemin d'Unix (le texte de l'agent de lancement) : l'essai est
  réservé à Unix (`#[cfg(unix)]`), et le refus est essayé à part, avec des chemins écrits.

Le code Windows ne se teste pas depuis un Mac : la CI GitHub Actions le compile et le teste.
Les deux clippy croisés tiennent parce qu'aucune dépendance ne compile de C (sous Linux,
wayland-backend tire `cc`, mais ne s'en sert qu'avec sa fonction `log`, qui n'est pas active) :
pas de rustls (son fournisseur par défaut dans ureq, ring, compile du C, et son script de
construction échoue vers Windows depuis un Mac), pas d'OpenSSL. Ne pas en ajouter sans revérifier
les trois.
Tout comportement visible (transparence, clics, animation) se vérifie à la main : dire
précisément quoi vérifier plutôt que de l'affirmer.
Après un changement de `tools/bundle-macos.sh` ou de `tools/install-macos.sh` (bash 3.2, celui de
macOS : pas de tableau vide sous `set -u`, et `${VAR}` entre accolades devant un caractère non
ASCII) : `bash -n`, puis l'installation vers un dossier temporaire (vide, puis par-dessus), avec
`CLAUDE_CONFIG_DIR` et `CAPRI_CONFIG_DIR` d'essai, `codesign --verify --strict` de la copie, et le
faux `settings.json` regardé avant et après. Ces essais se font avec `CAPRI_SIGN_IDENTITY=-`
(sans elle, le script signe avec l'identité du trousseau, et macOS peut demander la clé) ; la
signature par une identité s'essaie à part, et les cas « aucune » et « plusieurs » identités par
une fausse commande `security` placée en tête du `PATH`.

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
  `idle`, son clignement et sa bouche) ; sans elle, elle garde `capri-cutout.png`. Les planches de
  sa vie (`assets/capri/life/`) en viennent aussi : l'ancien jeu de cases de 240 px, dont ce
  dépôt **ne nomme pas l'auteur**, écarté jusque-là et autorisé par l'utilisateur le 23 novembre
  2026 (sa licence dit que tout son dessin est de son artiste, et son README ne les range pas
  parmi les parties faites par d'autres) : `arrive` et `leave` aujourd'hui, et pour les lots
  suivants `walk`, `sleep`, `wake`, `dangle`, `glitch`, `grin`. `CREDITS.md` dit ce qu'on en sait,
  sans leur prêter un auteur. **Jamais une image que ce dépôt crédite à quelqu'un d'autre**
  (`click.png`, `form.png`, l'intro « Peek », les emotes) : une attribution se vérifie dans son
  README, sa licence et l'historique du fichier, pas de mémoire. Sans ses planches de vie, elle
  apparaît et disparaît d'un coup. **Le repli** : seul `capri-cutout.png` est indispensable ; une
  planche absente, illisible ou refusée par la carte graphique ne fait jamais planter, sa pose
  revient à son dessin d'avant (`look::Wardrobe`), et chaque combinaison (debout, assise, vie :
  les huit) est testée.
  Dans un worktree sans images, utiliser `CAPRI_ASSETS=/chemin/vers/assets`.
- **Une `Capri.app` qui contient les images ne se distribue jamais** : ni téléversée, ni publiée,
  ni partagée, ni jointe à une version, ni fabriquée par la CI. Les images ne sont pas à ce projet
  (PolyForm Noncommercial pour ses poses, captures et fan art pour son dessin d'avant), et chacun
  les télécharge pour soi : l'app de `tools/bundle-macos.sh` est faite sur une machine, pour cette
  machine. Son icône (`Capri.icns`), tirée de `capri-face.png`, en fait partie : jamais dans le
  dépôt (ni elle, ni un `.iconset`, ni une image de travail), seulement dans l'app fabriquée.
  La CI (`.github/workflows/ci.yml`) ne télécharge aucune image, ne lance ni
  `fetch-assets` ni `bundle-macos.sh`, et ne téléverse que les deux binaires (`capri`, `capri-hook`),
  qui ne contiennent aucune image (elles sont lues au lancement, jamais `include_bytes!`) : ne rien
  y ajouter qui en contienne.
  **La signer avec une identité « Developer ID Application » n'y change rien**, et se fait sur la
  machine sans rien envoyer : `--timestamp=none` à chaque signature (pas d'horodatage par le
  serveur d'Apple), et **jamais de notarisation** (`notarytool`, `altool`, `stapler`), qui la
  téléverserait chez Apple avec ses images. Le nom de l'identité, celui de son titulaire,
  l'identifiant d'équipe et l'empreinte du certificat ne s'écrivent **jamais dans le dépôt** (ni
  script, ni documentation, ni essai, ni message de commit) : le script les trouve dans le
  trousseau, ou les reçoit par `CAPRI_SIGN_IDENTITY`. Ni certificat ni clé dans le dépôt, ni dans
  la CI.
  Le workflow de publication (`.github/workflows/release.yml`, déclenché par un tag `v*.*.*`)
  obéit à la même règle et à celle de la signature : il ne nomme ni `fetch-assets`, ni
  `bundle-macos`, ni `install-macos`, ni `codesign`, ni `assets/`, ne signe rien, ne reçoit ni
  certificat ni clé, et ne joint à la version que les deux binaires de chaque système,
  `Info.plist` et `latest.json`. Un essai de `assets.rs` le vérifie, sur lui et sur `ci.yml`.
- `tools/release-manifest.sh <dossier> <version> <format> <bundle>` écrit `latest.json` sur la
  sortie standard (bash 3.2 et Ubuntu, `shasum` ou `sha256sum`, pas de `jq`, aucun réseau) ; le
  contrat du manifeste (noms des fichiers de la version, champs) est celui que lit l'app, et le
  workflow de publication l'appelle. `tools/macos/bundle-format` en donne le numéro de bundle.
- **`tools/install-macos.sh`** ne quitte, ne tue ni ne signale **jamais** Capri ni aucun processus :
  il refuse et dit de la quitter par son menu. Il ne demande jamais `sudo`. Il ne touche au
  `settings.json` de Claude Code **que par `capri hook install --if-plugged`** de l'app installée,
  et seulement pour faire suivre une inscription existante (`settings::follow` ne branche jamais
  un fichier où Capri ne l'est pas) : ni `sed`, ni `jq`, ni lecture du fichier par le script. Il
  n'écrit rien dans `~/Library/LaunchAgents` (c'est l'app qui répare son agent) et ne lance pas
  l'app. Ses essais se font vers un dossier temporaire, jamais vers `/Applications` ni
  `~/Applications`, avec `CLAUDE_CONFIG_DIR` et `CAPRI_CONFIG_DIR` d'essai posés à chaque
  lancement (sans eux, l'étape du hook toucherait le vrai `~/.claude/settings.json`).
- Aucune télémétrie, rien n'envoyé sur le réseau sans action explicite de l'utilisateur. La
  fenêtre de réglages ne demande d'elle-même la liste des modèles qu'à un serveur de cette
  machine ; à tout autre, seulement quand on appuie sur « Demander la liste ».
- **Une seule exception à la règle précédente, explicite et étroite : savoir qu'une version
  existe.** Décidée par l'utilisateur le 23 novembre 2026, comme Droppy : la case « Chercher les
  mises à jour » de son menu, **cochée par défaut**. Rien d'autre ne devient permis.
  1. **Ce qui part** : un `GET` sans corps du seul fichier `latest.json` de la dernière version
     publiée (`capri_chat::update::manifest_url`, calculée de `REPOSITORY`), et **rien sur
     l'utilisateur ni sur la machine** : la ligne de requête et `Host`, ni `User-Agent`, ni
     `Accept`, ni cookie, ni clé (`capri_net::update::fetch` n'en prend aucune, ni `Ask`). Ce qu'on
     ne peut pas cacher : **GitHub voit l'adresse IP** (et l'heure, et que ce fichier a été lu),
     et le réseau voit le nom des hôtes joints.
  2. **Quand** : `schedule::GRACE` (60 s) après son lancement, sur le thread « capri-maj » (rien
     ne retarde son apparition), puis une fois par jour de marche (`EVERY` ; la veille de la
     machine ne compte pas) ; après un échec, 15 min, puis le double, jamais plus d'un jour.
     Chaque lancement en refait une, une minute après.
  3. **Vers quels hôtes** : `route::GITHUB_HOSTS` seulement (`github.com`,
     `release-assets.githubusercontent.com`, d'après la documentation de GitHub citée dans
     `route.rs`), en `https`, au port 443, sans identifiants, trois redirections au plus, suivies
     une à une par la règle, ureq restant à zéro ; sans proxy ; 64 Kio au plus. Une autre liste
     se vérifie dans la documentation de GitHub, pas de mémoire.
  4. **Comment la couper** : décocher la case (`update.json` : `{ "enabled": false }`) ; un
     fichier illisible la coupe aussi. Le thread s'arrête, et elle oublie ce qu'elle avait trouvé.
  5. **Ce qui ne change pas** : elle ne télécharge ni n'installe rien d'elle-même (c'est un
     geste : voir la règle suivante) ; l'adresse d'un fichier se calcule (`asset_url`), jamais lue
     dans le manifeste ;
     rien n'en est envoyé à son cerveau ; le journal ne dit que la conclusion (« à jour », « version
     … disponible », « vérification impossible » et une cause courte), jamais une adresse signée
     (`Address` ne montre que l'hôte ; ureq masque le chemin au niveau `debug`, et ses traces sont
     refusées) ; la bulle et le menu ne renvoient à aucune commande.
  6. **Les essais et répétitions ne sortent jamais de la machine** : `CAPRI_CONFIG_DIR` posée ou
     une répétition en cours, elle ne regarde nulle part, ni l'app ni `capri update check`. En
     version de débogage seulement, `CAPRI_UPDATE_URL` désigne un faux serveur de cette machine
     (`http://127.0.0.1:<port>/…`, `Route::this_machine`) ; une autre adresse est refusée, jamais
     remplacée par GitHub. Aucun essai ne contacte GitHub, et le chemin réel ne s'essaie qu'avec
     l'accord de l'utilisateur.
- **Installer une version : seulement sur un geste, et sur macOS seulement.** Décidé par
  l'utilisateur le 23 novembre 2026. D'elle-même, elle ne fait toujours que lire le manifeste.
  1. **Le geste** : la ligne « Installer la version … » de son menu (là seulement si la version
     est `Installable` et que ce système sait l'installer ; ailleurs, la ligne dit pourquoi :
     Windows et Linux, une version qui change ce que contient l'app, avec alors le seul renvoi
     permis à une commande, `tools/install-macos.sh`, puisque l'app ne sait pas faire cette
     installation-là). Puis « Redémarrer pour passer à la … », le second geste.
  2. **Ce qui part** : les trois fichiers de la version (`capri`, `capri-hook`, `Info.plist` de
     la cible `macos-arm64`, exactement ceux-là), chacun par un `GET` à l'adresse que calcule
     `asset_url` (jamais lue dans le manifeste), par la même règle que le manifeste (mêmes hôtes,
     redirections une à une, ni proxy, ni `User-Agent`, ni `Accept`, ni clé possible :
     `capri_net::update::download`). Rien d'autre.
  3. **Ce qui est vérifié avant d'installer** : chaque fichier à la taille exacte et à l'empreinte
     SHA-256 du manifeste (calculée en route ; sinon il est retiré) ; l'`Info.plist` reçu, notre
     identifiant de bundle, la version du manifeste, notre programme ; la signature de l'app
     installée, avant de recopier ses ressources ; la signature de la nouvelle, à côté puis à sa
     place.
  4. **Où** : un seul dossier caché à côté de l'app installée (`.Capri.app.capri-update-<pid>`,
     `0700`, qui ne finit pas par `.app`, sa marque `capri-managed` verrouillée tant qu'on s'en
     sert), et, le temps du remplacement, l'installation précédente mise de côté à côté
     (`…-previous`). La nouvelle app y est refaite avec `Contents/Resources` de l'installée recopié
     tel quel (les images ne sont jamais téléchargées : elles restent celles de l'utilisateur, et
     l'app ne se distribue toujours pas), jamais son ancien `_CodeSignature`. Au moindre refus,
     case décochée ou Capri quittée, ce dossier est retiré. **Au lancement**, un reste n'est
     retiré que s'il est à nous sans doute possible (vrai dossier, notre nom au chiffre près, notre
     marque au texte exact, son verrou libre) ; sinon il est laissé, avec une ligne au journal.
     L'installation mise de côté n'est retirée que si l'app en place se vérifie (elle est
     peut-être la seule bonne) ; sinon elle reste, et son dossier caché avec elle. Du dossier
     listé pour les trouver, seuls nos noms sont gardés, jamais ceux des autres apps.
  5. **La signature** : celle de l'app installée, refaite sur la machine (`codesign` par son
     chemin absolu, toujours `--timestamp=none`, le second programme puis l'app, puis
     `--verify --strict`) ; jamais de notarisation ni d'outil qui téléverse (l'essai
     `signing_never_leaves_the_machine` de `capri_core::codesign` le vérifie sur les arguments).
     « Ad hoc » si l'installée l'est ; sinon l'identité valide du trousseau au même nom (seuls les
     noms et les empreintes des identités de signature sont lus, et seulement dans ce cas) : une,
     on signe avec elle ; aucune, « ad hoc », et elle dit que macOS redemandera le micro ;
     plusieurs, refus. **Le nom et l'empreinte d'une identité ne s'écrivent jamais**, ni dans le
     journal, ni dans un message, ni dans le dépôt. Les essais ne signent qu'en « ad hoc » et ne
     lisent jamais le trousseau.
  6. **La mise en place, au redémarrage** : l'ancienne lance, détaché, `capri update finish
     <app>` de la nouvelle app (`capri_platform::launch::hand_over`), puis se quitte par le même
     chemin que « Quitter » (la boucle s'arrête, `App::exiting`). Le programme de fin attend sa
     sortie **sans jamais signaler aucun processus, pas même par le signal 0** : son entrée est un
     tube dont elle garde l'autre bout ; « go » écrit, puis le tube fermé à sa sortie ; 30 s au
     plus, sinon il renonce et ne touche à rien. Il prend ensuite le verrou du dossier caché, que
     l'ancienne tenait jusqu'à sa sortie, en réessayant 2 s au plus (`LOCK_WAIT` : le système peut
     fermer le tube un instant avant de lâcher le verrou). Puis il fait comme `tools/install-macos.sh` :
     jamais une demi-app (des renommages), jamais par-dessus autre chose qu'une Capri (identifiant
     de bundle), jamais à travers un lien, l'ancienne remise en place au moindre échec, jamais
     `sudo`. Si elle ne peut pas l'être (sa place est prise : la nouvelle, dont la signature ne
     se vérifie plus, n'a pas pu en être retirée ; ou le renommage échoue), rien n'est mis
     par-dessus : l'ancienne reste de côté, rien n'est relancé, et le journal le dit (`Left` dit
     ce qu'il y a à la place après un échec ; seule `Left::Installed` se relance). Ses renommages
     et ses vérifications passent par un trait `Ground`, que les essais remplacent.
     Il relance l'app par `/usr/bin/open`, sauf avec `CAPRI_CONFIG_DIR` (un essai : `open`
     ne lui passerait pas ses lieux d'essai, et pourrait retrouver la Capri de l'utilisateur, du
     même identifiant). **Ce relais est un contrat entre deux versions** (l'ancienne lance le
     programme de la nouvelle : `capri update finish <app>`, la ligne `HANDOVER_READY`, puis
     `HANDOVER_GO` et la fermeture du tube, dans `capri_core::bundle`) : il ne change pas, ou par
     des ajouts que l'autre côté ignore. Une nouvelle qui ne prend pas le relais en 5 s laisse
     l'ancienne en place, qui le dit.
  7. **Ce qui se voit** : chaque étape se dit au-dessus de sa tête, à son tour
     (`BannerView::announce`), jamais une commande de terminal sauf le cas du point 1 ; la ligne
     du menu dit où en est l'installation. Le travail se fait hors du thread principal.
  8. **Le journal** ne dit que les étapes et la conclusion : jamais une adresse signée, jamais une
     identité, jamais la sortie de `codesign` ni de `security`.
  Limite connue : pendant le remplacement (quelques millisecondes), un hook de Claude Code qui
  appellerait `capri-hook` ne le trouverait pas.
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
- La musique : elle sait que de la musique joue par **les annonces des lecteurs connus, et
  seulement par elles** : Spotify (`com.spotify.client.PlaybackStateChanged`) et Musique
  (`com.apple.Music.playerInfo`), deux notifications distribuées. Tout autre son (navigateur,
  vidéo, jeu) ne la fait pas se balancer. (L'écoute de CoreAudio a été retirée : Spotify garde sa
  sortie audio ouverte en pause, mesuré 45 minutes d'affilée ; et macOS n'offre aucun moyen public
  et sans permission de savoir si un son est réellement audible.) La règle d'avant (« un oui ou un
  non, rien d'autre ») est **assouplie sur deux points, et seulement ceux-là**, par décision de
  l'utilisateur :
  1. l'annonce reçue **transporte** aussi le titre, l'artiste, l'album et d'autres champs, dans
     le même dictionnaire. Capri n'en lit **que la clé `Player State`** (`capri_core::music::STATE_KEY`),
     par `objectForKey:` : jamais d'itération du dictionnaire, jamais de `Debug`, de
     `description` ni d'impression de la notification ou de son `userInfo`. Le reste n'est ni lu,
     ni gardé, ni écrit dans le journal, ni affiché, ni envoyé à son cerveau. Toute valeur autre
     que `Playing`, `Paused`, `Stopped` (ou absente, ou pas une chaîne) vaut « ne joue pas » ;
  2. son code **nomme ces deux apps**, en constantes (`capri_platform::music::SPOTIFY`, `MUSIC` :
     le nom de l'annonce et l'identifiant de bundle, vérifiés dans leur binaire et leur
     `Info.plist`, pas de mémoire). Un lecteur cru en lecture est vérifié toutes les
     `ALIVE_EVERY` par `runningApplicationsWithBundleIdentifier:` avec **notre** constante, et
     seul le nombre d'apps rendues compte : jamais une liste des apps, jamais le nom d'une autre.
  Tout le reste tient : **aucune permission** (ni AppleScript, ni Automatisation, ni capture du
  son), **aucune API privée** (pas de MediaRemote), rien ne part sur le réseau, rien n'est envoyé
  à son cerveau ; Capri **ne pilote jamais** un lecteur (ni lecture, ni pause, ni touche
  multimédia) et ne poste jamais l'annonce d'un vrai lecteur, même en essai. Le journal ne dit des
  annonces que ce qu'elle en conclut (« se balance », « ne se balance plus ») et qu'elle s'abonne
  ou se désabonne : jamais le lecteur, ni rien de l'annonce. Ses répétitions non plus (`capri
  music rehearse` et `CAPRI_MUSIC_REHEARSAL`) : des vrais lecteurs, elles ne disent que des
  nombres pour eux tous ensemble (annonces reçues, combien tournent), jamais lequel.
  L'abonnement existe tant que la case de son menu est cochée (`music.json`), dès le lancement,
  assise ou non ; décochée, désabonnée et l'état oublié. Elle ne se balance qu'assise (avec ses
  planches) et visible ; debout, la musique ne change rien. Limite connue : une musique déjà en cours quand elle s'abonne
  ne compte qu'à la prochaine annonce du lecteur. Le lecteur d'essai, ses noms et le poste
  d'annonces n'existent qu'en version de débogage ; un nom d'annonce d'essai est propre au
  processus qui guette. À revérifier après tout changement du guetteur :
  `CAPRI_ASSETS=… cargo test -p capri-app --test musique_sans_titre -- --ignored --test-threads=1`
  (la répétition et la vraie app : reçue, état lu, jamais un faux titre, artiste ni album, ni le
  nom d'un vrai lecteur, dans la sortie ou le journal au niveau `trace`, fichier du journal
  compris ; la vraie app s'en tient au dossier d'essai de son ouverture de session, ne veille pas
  sur Ollama et n'écrit que son journal ; relancé et passé le 15 novembre 2026).
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
- **Se lancer à l'ouverture de session, et Ollama.** Capri ne s'inscrit **que sur un geste de
  l'utilisateur** (la case de son menu), jamais d'elle-même ; elle ne fait ensuite que réparer
  **son propre** agent quand l'app a changé de place. Elle n'inscrit que `Capri.app` (par `open`,
  pour garder les permissions de l'app), jamais un binaire nu. Elle **ne lance qu'Ollama**, et
  seulement **case cochée, cerveau Ollama, sur cette machine même** (`Reach::ThisMachine`, en
  `http`) et rien n'y répondant : jamais LM Studio, jamais vers une autre machine, jamais un autre
  programme, et toujours pas Claude Code. Ollama est cherché à des emplacements fixes
  (`startup::candidates`), jamais par le `PATH`, et écoute sur l'adresse réglée
  (`OLLAMA_HOST`, `localhost` devenu `127.0.0.1`). Elle ne l'arrête **jamais**, même en la
  quittant. Elle ne retire **que son propre fichier** (signé `capri-managed`), et décocher ne
  retire que lui. Savoir si quelque chose répond passe par `capri_net::knock` (cette machine
  seulement, rien d'envoyé), hors du thread principal : rien ne retarde son apparition. Les essais
  n'écrivent jamais dans le vrai `~/Library/LaunchAgents` et ne lancent jamais un vrai Ollama.
  `CAPRI_CONFIG_DIR` posée, la vraie app n'y lit ni n'y écrit rien non plus : son dossier est
  `<CAPRI_CONFIG_DIR>/LaunchAgents` (`capri_claude::places::login_agents`), que `launchd` ne lit
  jamais, et la case dit ce qu'il contient (vide : décochée, et Ollama n'est pas veillé) ; les
  règles d'écriture ne changent pas. Sans elle, rien ne change. Ce
  qu'elle en dit ne renvoie à aucune commande de terminal. Le format de l'agent se vérifie dans
  `man 5 launchd.plist`, celui d'Ollama dans sa documentation (<https://docs.ollama.com/faq>,
  <https://docs.ollama.com/macos>) et `ollama serve --help`, pas de mémoire.
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
  `X-OpenRouter-Title`…). Le `User-Agent` que ureq envoie de lui-même (`ureq/<version>`) **reste**
  dans les requêtes du chat, par décision de l'utilisateur le 23 novembre 2026 : il ne nomme ni
  Capri ni l'utilisateur, la requête porte déjà sa clé, et des services refusent une requête qui
  n'en a pas (le « Browser Integrity Check » de Cloudflare, d'après sa documentation ; pas
  vérifié fournisseur par fournisseur). Seule la lecture des versions le retire, parce que rien
  d'autre n'y identifie quoi que ce soit.
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
  `trace` de ureq (`NoWireLog` dans `journal.rs`), qui écrivent les octets échangés sans masque :
  ni sur la sortie d'erreur, ni dans le fichier du journal.
- **Le journal en fichier.** Seulement l'app à fenêtre (`journal::start`), jamais ses commandes de
  terminal, jamais `capri-hook`. Une seule exception : le programme de fin d'une mise à jour
  (`capri update finish`, `journal::start_finisher`), qui n'est pas une commande de terminal (l'app
  le lance, sur un geste), et n'ouvre le fichier qu'une fois l'ancienne Capri sortie ; refusé ou
  renonçant avant, il n'écrit rien (l'essai `journal_des_commandes` le lance hors d'une mise à
  jour). Où : `~/Library/Logs/Capri/capri.log` sur macOS,
  `<dossier de Capri>/logs/capri.log` ailleurs, et `<CAPRI_CONFIG_DIR>/logs/capri.log` dès que
  `CAPRI_CONFIG_DIR` est posée, sur tous les systèmes (`capri_claude::places::log_file`) : les
  essais et les répétitions posent `CAPRI_CONFIG_DIR` et n'écrivent **jamais** dans le vrai
  `~/Library/Logs`. **Borné** : deux fichiers au plus (`capri.log`, `capri.old.log`), chacun d'au
  plus `capri_core::journal::MAX_BYTES` (1 Mio), la rotation au lancement et avant la ligne qui
  ferait dépasser la borne, même si une ligne se répète en boucle. **Réservé à l'utilisateur** :
  dossier créé en `0700`, fichier en `0600` (resserré s'il existait) sous Unix ; jamais écrit à
  travers un lien ni par-dessus autre chose qu'un fichier ordinaire (alors, pas de fichier, et
  elle continue). **Jamais une panne d'elle** : quoi qu'il arrive au fichier, il cesse d'écrire,
  le dit une fois sur la sortie d'erreur, ne panique jamais (le verrou d'`env_logger` serait
  empoisonné : jamais d'`unwrap`, jamais de `log::…` depuis son écriture), et la sortie d'erreur
  continue. Chaque ligne est écrite tout de suite, sans tampon. Le même filtre (`RUST_LOG`) et le
  même format (celui d'`env_logger`, en temps universel) que la sortie d'erreur, qui ne change pas.
  Elle ne le relit jamais, et **rien n'en part nulle part** : ni réseau, ni cerveau, ni
  téléversement. **Les interdits du journal valent d'autant plus qu'il reste sur le disque** :
  jamais ce qui se dit (au micro, dans la conversation, la ligne du miroir : seulement sa
  longueur), jamais une clé, jamais le titre d'une fenêtre ni le nom d'une app, rien d'une
  annonce de musique, jamais les traces de ureq. Tout nouvel appel au journal se relit contre
  cette liste, **à tout niveau** : `info`, `warn` et `error` s'écrivent par défaut, et
  `RUST_LOG=debug` (ou `trace`) fait entrer les autres dans le fichier aussi (tous relus le 15
  novembre 2026). Des événements de Claude Code, le journal n'écrit que leur forme pour lui
  (`capri_claude::protocol::Message::for_log`, essayée avec un événement de chaque sorte plein de
  faux secrets) : la sorte d'événement, le nom de l'outil et le type de sous-agent s'ils ont la
  forme d'un nom, les catégories de Claude Code (`permission_prompt`, `rate_limit`) si elles en
  ont la forme, les durées, des longueurs, et une étiquette (`#` et huit chiffres hexadécimaux)
  pour chaque identifiant ; jamais le résumé d'un outil, le message d'une notification, l'erreur
  d'un outil, la ligne du miroir, le dossier de travail, le modèle ni un identifiant lui-même.
  Un nouvel événement, ou un nouveau champ d'événement, oblige à décider ce qui en est écrit (ni
  `_` ni `..` dans son `match`). Limite connue : un nom d'outil ou de sous-agent est écrit tel
  quel s'il a la forme d'un nom (Claude Code les tire de sa configuration : un nom d'outil MCP
  nomme son serveur).
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
  Pendant qu'elle apparaît, la ligne de Claude et un message de l'app attendent qu'elle soit là
  (`Look::is_here`) ; pendant qu'elle s'efface, son bandeau s'en va avec elle, comme cachée.
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
  winit (Quitter, Masquer). Capri ne se quitte que par un geste dans son propre menu, « Quitter »,
  ou « Redémarrer pour passer à la … » une fois une mise à jour prête (et seulement si la nouvelle
  version a pris le relais), et ne se masque que par son propre raccourci. Tout nouveau panneau
  qui prend le clavier porte cette vue.
- Le `settings.json` de Claude Code appartient à l'utilisateur et à d'autres outils : toute
  modification passe par `capri_claude::settings` (réécriture à l'octet près, vérification) et
  par `hook_setup` (sauvegarde, écriture atomique). Jamais d'édition directe.
- CapriChooms/CapriCore (GPL-3.0) : on s'inspire de ses idées, on ne copie pas son code.
