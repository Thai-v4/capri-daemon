# Capri

Capri, la construct de *Cyberpunk: Edgerunners* saison 2, vit sur ton bureau. Écrite en Rust,
100 % native, pour **macOS** et **Windows**.

> Projet de fan non commercial. Voir [CREDITS.md](CREDITS.md) pour le personnage et les images.

## Jalon 1 : ce qu'elle fait

- Elle flotte en bas à droite de l'écran, animée façon anime : la boucle dessinée par l'artiste du
  projet [Capri](https://github.com/lachydotmcg/capri-desktop) (voir
  [plus bas](#ses-dessins--debout-assise-et-sur-ta-musique)), tenue « en deux » (12 dessins par
  seconde), et un flottement en poses clés. Sans ces planches, son dessin d'avant : un trait qui
  « bout » à chaque dessin, une copie violette mal repérée.
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

**Lâche-la doucement**, debout, les pieds près du haut d'une fenêtre (à moins de 45 points au-dessus
ou en dessous de son bord) : elle apparaît sur le bord et **s'y assoit** : le bord passe sous ses
cuisses, ses jambes pendent devant la fenêtre, sa main s'y tient (voir
[ses dessins](#ses-dessins--debout-assise-et-sur-ta-musique)). Sans ses planches assises, elle s'y
tient debout, comme avant : elle s'écrase un instant sur ses pieds, rebondit, puis reste immobile.
**Lance-la** : là où elle s'arrête, c'est la même règle. Posée, elle **suit la fenêtre** quand tu la déplaces ou la
redimensionnes, à la même place le long du bord (tirée par un seul côté, la fenêtre ne l'emmène pas :
le bord sous ses pieds n'a pas bougé), sans quitter le bord ni l'écran, à chaque rafraîchissement de
l'écran (120 fois par seconde sur un écran ProMotion, 60 sur la plupart des autres ; de même quand
on la tient ou qu'elle vole). Ses dessins, eux, restent des poses tenues à 24 images par seconde :
seule sa position suit l'écran. Elle **s'efface du bord**, là où elle était (en une seconde), puis
**se remet à flotter**, debout, avec un petit étirement, quand :

- sa fenêtre n'est plus à l'écran (fermée, réduite dans le Dock, app masquée, autre bureau) ;
- une autre fenêtre passe devant le morceau de bord où elle est assise ;
- le bord sort de l'écran, ou ne lui laisse plus la place (plein écran) ;
- tu décoches « S'asseoir sur les fenêtres ».

**Attrape-la** : elle se lève tout de suite, sans s'effacer, tenue par le même endroit d'elle (ses
cheveux, ses pieds…). Debout, elle reprend sa place à l'écran : si le bord est trop près du haut
de l'écran pour qu'elle y tienne debout, elle réapparaît un peu plus bas, devant la fenêtre.

Ne comptent que les fenêtres d'app ordinaires (ni menus, ni Dock, ni panneaux flottants), à
l'écran, opaques, d'au moins 90 × 45 points, sur son écran (celui qui porte la barre des menus), et
pas les siennes ; et seulement les parties visibles de leurs bords : une fenêtre devant cache le
bord de celle de derrière.

**Ce qu'elle lit, et ce qu'elle ne lit jamais.** Aucune permission : ni Accessibilité, ni
Enregistrement de l'écran. Sur macOS, elle lit la liste des fenêtres de CoreGraphics
(`CGWindowListCopyWindowInfo`), et de chaque fenêtre seulement son numéro, son rectangle, sa couche,
son opacité, et si elle est à Capri. **Jamais le titre d'une fenêtre ni le nom d'une app** : ni lus,
ni gardés, ni écrits dans le journal, qui ne dit que des nombres. Elle ne garde que des numéros de
fenêtre, des rectangles et leur ordre, le temps de décider.

**Ce que ça coûte.** La liste n'est lue que quand elle sert : une fois quand elle s'arrête ; posée,
4 fois par seconde quand rien ne bouge ; à chaque rafraîchissement de l'écran pendant que sa fenêtre
bouge ou qu'un bouton de la souris est enfoncé, et encore une demi-seconde après. Jamais quand elle
est cachée, tenue ou en vol, ni quand c'est désactivé. Au repos, la boucle de l'app se réveille
60 fois par seconde, comme avant ; pressée, c'est l'écran qui la réveille (sur macOS 14 et suivants,
un `CADisplayLink`, en pause le reste du temps). Elle ne se redessine que quand sa pose ou sa
position change. Mesuré le 11 novembre 2026 sur un MacBook Pro M5 Pro (écran à 120 Hz), processeur
de tout le processus, version optimisée, par la répétition `CAPRI_PERCH_REHEARSAL` (plus bas) :

| moment | processeur (% d'un cœur) | lectures par seconde | images par seconde |
| ------ | ------------------------ | -------------------- | ------------------ |
| elle flotte | 2,3 à 2,7 | 0 | 12 |
| posée, rien ne bouge | 2,1 à 2,7 | 4 à 5 | 12 |
| posée, un bouton de la souris enfoncé, la fenêtre immobile | 5,2 à 5,3 | 120 | 12 |
| posée, sa fenêtre déplacée | 12,4 à 14,3 | 115 à 119 | 100 à 108 |
| *avant ce réglage*, sa fenêtre déplacée | 4,7 (une mesure) | 24 | 24 |

Ces mesures datent d'avant ses planches (son dessin d'avant). Avec elles, flottante au repos :
2,3 % d'un cœur (0,35 s de processeur en 15 s, version optimisée, mesuré le 11 novembre 2026 ;
2,3 % aussi avec son dessin d'avant, mesuré de même). Posée, et pendant qu'on déplace sa fenêtre,
pas remesuré avec ses planches : elle dessine toujours 12 dessins par seconde au repos, une seule
case (deux quand elle parle debout), sans trait qui bout ni copie violette.

Posée au repos, elle ne coûte donc rien de plus que flottante. Pendant un déplacement, chaque image
coûte : c'est le prix de 100 positions par seconde au lieu de 24, et seulement le temps du
déplacement (une demi-seconde après son arrêt, elle revient au repos). Une lecture de la liste prend
0,16 ms au repos ; pendant qu'une fenêtre bouge, elle prend 3,5 ms (médiane), mais seulement
0,15 ms de processeur : le reste est l'attente de la réponse du serveur de fenêtres.

**Son retard sur la fenêtre.** Elle n'est pas collée à sa fenêtre : elle la suit avec environ une
image de retard. CoreGraphics donne la position d'une fenêtre après coup : déplacée par programme,
la liste la montre 6,4 à 6,9 ms plus tard (médiane ; une fois sur vingt seulement tout de suite).
La position qu'elle dessine a environ 7 ms quand l'image part (8 ms pour 90 % des images), puis
l'écran la montre au rafraîchissement suivant (jusqu'à 8,3 ms de plus, que rien ne mesure ici).
Pour une fenêtre qui file jusqu'à 785 points par seconde, l'écart entre elle et la fenêtre est de
4 points en médiane, de moins de 7 points neuf fois sur dix (avant ce réglage : 12 points en
médiane, jusqu'à 29 neuf fois sur dix, et elle avançait par sauts). Lire la position plus tard
n'y changerait rien : elle est déjà lue juste avant de dessiner (0,3 ms entre la lecture et l'image
envoyée) ; le retard est dans CoreGraphics et dans l'écran. Ces chiffres viennent d'une fenêtre
déplacée par programme : une fenêtre tirée à la main n'a pas été mesurée.

**Le désactiver** : décoche « S'asseoir sur les fenêtres » dans son menu (clic droit sur elle, ou son
icône dans la barre des menus). C'est retenu d'un lancement à l'autre (`perch.json`, dans son
dossier) ; activé par défaut.

**Sa taille.** Plus petite, elle se pose sur bien plus de fenêtres (il lui faut, assise, la place
de sa tête au-dessus d'un bord). Dans son menu (clic droit sur elle, ou son icône ; macOS et Windows),
« Taille : 100 % » ouvre « Plus grande », « Plus petite » et « Taille d'origine » : par crans de
10 %, de **40 %** (120 points de haut) à **140 %** (420 points). Plus finement, d'un geste sur
elle : la molette, ou deux doigts sur le pavé tactile, **avec ⌥ tenue** sur macOS (**Ctrl** sous
Windows), par crans de 2 %. Sans la touche, faire défiler sur elle ne fait rien, comme avant, et la
page sous elle ne peut pas la changer de taille : la fenêtre ne reçoit la molette que quand le
curseur est sur elle, et l'élan qui suit un geste sur le pavé tactile ne compte pas. Pas pendant
qu'on la tient.

- elle change tout de suite : ses pieds restent au même endroit, elle reste entière à l'écran, son
  bandeau et sa bulle se replacent ; posée, elle reste sur son bord si la place au-dessus lui
  suffit, sinon elle repart flotter ;
- c'est retenu d'un lancement à l'autre, dans `size.json` de son dossier (`{ "scale": 0.8 }`, rien
  d'autre), au plus tard une demi-seconde après le dernier cran de molette ; un fichier illisible
  donne la taille d'origine, avec une ligne dans le journal, et n'est pas réécrit tant qu'on ne
  change pas sa taille ;
- **agrandie, elle est floue** : sa silhouette debout fait 347 pixels de haut dans ses planches.
  Sur un écran Retina, elle est déjà agrandie 1,73 fois à la taille d'origine (1,44 fois assise),
  2,42 fois à 140 % (2,02 fois assise) : ses bords sont un peu doux à 100 %, nettement à 140 %. La
  borne haute (140 %) avait été choisie pour son dessin d'avant (220 × 420 pixels, agrandi 2 fois
  à 140 %) ; elle n'a pas changé. Réduite, elle reste nette ; très réduite, son trait fin peut
  scintiller (les planches n'ont qu'une seule résolution, sans mipmaps) ;
- la taille est une échelle rapportée à sa hauteur debout (`capri_core::scale`), pas des
  dimensions : 100 % = 300 points pour sa silhouette debout, des cheveux à la pointe du pied (pas
  pour la case de la planche, qui a de la marge), et sa pose assise est à la même échelle.

Limites connues :

- assise, il lui faut au-dessus du bord la place de sa tête (171 points à la taille d'origine, 68 à
  la plus petite, 239 à la plus grande), en dessous celle de ses jambes (82 points à la taille
  d'origine), et un morceau de bord visible d'environ 48 points de part et d'autre de ses pieds
  pour sa main et son genou. Une
  fenêtre agrandie, ou dont le haut est près de la barre des menus, ne peut pas la porter. Sans ses
  planches assises, debout, il lui faut toute sa hauteur (300 points à la taille d'origine) ;
- elle vit sur l'écran qui porte la barre des menus : les fenêtres d'un autre écran ne comptent pas,
  et elle repart quand sa fenêtre quitte son écran ;
- une fenêtre fermée, réduite ou passée devant elle au clavier se remarque au plus tard un quart de
  seconde après ; une fenêtre déplacée à la souris, elle la suit à chaque rafraîchissement de
  l'écran, avec environ une image de retard (voir plus haut) ; une fenêtre déplacée sans la souris
  (un raccourci, un gestionnaire de fenêtres), elle ne la voit partir qu'à sa lecture suivante, un
  quart de seconde au plus, puis la suit de même ;
- sur macOS avant la version 14, pas de `CADisplayLink` de vue : un minuteur réglé sur la fréquence
  de l'écran le remplace (non essayé) ;
- une fenêtre que le système dit opaque mais qui est transparente par endroits compte comme pleine ;
- Windows : écrit et compilé, jamais exécuté (voir plus bas). La fréquence de l'écran y vient de
  winit (`EnumDisplaySettingsExW`), les pas pressés d'un minuteur à cette fréquence (winit prend un
  minuteur à haute résolution), et la présentation (`Fifo`) se cale sur l'écran ; Linux : elle
  flotte comme avant.

### Ses dessins : debout, assise, et sur ta musique

Debout comme assise, elle a les dessins de l'artiste du projet
[Capri](https://github.com/lachydotmcg/capri-desktop) (voir `CREDITS.md`, usage non commercial
seulement), que `tools/fetch-assets.sh` télécharge dans `assets/capri/stand/` et
`assets/capri/perch/`. Tous sont tenus « en deux » : 12 dessins par seconde. Rien n'est tiré au
hasard : à la même image, le même dessin.

- **Debout** : sa boucle (2,5 s), et le flottement que le programme lui ajoute (la boucle de
  l'artiste ne flotte pas), dont le tour dure aussi 2,5 s : elle est au plus bas quand ses cheveux
  retombent. Elle cligne des yeux quatre fois toutes les 20 s, toujours aux mêmes tours (espacées de
  5 s, 2,5 s, 5 s puis 7,5 s). **Tenue ou lancée** : la même boucle, écrasée à l'impact et étirée
  dans le sens de sa course, comme avant.
- **Elle parle** : sa bouche, une des cinq formes de l'artiste, change 12 fois par seconde,
  posée sur son visage. « Parler », c'est : si sa réponse se dit à voix haute, tant que sa voix
  parle (pas avant le premier son, ni après le dernier, même si sa bulle reste) ; sinon, tant que
  sa bulle montre ses mots (pas « … » quand elle réfléchit). La ligne que Claude écrit pour elle
  (le miroir) compte de même. Assise, c'est la boucle parlante de l'artiste, la même que sa boucle
  assise, dessin pour dessin.
- **Assise** : elle apparaît sur le bord à partir d'un trait (1,2 s), puis sa boucle, où elle cligne
  aux mêmes tours que debout. Le bord passe sur sa ligne d'assise (sous ses cuisses) : ses jambes
  pendent devant la fenêtre, sa main tient le bord. Elle s'efface jusqu'à un trait (1 s) quand elle
  repart d'elle-même ; attrapée, elle est debout tout de suite. Les deux poses sont à la même
  échelle : un pixel de ses planches debout vaut 1,20 pixel de ses planches assises
  (`STAND_OVER_PERCH`, choisi le 12 novembre 2026 ; le rapport du projet Capri est 1,11, voir plus
  bas ce qu'en montrent les images).
- **Sur ta musique**, assise : quand Spotify ou Musique annonce une lecture, elle se balance, les
  yeux fermés, une main dans les cheveux ; elle s'arrête dès qu'il annonce une pause ou un arrêt.
  Elle entre dans la musique et en sort au début d'un tour de sa boucle (jusqu'à 2,5 s plus tard), là où les deux
  dessins se ressemblent le plus. Parler passe avant : elle se met à parler tout de suite, et ne
  reprend la musique qu'au tour suivant. **Debout, la musique ne change rien** : il n'y a pas de
  dessin pour cela.
- **Sans ses planches** (manquantes, illisibles, ou refusées par la carte graphique) : son dessin
  d'avant (`capri-cutout.png`), comme avant, avec son trait qui bout et sa copie violette ; sans
  planches assises, elle se tient debout sur les fenêtres ; et sans planches debout mais avec les
  assises, son dessin d'avant debout, assise avec ses planches. Le journal le dit en une ligne.

**Sur ta musique : les annonces de Spotify et de Musique, et seulement elles.** Ces deux lecteurs
annoncent leur état à tout le système (une notification distribuée, à chaque lecture, pause, arrêt
ou changement de morceau). Sur macOS, Capri s'y abonne (`capri_platform::music`, décidé par
`capri_core::music`) et n'en lit **qu'une clé**, l'état du lecteur (`Player State` : `Playing`,
`Paused` ou `Stopped` ; toute autre valeur vaut « ne joue pas »). L'annonce transporte aussi le
titre, l'artiste et l'album : ils ne sont jamais lus, gardés, écrits dans le journal, affichés ni
envoyés à son cerveau. Aucune permission, aucune API privée, rien sur le réseau, et elle ne pilote
jamais le lecteur. Le journal n'en dit que « se balance » ou « ne se balance plus », jamais le
lecteur.

Pourquoi pas « un son sort-il ? », comme avant : Spotify garde sa sortie audio ouverte même en
pause (mesuré : en marche 45 minutes d'affilée, musique coupée), elle se balançait donc sur une
musique arrêtée ; et macOS n'offre aucun moyen public et sans permission de savoir si un son est
réellement audible.

**Ses limites.** Seuls **Spotify et Musique** comptent : un navigateur, une vidéo, un jeu, un
autre lecteur ne la font pas se balancer. **Une musique déjà en cours** quand Capri démarre (ou
quand on recoche la case) ne compte qu'à la prochaine annonce du lecteur : pause puis lecture, ou
morceau suivant. Un lecteur qu'on quitte ou qui plante sans annoncer qu'il s'arrête : tant qu'un
lecteur est cru en lecture, elle demande au système toutes les 5 s s'il tourne encore (par son
identifiant, jamais une liste des apps ; 0,2 à 1 ms, mesuré), et l'oublie sinon.

L'abonnement existe **tant que c'est permis**, dès le lancement, qu'elle soit assise ou non : une
annonce ne coûte rien, et elle sait déjà ce qui joue quand elle s'assoit. Elle ne se balance
qu'assise (avec ses planches) et visible. Aucun thread à elle : les annonces arrivent par la boucle
du thread principal, même si l'app n'est jamais active (elle s'y abonne avec la livraison
immédiate). **Le désactiver** : décoche « Se balancer sur la musique » dans son menu (clic droit
sur elle, ou son icône) : elle se désabonne et oublie. C'est retenu d'un lancement à l'autre
(`music.json`, dans son dossier, rien d'autre que `{ "enabled": false }`) ; permis par défaut.
Windows et Linux : elle ne sait pas si de la musique joue, et ne se balance pas ; la case est
grisée.

**Ce que ça pèse.** Les planches sont lues au lancement (44 ms en version optimisée), remises en
grilles (81 Mo), puis lâchées une fois confiées à la carte graphique. Seuls les dessins montrés y
vont (un sur deux pour les boucles et le clignement) : 47 Mo de textures. Sur un Mac à puce Apple,
elles comptent dans la mémoire de l'app (son « empreinte », celle du Moniteur d'activité) : mesuré
par `footprint`, 253 Mo avec ses planches contre 200 Mo avec son dessin d'avant seulement.

**Les deux poses à la même échelle : ce qu'on en sait.** Les deux dessins n'ont pas les mêmes
proportions, et leurs mesures ne s'accordent pas : du cou à l'entrejambe, il faudrait 1,08 ; pour
que sa tête (du haut des cheveux au menton : 107 pixels debout, environ 153 assise) ait la même
taille, environ 1,4, mais assise ses cheveux sont plus hérissés et sa tête penchée, son visage vu
de trois quarts ; ses jambes, assise, sont dessinées bien plus longues. 1,11 est le rapport du
projet Capri. À l'œil, à 1,11, elle paraît un peu plus grande assise que debout, la tête surtout ;
à 1,20, les têtes se rapprochent. **1,20 a été choisi le 12 novembre 2026**, après l'avoir vue en
mouvement à 1,11 et regardé les deux comparaisons rendues : assise, elle est dessinée un peu plus
petite. L'image `cote-a-cote-a-1-11.png`, rendue par les essais de `portraits.rs`, garde la
comparaison avec 1,11.

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

Sur macOS, Capri peut aussi lancer Ollama elle-même quand il ne tourne pas : coche « Se lancer à
l'ouverture de session » dans son menu ([plus bas](#se-lancer-à-louverture-de-session)).

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

## Se lancer à l'ouverture de session

Sur macOS, **« Se lancer à l'ouverture de session »** dans son menu (icône de la barre des menus,
ou clic droit sur elle), décochée par défaut. Cochée, deux choses :

- **Capri se lance toute seule** quand tu ouvres ta session, sans passer au premier plan ;
- **à chaque lancement** (à l'ouverture de session ou à la main), si son cerveau est **Ollama sur
  cette machine** (`http://localhost:11434/v1`, ou une autre adresse de la machine) et que rien n'y
  répond, **elle lance Ollama**, et le dit : « J'ai lancé Ollama, mon cerveau est prêt ». Si
  quelque chose y répond déjà, elle ne fait rien et ne dit rien. Cocher la case regarde aussi tout
  de suite.

Décochée, elle ne lance jamais rien. Elle dit dans son bandeau ce qu'elle vient de faire : « Je me
lancerai à l'ouverture de ta session, et Ollama avec moi » (ou sans Ollama, si ce n'est pas son
cerveau ici), « Je ne me lancerai plus à l'ouverture de ta session ».

**Comment elle s'inscrit.** Par un agent de lancement de ton utilisateur,
`~/Library/LaunchAgents/local.capri-daemon.capri.login.plist`, qui ne fait qu'une chose :
`/usr/bin/open -g -a <ton Capri.app>`, c'est-à-dire ouvrir l'app comme un double-clic (ses
permissions, le micro par exemple, restent celles de l'app). La case dit si ce fichier est là,
rien d'autre ne le retient. Il est écrit d'un seul coup et signé d'un commentaire `capri-managed` :
Capri n'écrase ni ne retire jamais un fichier qui ne porte pas cette signature, ni un lien, et ne
touche à aucun autre fichier du dossier. Décocher retire ce fichier, et rien d'autre (elle ne
demande rien à `launchctl` : la tâche déjà chargée dans ta session y reste jusqu'à ce que tu la
fermes, sans plus jamais rien lancer). Si tu déplaces `Capri.app`, l'inscription se répare au
lancement suivant, case cochée, depuis la nouvelle place ; une seconde copie de l'app ne la prend
pas tant que la première existe (décoche puis recoche depuis celle que tu veux).

Pourquoi un agent de lancement plutôt que `SMAppService`, l'API de macOS 13 pour les éléments
d'ouverture : d'après son en-tête (`SMAppService.h`), l'app doit être signée, et l'inscription doit
être refaite quand l'exécutable change. Capri est signée « ad hoc » et reconstruite souvent : un
fichier lié à un chemin survit aux reconstructions, et se vérifie sans écran.

**Seulement depuis `Capri.app`** (`tools/bundle-macos.sh`) : lancée par `cargo run` ou
`target/release/capri`, cocher la case refuse (« Pour m'inscrire, lance-moi depuis Capri.app ») :
le binaire seul n'aurait pas les permissions de l'app. Le lancement d'Ollama, lui, vaut aussi
pour le binaire, case cochée.

**Comment elle trouve et lance Ollama.** À des emplacements fixes, jamais par le `PATH` (celui
qu'une tâche de `launchd` reçoit, `/usr/bin:/bin:/usr/sbin:/sbin`, ne contient pas celui de
Homebrew) : `/Applications/Ollama.app` puis `~/Applications/Ollama.app` (seulement pour l'adresse
par défaut, `localhost` ou `127.0.0.1` au port 11434 : l'app ne prend pas sa configuration de qui
la lance), puis `/opt/homebrew/bin/ollama` (Homebrew), `/usr/local/bin/ollama` (Homebrew sur Intel,
ou le lien que pose l'app), et le programme contenu dans l'app. Le programme est lancé avec
`serve` et `OLLAMA_HOST` réglé sur l'hôte et le port de ton adresse (`localhost` devient
`127.0.0.1`, l'adresse par défaut d'Ollama d'après [sa documentation](https://docs.ollama.com/faq)) :
il n'écoute que sur cette machine. Pour savoir si quelque chose répond, elle ouvre une connexion à
l'adresse et la referme aussitôt, sans rien envoyer, une fois par seconde. Elle laisse **6 s** à un
Ollama qui serait déjà en train de démarrer (par `brew services`, à l'ouverture de session), puis
le lance **une seule fois**, et attend au plus **30 s** qu'il réponde. Tout cela sur un thread à
part : elle apparaît sans attendre.

**Elle ne l'arrête jamais** : un Ollama lancé par elle continue de tourner quand tu la quittes
(un autre outil peut s'en servir). Il est détaché d'elle : un Ctrl+C dans le terminal qui l'a
lancée ne l'atteint pas non plus.

Ce qu'elle dit quand ça ne va pas, sans jamais renvoyer à une commande : « Ollama ne répond pas, et
je ne le trouve pas sur ce Mac » (installe-le), « Je n'ai pas pu lancer Ollama, détails dans le
journal », « J'ai lancé Ollama, mais il ne répond pas encore », « Un fichier qui n'est pas à moi
porte déjà ce nom, je n'y touche pas ».

Ses limites :

- **elle ne lance qu'Ollama** : ni LM Studio (son serveur se démarre dans LM Studio), ni un
  Ollama d'une autre machine, ni une adresse en `https://`, ni rien d'autre. Elle ne lance jamais
  Claude Code ;
- **sous Windows et ailleurs, la case est grisée** : rien n'y est écrit. Il y faudrait une entrée
  dans `HKCU\…\Run` ou le dossier « Démarrage », et de quoi lancer `ollama.exe` : pas encore écrit ;
- si tu désactives Capri dans Réglages Système (Général → Ouverture, « Autoriser en
  arrière-plan »), le fichier reste et la case reste cochée : elle ne le sait pas ;
- avec `brew services start ollama` (qui le lance à chaque ouverture de session et le relance s'il
  s'arrête), Capri n'a rien à faire, et attend 6 s avant de rien lancer. Si Ollama mettait plus
  longtemps à démarrer, les deux lanceraient chacun le leur : un seul obtient le port, et celui de
  `brew services` réessaierait sans cesse tant que celui de Capri le tient. C'est déduit de leurs
  réglages, pas observé ;
- macOS peut afficher une notification « Éléments d'arrière-plan ajoutés » quand la case est
  cochée : pas encore vu.

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
                   départ, cadence de lecture), et quand la boucle se réveille (pace.rs : 60 Hz
                   au repos, la fréquence de l'écran quand elle suit quelque chose), et sa taille
                   (scale.rs : une échelle rapportée à sa hauteur debout, ses bornes, ses crans),
                   ses planches (sheets.rs, sheets/stand.rs : cases, dessins tenus en deux, ligne
                   d'assise, grilles serrées des seuls dessins montrés), et son dessin (look.rs :
                   quelle case montrer, où, ce qui l'attrape, ce qu'il lui faut sur un bord),
                   et si de la musique joue (music.rs : l'état annoncé par les lecteurs), et le
                   texte de son inscription à l'ouverture de session (login.rs : l'agent de
                   lancement, la reconnaissance du nôtre, sa réparation) (testée)
  capri-claude/    logique pure : hooks de Claude Code, protocole, sessions, settings.json, style
                   de sortie « Capri », miroir, dossiers de Capri et de Claude Code (testée)
  capri-chat/      logique pure : conversation, fournisseurs et format « chat completions », flux
                   de réponse, règles d'adresse et de clé, réglages du cerveau, personnalité,
                   phrases à dire, dictée, les effets de voix (voice.rs : préréglages,
                   intensité, hauteur, débit, voice.json), lancer Ollama au lancement (startup.rs :
                   faut-il, où le chercher, le délai, ce qu'elle en dit), et le modèle de la fenêtre de
                   réglages (settings/ : rubriques « Cerveau », « Voix » et « Claude Code »)
                   (testée)
  capri-net/       le seul crate qui parle au réseau : envoyer la conversation (en https par le TLS
                   du système), lire la réponse au fil de l'eau (testé contre un faux serveur local),
                   et frapper à une adresse de cette machine seulement (knock : Ollama tourne-t-il ?)
  capri-render/    rendu wgpu : Metal sur macOS, DirectX 12 + DirectComposition sur Windows ;
                   ses images confiées une fois, une case de grille par dessin ; Offscreen, le
                   même rendu dans une image en mémoire, pour les essais
    sprite.wgsl    le shader : une part de l'image, boil et copie violette
    banner.rs      le texte du bandeau et de la bulle, tracé avec les polices du système
  capri-platform/  la fenêtre « Overlay » : première des deux couches qui parlent au système
    macos.rs         AppKit via objc2 : un NSPanel non activant, sans fenêtre winit, et le
                     CADisplayLink qui réveille la boucle au rythme de l'écran quand il le faut
    macos/input.rs   le champ de saisie : un des deux panneaux qui prennent le clavier, sur demande
    settings_window.rs la fenêtre de réglages, qui affiche un formulaire de capri-core ;
                     macos/settings_window.rs : un NSPanel à onglets (refus ailleurs)
    macos/edit_keys.rs ⌘X, ⌘C, ⌘V, ⌘A, ⌘Z dans les champs d'une app sans menu Édition
    voice.rs         sa voix ; macos/voice.rs : la synthèse vocale du système, chemin direct et
                     chemin traité dans une seule file, et le repli ;
                     macos/voice_stage.rs : la chaîne d'effets (AVAudioEngine)
    ear.rs           son oreille ; macos/ear.rs : le micro et la reconnaissance sur l'appareil
    desk.rs          la liste des fenêtres des autres apps, sans permission ; macos/desk.rs
                     (CoreGraphics), windows/desk.rs (Win32 et DWM, jamais exécuté) ; en version
                     de débogage, macos/test_window.rs (la fenêtre d'essai qu'on déplace)
    music.rs         les annonces de Spotify et de Musique, dont seul l'état est lu ;
                     macos/music.rs (NSDistributedNotificationCenter ; refus ailleurs)
    keychain.rs      le trousseau des clés d'API ; macos/keychain.rs, windows/keychain.rs
    login.rs         son agent de lancement : écrit, lu, retiré, dans un dossier passé en
                     paramètre (refus ailleurs que sur macOS)
    launch.rs        lancer Ollama : un programme détaché, par son chemin absolu, ou une app par
                     open (refus ailleurs que sur macOS)
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
                   afficher, lancée par tests/fenetres_du_bureau.rs) et `CAPRI_PERCH_REHEARSAL`
                   (perch_rehearsal/follow.rs : le suivi d'une fenêtre d'essai, mesuré) ; sa
                   taille (size_setup.rs pour size.json) ; ses dessins (assets.rs : lecture
                   tolérante des planches ; figure.rs : confiées à la carte graphique, masques de
                   clic, ce que look.rs décide traduit pour le rendu ; portraits.rs : des images
                   rendues pour les regarder, essais ignorés) ; sa musique (music_view.rs :
                   l'abonnement aux annonces, le lecteur disparu ; music_setup.rs pour
                   music.json ; en version de débogage `capri music rehearse` et
                   `CAPRI_MUSIC_REHEARSAL`, music_rehearsal.rs, lancés par
                   tests/musique_sans_titre.rs) ; l'ouverture de session (login_setup.rs : la case,
                   l'état lu dans le système, la réparation ; ollama_view.rs : Ollama veillé et
                   lancé sur un thread à part)
tools/             récupération vérifiée des images, fabrication de Capri.app (macOS)
```

Règle : tout ce qui peut être testé sans écran va dans `capri-core`, `capri-claude` ou
`capri-chat`.

## État de validation

Vérifié automatiquement :

- formatage, `clippy -D warnings` et 524 tests sur **macOS Apple Silicon** (plus 12 qui ne
  tournent qu'à la main : ils écrivent des images ou dessinent sur la vraie carte graphique,
  lisent un vrai `settings.json`, touchent au vrai trousseau, créent de vraies voix et un vrai
  moteur audio, sans bruit, lisent la vraie liste des fenêtres, ou guettent une annonce d'essai
  d'un lecteur, en ligne de commande et dans la vraie app) : animation, lancer et rebonds, hit-test, clics, bandeau, parole en pages, bulle
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
  autre écran, une place qui n'est pas un nombre) ; la cadence de lecture (4 fois par seconde au
  repos, à chaque rafraîchissement de l'écran pendant que sa fenêtre bouge ou qu'un bouton est
  enfoncé, et une demi-seconde après, sans lire deux fois dans la même image ni sauter celle qui
  suit un réveil en retard) et celle de la boucle (la fréquence de l'écran crue seulement dans des
  bornes sensées, un minuteur qui ne dérive pas et ne repousse jamais un réveil) ; sa pose (l'écrasement
  et le rebond tenus 3 et 2 images, ses pieds immobiles, puis plus aucun flottement ; l'étirement
  au départ ; attrapée, elle quitte le bord) ; `perch.json` (activé sans fichier, retenu, un
  fichier étrange signalé) ;
- sa taille, sans écran : l'échelle toujours dans ses bornes et au centième (ce qui n'est pas un
  nombre donne la taille d'origine), les crans du menu alignés sur les dizaines et arrêtés aux
  bornes, ceux de la molette proportionnels et jamais nuls, la même densité pour un autre dessin
  que pour le dessin debout (2 pixels d'écran par pixel du dessin à 140 % sur un écran Retina) ;
  ses pieds qui restent en place et elle entière à l'écran quand elle change de taille ; posée,
  elle reste si la place suffit, repart sinon, et se pose plus petite là où elle ne tenait pas ;
  `size.json` (la taille d'origine sans fichier, retenue, un nombre hors bornes ramené dedans, un
  fichier étrange signalé) ;
- ses dessins, sans écran : debout, sa boucle tenue en deux et ses clignements aux tours choisis,
  toujours les mêmes ; son flottement au plus bas quand ses cheveux le sont ; sa bouche seulement
  quand elle parle, au même endroit qu'elle, tenue ou lancée aussi ; posée, l'arrivée puis sa
  boucle, sa ligne d'assise sur le bord à moins d'un pixel, et qui suit sa fenêtre ; le départ là
  où elle était, sans étirement avant la fin, puis l'étirement ; attrapée, debout tout de suite,
  tenue par le même endroit d'elle ; parler avant la musique, la musique prise et quittée au début
  d'un tour, une image sautée sans la perdre ; chaque masque de clic selon la pose dessinée ; ce
  qu'il lui faut sur un bord assise (moins de place au-dessus, un morceau de bord plus long, de la
  place pour ses jambes) ; sans planches, ou avec une seule des deux poses, son dessin d'avant comme
  avant ; les vraies planches mesurées (sa largeur assise, ses masques) ; seuls les dessins montrés
  serrés dans les grilles confiées à la carte graphique, chacun intact ; « parler » d'après sa
  bulle ;
- la musique, sans son ni lecteur : seules les trois valeurs exactes de l'état comptent, toute
  autre (ou aucune) vaut « ne joue pas » ; une lecture compte tout de suite, une pause l'arrête
  tout de suite, un changement de morceau la laisse jouer ; abonnée dès le lancement, assise ou
  non, elle ne se balance qu'assise et visible ; décochée, désabonnée et l'état oublié, recochée,
  elle ne sait rien de la musique en cours ; jamais abonnée quand c'est interdit, un seul essai
  quand le système ne sait pas ; un lecteur cru en lecture vérifié une fois toutes les 5 s, oublié
  s'il a disparu, plus aucune vérification sans lecture ; `music.json` (permis sans fichier,
  retenu, un fichier étrange signalé) ;
- se lancer à l'ouverture de session, sans toucher au vrai `~/Library/LaunchAgents` (des dossiers
  d'essai) ni à un vrai Ollama : le texte de l'agent (l'app par `open -g -a`, `RunAtLoad`, session
  graphique seulement, rien qui la relance, aucun autre programme), un chemin étrange écrit et relu,
  seul le chemin absolu d'une app inscrit (jamais un binaire nu), le nôtre reconnu par sa signature
  et rien d'autre (sans elle, ailleurs qu'en tête, en binaire, trop gros, un dossier, un lien) ;
  inscrit, relu, mis à jour, retiré sans rien laisser ni toucher à ses voisins, lisible par tous et
  modifiable par l'utilisateur seul ; l'app déplacée réparée au lancement suivant, une seconde copie
  qui ne prend pas l'inscription, rien de réécrit hors d'une app ; Ollama veillé seulement case
  cochée, cerveau Ollama, sur cette machine et en `http` (ni LM Studio, ni le réseau privé, ni
  Internet) ; `OLLAMA_HOST` (`localhost` devenu `127.0.0.1`, IPv6 entre crochets) ; les emplacements,
  tous absolus, l'app seulement pour l'adresse par défaut ; un Ollama déjà là laissé tranquille et
  sans un mot, un Ollama qui démarre attendu 6 s, lancé une seule fois, puis 30 s pour répondre,
  sur une horloge factice ; introuvable, lancement raté ou muet dits sans commande ; la vraie
  façon de frapper contre de faux serveurs locaux (rien d'envoyé, `localhost` sans résolution,
  refusée avant toute connexion ailleurs que sur cette machine ou en `https`) ; un programme
  inoffensif (`/bin/sh`) lancé détaché avec ses arguments et son environnement ;
- `clippy -D warnings` en compilation croisée pour **Windows x64** et **Linux x64** (le code
  compile, il n'est pas exécuté).

**Vérifié sur ce Mac, sans écran ni bruit** (le 11 novembre 2026, à la main) :

- le rendu sur la vraie carte graphique (`cargo test -p capri-render gpu_tests -- --ignored`) :
  une case de grille dessinée seule, sans rien prendre à ses voisines, même agrandie ;
- de vraies images de chaque pose, rendues par le même code que l'app sur un faux bureau
  (`CAPRI_PORTRAITS_DIR=… cargo test --release -p capri-app portraits -- --ignored`), et
  regardées : debout, clignant, ses cinq bouches (de près, et quatre fois agrandie : elles
  couvrent celle de sa boucle, rien ne dépasse), tenue et lancée (étirée), assise sur le bord d'un
  rectangle clair (le bord passe sous ses cuisses, ses jambes devant), clignant, parlant, en
  musique, l'arrivée et le départ dessin par dessin (nés d'un trait sur le bord, au même endroit
  que sa pose assise), les deux poses côte à côte (à 1,20, la valeur retenue, et à 1,11, celle du projet d'origine) ; et le repli : planches
  refusées par la carte graphique, ou absentes, son dessin d'avant debout comme posée ;
- les annonces (`CAPRI_ASSETS=… cargo test -p capri-app --test musique_sans_titre -- --ignored
  --test-threads=1`, le 11 novembre 2026) : `capri music rehearse` reçoit chaque annonce d'essai,
  postée par un autre processus (lecture, pause, morceau suivant, arrêt, valeur inconnue, sans
  état) en 20 à 25 ms, processus lancé compris, et en lit l'état ; le lecteur d'essai introuvable
  est oublié ; Spotify, qui tourne, est bien trouvé par son identifiant. La vraie app (version de
  débogage, lieux d'essai, `CAPRI_MUSIC_REHEARSAL`) reçoit l'annonce en moins d'une milliseconde
  **alors qu'elle n'est pas active** ; oublie le lecteur d'essai introuvable au bout de 5,0 s ;
  et, la livraison suspendue à la main comme AppKit le ferait, reçoit encore la sienne, quand un
  abonnement ordinaire, lui, ne reçoit rien. Mesuré au passage : AppKit n'avait pas suspendu la
  livraison (Capri n'ayant jamais été active, elle n'a jamais cessé de l'être) ; la livraison
  immédiate la protège pour le jour où il le ferait. Ni la sortie ni le journal (au niveau
  `trace`) ne contiennent les faux titre, artiste et album de l'annonce. Pas vérifié ce jour-là :
  une vraie annonce de Spotify ou de Musique (celles de Spotify l'ont été le 12 novembre 2026 :
  « Vérifié à la main sur un vrai Mac », point 22, plus bas ; celles de Musique jamais) ;
- la vraie app lancée quelques secondes avec des lieux d'essai : planches lues en 44 ms, 47 Mo de
  textures, 2,3 % d'un cœur flottante au repos ; puis avec son seul dessin d'avant
  (`CAPRI_ASSETS`) : elle démarre, le journal dit qu'elle garde son dessin d'avant, 0,4 Mo de
  textures. Ce jour-là, ni le mouvement, ni le suivi d'une vraie fenêtre, ni une vraie musique
  n'ont été regardés (le mouvement et la musique de Spotify l'ont été le 12 novembre 2026 :
  « Vérifié à la main sur un vrai Mac », points 21 et 22, plus bas).

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
  (impossible sans souris) ;
- le suivi d'une fenêtre (le 11 novembre 2026, plus tard), par la répétition
  `CAPRI_PERCH_REHEARSAL=1` de la vraie app : un second processus (`capri perch window`, version de
  débogage) ouvre une petite fenêtre d'essai à lui (400 × 240 points, au milieu de l'écran, sans
  prendre le clavier), Capri y est posée comme si on l'avait lâchée au-dessus de son bord, la
  fenêtre reste immobile, puis fait un va-et-vient de 250 points par programme pendant deux
  secondes, et se ferme : moins de quatre secondes à l'écran. Le journal donne le processeur de
  chaque étape, les lectures et les images par seconde, et le retard : ce sont les chiffres
  ci-dessus. `CAPRI_PERCH_REHEARSAL=avant` fait la même chose avec la cadence d'avant, pour
  comparer. Mesuré en version optimisée avec les assertions de débogage de `capri-app` et
  `capri-platform` seulement (la répétition n'existe pas dans la version livrée) : `cargo build
  --release -p capri-app --config 'profile.release.package.capri-app.debug-assertions=true'
  --config 'profile.release.package.capri-platform.debug-assertions=true'`, puis lancée avec un
  dossier, un trousseau et une socket d'essai, à côté de la Capri de l'utilisateur sans y toucher
  (deux Capri à l'écran pendant une douzaine de secondes, ses raccourcis restant à la première).
  Cinq passages dans cette configuration (quatre avec la nouvelle cadence, un avec celle d'avant),
  et d'autres avant, compilés autrement, qui disent la même chose ;
- sa taille, dans la même répétition : partie de 60 % (lue dans un `size.json` d'essai), posée
  sur la fenêtre d'essai, elle passe par le vrai chemin de l'app à 140 % (440 × 840 pixels), 40 %
  (126 × 240) puis 100 % (314 × 600) : toujours posée, les pieds déplacés de 0 pixel. Lancée trois
  secondes avec un `size.json` illisible : une ligne dans le journal (« elle prend sa taille
  d'origine : … « scale » n'est pas un nombre »), la taille d'origine, et le fichier laissé tel
  quel. Ni le menu ni la molette n'ont été essayés (impossible sans souris).

Le rendu réel sous Linux avec un GPU logiciel (le sprite s'affiche, flotte et « bout ») avait été
vérifié avant le passage à la fenêtre « Overlay » ; il n'a pas été revérifié depuis.

**Vérifié sur ce Mac, pour l'ouverture de session** (le 12 novembre 2026, sans inscrire Capri) :

- notre agent, tel que Capri l'écrit (seuls l'étiquette, l'identifiant associé et le chemin de
  l'app changés pour l'essai, vers une app qui n'existe pas), accepté par `plutil -lint` et chargé
  par `launchctl bootstrap gui/501` : `launchd` y lit `/usr/bin/open -g -a <app>`, le lance aussitôt
  (`RunAtLoad` : une exécution, `open` sort en erreur faute d'app), et lui donne pour `PATH`
  `/usr/bin:/bin:/usr/sbin:/sbin` (d'où les emplacements fixes d'Ollama). Retiré aussitôt
  (`launchctl bootout`, fichier supprimé, rien dans `~/Library/LaunchAgents`) ;
- `ollama serve --help` (Ollama 0.40.14, Homebrew) : `OLLAMA_HOST`, « default 127.0.0.1:11434 ».

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

21. Ses dessins, regardés le 12 novembre 2026 : sa boucle debout et son flottement vont bien, et
    elle s'assoit bien sur les fenêtres, avec ses planches. Pas de retour point par point (voir
    plus bas la liste « Ses dessins et la musique ») ; l'échelle de 1,20 entre ses deux poses n'a
    pas encore été jugée en mouvement.

22. Sa musique avec Spotify, le 12 novembre 2026 : elle se balance à la lecture et s'arrête à la
    pause. La vraie app a reçu de vraies annonces de Spotify (quatre pauses, quatre lectures), et
    l'utilisateur a confirmé que ça fonctionne. Musique (Apple Music) n'a jamais été essayée.

23. L'ouverture de session, le 12 novembre 2026 : la case cochée dans le vrai menu, le fichier de
    l'agent écrit, et valide. Puis l'agent chargé à la main (`launchctl bootstrap`), Ollama arrêté :
    Capri a été lancée, elle a lancé `/opt/homebrew/bin/ollama serve` 7 secondes plus tard, qui a
    répondu une seconde après sur `127.0.0.1:11434`, et qui a continué de tourner une fois Capri
    quittée (par un signal, pas par son menu). Pas fait : une vraie fermeture puis réouverture de
    session, ce que dit son bandeau, le micro quand elle est lancée ainsi, ce qu'affiche Réglages
    Système.

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

**Jalon 3 : à regarder à la main** (le 11 novembre 2026, sur le MacBook Pro à écran ProMotion :
**se poser a été vu fonctionner** ; le reste de la liste n'a pas été regardé point par point, et
elle saccadait en suivant sa fenêtre, d'où la nouvelle cadence ; le 12 novembre 2026, elle s'y
assoit bien, avec ses planches : « Vérifié à la main sur un vrai Mac », point 21, plus haut ;
depuis `Capri.app` reconstruite, ou `RUST_LOG=debug cargo run --release` pour lire le journal) :

1. **se poser** : une fenêtre ordinaire (Finder, TextEdit) dont le haut est vers le milieu de
   l'écran ; lâche Capri doucement, les pieds un peu au-dessus du bord : elle disparaît et
   apparaît assise sur le bord (voir la liste suivante pour ce qu'il faut y regarder). Sans ses
   planches assises : elle descend s'y poser, ses pieds sur le bord, s'écrase un instant, rebondit,
   puis ne bouge plus. Lâchée les pieds dans la barre de titre : elle s'y pose aussi. Lâchée
   nettement plus haut (au-delà d'environ 45 points) : elle flotte, comme avant ;
2. près d'un coin arrondi : ses pieds rentrent un peu sur le bord, elle ne se tient pas dans le
   vide ; contre le bord de l'écran, elle reste entière à l'écran ;
3. **la lancer** vers une fenêtre : si elle s'arrête près d'un bord, elle s'y pose ; sinon elle
   flotte. Les rebonds sur les bords de l'écran sont ceux d'avant ;
4. **suivre** : déplace sa fenêtre lentement puis vite, sur l'écran ProMotion : elle suit **sans
   saccades**, glisse avec la fenêtre au lieu d'avancer par petits sauts, reste sur le bord et dans
   l'écran, et part avec la fenêtre dès le début du déplacement. Un retard d'environ une image
   (quelques points à grande vitesse) est attendu ; un décalage plus grand, ou des à-coups
   réguliers, ne le sont pas. Ses dessins restent au rythme d'avant pendant ce
   temps. Lâche la fenêtre : elle s'arrête avec elle, sans rebond ni dernier saut un instant après.
   Fais de même en la tenant elle-même (attrapée, sans fenêtre) : elle suit le curseur aussi
   doucement. Redimensionne-la par la gauche : elle ne bouge pas ; par la droite,
   jusqu'à plus étroite que l'endroit où elle était : elle glisse au bout du bord, sans le quitter ;
   par un coin du haut : elle monte ou descend avec le bord ;
5. **fenêtre fermée** (⌘W), réduite dans le Dock (⌘M), app masquée (⌘H) : en un quart de seconde
   au plus, elle s'efface du bord là où elle était (une seconde), puis flotte debout au même
   endroit, avec un petit étirement, sans sauter ailleurs ;
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
12. **le processeur**, dans le Moniteur d'activité (% processeur de Capri) : flottante, puis posée
    sans que rien ne bouge (pareil), pendant qu'on déplace sa fenêtre (nettement plus, une dizaine
    de % d'un cœur mesurés, seulement le temps du déplacement), une demi-seconde après l'avoir
    lâchée (de retour au repos), puis cachée (comme cachée avant ce jalon) ;
13. aucune demande de permission n'apparaît (ni Accessibilité, ni Enregistrement de l'écran), et
    elle se pose quand même ;
14. avec `RUST_LOG=debug`, les lignes « posée sur un bord », « aucun bord sous ses pieds » et
    « quitte le bord : … » ne contiennent que des nombres ;
15. si tu utilises Stage Manager ou Mission Control : rien n'a été prévu pour eux, regarder
    seulement que rien ne casse ;
16. **sa taille, par le menu** : « Taille : 100 % » dans les deux menus ; « Plus petite » plusieurs
    fois : elle rapetisse tout de suite, ses pieds au même endroit, jusqu'à 40 %, où « Plus petite »
    s'éteint ; « Plus grande » jusqu'à 140 %, où elle s'éteint à son tour ; « Taille d'origine »
    revient à 100 %. Le titre du sous-menu suit, dans les deux menus. Son bandeau (une session
    Claude Code) et sa bulle (⌃⌥P) se placent au-dessus de sa tête à chaque taille ;
17. **sa taille, d'un geste** : curseur sur elle, ⌥ tenue, la molette ou deux doigts vers le haut :
    elle grandit doucement ; vers le bas : elle rapetisse ; doigts levés, elle s'arrête net (pas
    d'élan). Sans ⌥, faire défiler sur elle ne fait rien ; faire défiler une page à côté d'elle, ⌥
    tenue ou non, ne la change pas ; la tenir et faire défiler ne la change pas non plus ;
18. **posée et changée de taille** : posée sur une fenêtre vers le milieu de l'écran, plus petite
    puis plus grande : elle reste sur le bord, les pieds au même endroit ; posée sur une fenêtre
    dont le haut est assez haut pour qu'à 140 % sa tête sortirait de l'écran : en grandissant, elle
    repart flotter quand il n'y a plus la place ; réduite à 40 %, elle se pose sur une fenêtre qui
    ne pouvait pas la porter à 100 % ;
19. **près des bords de l'écran** : contre le bord droit ou en bas, agrandie, elle reste entière à
    l'écran ;
20. **retenue** : change sa taille, quitte Capri, relance-la : même taille, même titre de
    sous-menu ;
21. **le flou** : à 140 %, son dessin est nettement plus doux qu'à 100 % (attendu) ; à 40 %, son
    trait ne doit ni scintiller ni s'émietter sur l'écran Retina.

**Ses dessins et la musique : à regarder et à écouter à la main** (le 12 novembre 2026, sa boucle
debout et son flottement, elle assise sur les fenêtres, et Spotify en lecture puis en pause ont été
regardés et confirmés, sans retour point par point : « Vérifié à la main sur un vrai Mac », points
21 et 22, plus haut ; le reste de cette liste n'a pas été regardé en mouvement ; les images fixes,
elles, l'ont été, voir plus haut) :

1. **debout** : elle flotte doucement, monte et descend en 2,5 s ; ses cheveux retombent quand
   elle est au plus bas : les deux ensemble, sans à-coup ni dessin qui saute à la fin d'un tour.
   Elle cligne de temps en temps (quatre fois en 20 s), pas à chaque tour ;
2. **la netteté** : à 100 %, sur l'écran Retina, ses bords sont-ils assez nets ? (ses planches y
   sont agrandies 1,73 fois ; légèrement doux sur les images rendues) ; à 140 %, nettement plus
   doux ; à 40 %, son trait ne scintille-t-il pas quand elle flotte ?
3. **attrapée, lancée** : la même boucle, qui continue pendant qu'on la tient ; lancée, étirée dans
   le sens de sa course, écrasée en touchant un bord de l'écran ; on l'attrape bien partout sur
   elle (cheveux, main sur la hanche, pointe du pied), et le clic passe à côté d'elle ;
4. **elle parle**, debout : écris-lui (⌃⌥P) ; sa bouche ne bouge pas pendant « … », puis bouge
   tant que sa bulle montre ses mots, et se ferme une fois la bulle partie. Avec sa voix activée :
   sa bouche bouge pendant qu'on l'entend, et seulement alors (pas pendant « … », ni une fois la
   dernière phrase dite) ; à toi de dire si le décalage entre ses lèvres et sa voix gêne. La bouche
   est-elle bien sur son visage, même pendant un clignement ou un lancer ?
5. **se poser** : sur une fenêtre (Finder, TextEdit) : elle apparaît à partir d'un trait sur le bord
   (un peu plus d'une seconde), puis s'assoit : **le bord de la fenêtre passe sous ses cuisses**,
   sa main tient le bord à gauche, ses jambes pendent devant la fenêtre (pas derrière, pas dans le
   vide au-dessus). Pas de saut entre la fin de l'arrivée et sa boucle assise ;
6. **assise** : elle respire, cligne aux mêmes moments que debout ; sa fenêtre déplacée, elle suit
   (comme au jalon 3) ; une fenêtre dont le haut est à environ 175 points du haut de l'écran la porte
   maintenant (il en fallait plus de 300) ; une fenêtre devant sa main ou son genou la fait partir ;
7. **elle repart** : ferme sa fenêtre (⌘W) : elle s'efface jusqu'à un trait, là où elle était,
   puis flotte debout, avec un petit étirement ; attrape-la pendant qu'elle s'efface : debout tout de
   suite. Attrape-la assise par les cheveux, puis par les pieds : elle se lève sous le curseur,
   tenue à peu près par le même endroit, sans saut ;
8. **elle parle assise** : écris-lui : sa bouche bouge, sans que son corps saute ;
9. **la même échelle** : passe d'une pose à l'autre plusieurs fois (pose-la, attrape-la) : a-t-elle
   l'air de la même taille ? Le rapport est tranché (1,20, le 12 novembre 2026, d'après ce qu'on
   avait vu à 1,11 en mouvement) ; ce qui reste à voir, c'est 1,20 lui-même en mouvement. Les
   images rendues `cote-a-cote.png` (à 1,20) et `cote-a-cote-a-1-11.png` (à 1,11, celui du projet
   d'origine) montrent les deux ;
10. **Spotify, lecture puis pause** (fait le 12 novembre 2026 pour la lecture et la pause :
    « Vérifié à la main sur un vrai Mac », point 22 ; reste à essayer : lui écrire pendant la
    musique) : Capri lancée, assise, mets un morceau dans Spotify : au début du tour suivant de sa
    boucle (jusqu'à 2,5 s), elle se balance, les yeux fermés ; mets en pause : elle reprend sa
    boucle au tour suivant ; relance la lecture : elle se balance de nouveau. `capri music
    rehearse` (version de débogage) dit combien d'annonces de Spotify elle a reçues pendant qu'elle
    tourne. Écris-lui pendant la musique : elle parle tout de suite, puis reprend la musique au tour
    suivant ;
11. **Musique, lecture puis pause** : la même chose dans l'app Musique d'Apple ;
12. **changement de morceau** : pendant la lecture, passe au morceau suivant (bouton, ou laisse
    finir le morceau) dans Spotify, puis dans Musique : elle continue de se balancer, sans revenir
    à sa boucle entre les deux ;
13. **quitter le lecteur pendant la lecture** : en musique, quitte Spotify (⌘Q) : elle reprend sa
    boucle au plus 5 s plus tard (plus un tour) ; refais-le en forçant Spotify à quitter (Moniteur
    d'activité) : pareil. Puis avec Musique ;
14. **musique déjà en cours au lancement** (limite connue) : mets la musique, puis lance Capri et
    assieds-la : elle ne se balance pas ; pause puis lecture (ou morceau suivant) : elle se balance ;
15. **un autre son** : une vidéo dans un navigateur, un jeu, Spotify coupé : elle ne se balance pas
    (voulu) ;
16. **debout, la musique ne change rien** : avec un morceau, attrape-la : debout, sa boucle
    habituelle ; repose-la : elle se balance de nouveau dès son arrivée assise finie (au début d'un
    tour), sans attendre d'annonce ;
17. **la case décochée** : « Se balancer sur la musique » est cochée dans les deux menus ;
    décoche-la pendant la musique : assise, elle reprend sa boucle au tour suivant ; recoche-la, la
    musique continuant : elle ne se balance pas avant la prochaine annonce (pause puis lecture) ;
    quitte et relance, case décochée : toujours décochée (`music.json`), et la musique ne la fait
    pas se balancer ; recoche-la ;
18. **cachée** (⌃⌥C) pendant la musique : rien ne bouge ; montrée de nouveau, elle reprend la
    musique au tour suivant ; mets en pause pendant qu'elle est cachée, puis montre-la : sa boucle ;
19. **le processeur et la mémoire** (Moniteur d'activité) : flottante, environ 2 % d'un cœur
    (comme avant) ; assise, pareil ; en musique, pas plus ; « Mémoire » d'environ 250 Mo (contre
    200 sans ses planches) ;
20. **le repli** : lance-la avec `CAPRI_ASSETS` vers un dossier qui n'a que
    `capri/capri-cutout.png` : son dessin d'avant, qui bout, posée debout sur les fenêtres, sans
    bouche, et sans se balancer ;
21. aucune demande de permission n'apparaît (ni micro, ni Automatisation, ni Enregistrement de
    l'écran) quand elle guette la musique ; `RUST_LOG=debug` ne dit que « se balance » ou « ne se
    balance plus » (et « elle guette les annonces des lecteurs »), jamais un lecteur, un titre ni un
    artiste ;
22. **dans `Capri.app`** (et non le binaire seul) : les points 10 et 13 au moins, puisque les
    essais automatiques n'ont lancé que le binaire de débogage.

**L'ouverture de session : à vérifier à la main** (le 12 novembre 2026, une partie du point 1 et
du point 4, voir « Vérifié à la main sur un vrai Mac », point 23 : la case cochée dans le vrai
menu et le fichier écrit, valide ; Capri lancée par l'agent chargé à la main, Ollama lancé par
elle et qui répond, toujours là après qu'elle a été quittée par un signal ; ni son bandeau, ni
« Quitter » de son menu. Le reste de la liste n'a pas été fait ; dans `Capri.app`, refaite par
`tools/bundle-macos.sh`) :

1. **la case** : « Se lancer à l'ouverture de session » décochée dans les deux menus ; cochée dans
   l'un, elle l'est dans l'autre, et le bandeau le dit ; le fichier
   `~/Library/LaunchAgents/local.capri-daemon.capri.login.plist` est là, et nomme ton
   `Capri.app` ; une notification de macOS sur les éléments d'arrière-plan, ou pas (à noter) ;
2. **Réglages Système** → Général → Ouverture (éléments d'arrière-plan) : Capri y figure, sous son
   nom ;
3. **l'ouverture de session** : ferme ta session (ou redémarre), rouvre-la : Capri apparaît seule,
   sans prendre le clavier à l'app au premier plan ; le micro (⌃⌥D) marche comme après un
   double-clic ;
4. **Ollama arrêté** (`brew services stop ollama`, puisqu'il tourne aujourd'hui par
   `brew services`), cerveau Ollama sur `localhost:11434` : relance Capri ; au bout de 6 à 10 s,
   « J'ai lancé Ollama, mon cerveau est prêt », et elle répond (⌃⌥P) ; quitte Capri : Ollama tourne
   encore (`curl http://127.0.0.1:11434/` répond « Ollama is running ») ;
5. **Ollama déjà lancé** : relance Capri : rien n'est dit, aucun second Ollama
   (`pgrep -fl "ollama serve"`) ;
6. **décoche** : le bandeau le dit, le fichier a disparu, les autres fichiers du dossier sont
   intacts ; à la session suivante, elle ne se lance plus ;
7. **déplacée** : case cochée, déplace `Capri.app`, lance-la depuis sa nouvelle place : le
   fichier nomme la nouvelle place ;
8. **depuis le terminal** (`cargo run --release`) : cocher la case dit « Pour m'inscrire,
   lance-moi depuis Capri.app », et rien n'est écrit.

Note : aujourd'hui, Ollama est lancé par `brew services`, mais aucun fichier `sh.brew.ollama.plist`
n'est dans `~/Library/LaunchAgents` (celui que `brew services` charge est dans le dossier de
Homebrew) : c'est sans doute pour cela qu'il ne tournait plus après le redémarrage. Ce n'est
qu'une déduction : `brew services start ollama` l'inscrirait pour de bon, indépendamment de Capri.

**Pas encore testé sur un vrai PC Windows** : la CI de GitHub y compile et y lance les tests
(477 réussis le 12 novembre 2026 au commit `81e348b`, dont ceux du tube nommé et du hook ; au
commit `807ab78`, quatre essais de l'ouverture de session y ont échoué, écrits pour des chemins
d'Unix, puis corrigés par `6f8b876`, dont le résultat dans la CI n'est pas encore connu), mais
personne n'y a encore lancé Capri. Il n'y a pas encore de fenêtre de réglages. Le tube nommé de `capri-ipc` n'y a
tourné que dans ces tests, le champ de saisie et la voix n'y existent pas encore,
et le chat n'y a jamais été lancé : ni le https (SChannel), ni le Gestionnaire d'identification,
ni la saisie d'une clé sans écho n'y ont tourné. « Se lancer à l'ouverture de session » y est
grisée : ni l'inscription, ni le lancement d'Ollama n'y sont écrits. Le style de sortie (dans
`%USERPROFILE%\.claude\output-styles`) et le miroir (dans `%LOCALAPPDATA%\Capri`) n'y ont pas
tourné non plus. La liste des fenêtres (jalon 3 : `EnumWindows`, le cadre et l'état « masquée »
de DWM, l'état des boutons de la souris) est écrite et compilée, jamais exécutée : sur un vrai PC,
`cargo test --release -p capri-platform --test liste_des_fenetres -- --ignored --nocapture`, puis
les points du jalon 3 ci-dessus (les fenêtres d'un autre bureau virtuel et les fenêtres réduites
ne doivent pas la porter). Sa taille y passe par les mêmes menus et par **Ctrl** + molette sur
elle (l'état de Ctrl lu auprès du système, la fenêtre n'ayant jamais le clavier) : écrit et
compilé, jamais exécuté ; que la molette arrive bien à sa fenêtre non activable, et que le suivi à
la fréquence de l'écran y soit fluide, reste à voir.

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
   Windows : Win32) : écrit et testé sans écran ; se poser vu sur un vrai Mac le 11 novembre 2026,
   puis le suivi rendu fluide (à la cadence de l'écran, mesuré sans écran) ; ~~les dessins de
   l'artiste du projet Capri, debout et assise~~, ~~se balancer sur ta musique~~ (écrits, testés
   sans écran, images fixes rendues et regardées ; regardés en mouvement le 12 novembre 2026 : sa
   boucle debout, son flottement, assise sur les fenêtres, l'échelle de 1,20 pas encore jugée ainsi ;
   la musique reconnue par les annonces de Spotify et de Musique : de vraies annonces de Spotify
   reçues par la vraie app et confirmées le 12 novembre 2026, Musique jamais essayée) ; reste à
   regarder le reste sur un vrai Mac, et à l'exécuter sur un vrai PC Windows.
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

Le **code** est sous licence **MIT** ou **Apache-2.0**, au choix de qui l'utilise :
[LICENSE-MIT](LICENSE-MIT) et [LICENSE-APACHE](LICENSE-APACHE).

Sauf demande contraire de votre part, toute contribution soumise pour être incluse dans ce projet,
au sens de la licence Apache-2.0, est placée sous cette même double licence, sans condition
supplémentaire.

Les **images** n'en relèvent pas, et elles ne sont pas dans le dépôt : voir
[CREDITS.md](CREDITS.md). Cela ne change rien au reste : Capri reste un projet de fan non
commercial, sans affiliation avec les ayants droit du personnage.
