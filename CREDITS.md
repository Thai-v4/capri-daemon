# Crédits

## Le personnage

**Capri** (*Project Capricorn*) est un personnage de *Cyberpunk: Edgerunners* saison 2.
Capri, Cyberpunk et Cyberpunk: Edgerunners appartiennent à **CD PROJEKT RED**, **Studio Trigger**
et **Netflix**. Ce projet est une création de fan, **non commerciale**, sans aucune affiliation
avec eux.

## Les images (`assets/capri/`)

Les images ne sont pas incluses dans ce dépôt : `tools/fetch-assets.sh` (ou `.ps1`) télécharge
celles de `assets/capri/` (sauf `perch/`, `stand/`, `life/` et `intro/`, voir plus bas) depuis le dépôt [mochiiwuff/capri-desktop](https://github.com/mochiiwuff/capri-desktop), à la
révision `c011764ca1fdbd191925321e3aaa8af5b84babe8`, et vérifie l'empreinte SHA-256 de chacune.

D'après ce dépôt, elles proviennent de captures de la série, d'un détourage réalisé par un fan et
de fan art (les gros plans de l'introduction). Merci à **mochiiwuff** pour leur sélection et leur
préparation, ainsi qu'aux fans qui les ont créées.

**Ces images ne sont couvertes par aucune licence de ce projet**, et ce projet ne revendique aucun
droit sur elles. Si un ayant droit souhaite leur retrait, ouvrez une issue : elles seront retirées
immédiatement. Le code les charge au lancement, sans les intégrer au binaire, pour qu'on puisse les
remplacer sans recompiler.

### Ses poses assise (`assets/capri/perch/`) et debout (`assets/capri/stand/`)

Les planches de sa pose assise sur une fenêtre (`perch.png`, `perch_blink.png`, `perch_talk.png`,
`perch_music.png`, `perch_arrive.png`, `perch_leave.png`) et celles de sa pose debout (`idle.png`,
`idle_blink.png`, `idle_mouth.png`) viennent du projet **Capri**
([lachydotmcg/capri-desktop](https://github.com/lachydotmcg/capri-desktop)), à la révision
`b40fc2ed271976545c7a06ef78038a07a38f1dd5` (du 23 novembre 2026 ; ces neuf fichiers y sont les
mêmes, à l'octet près, qu'à la révision `6f719c4` du 11 novembre, d'où elles venaient d'abord).
Son README attribue les premières ainsi : « Window perch
and music perch animations: her artist, who prefers to stay private. » Les secondes y sont arrivées
ensemble, par le commit `cf3fa56` (« she stands in her artist's new loop at 24 fps… »), et sa
licence dit que « Capri's art and animations are by her artist » ; son README ne les range pas parmi
les parties faites par d'autres. Merci à cette artiste, et à **Lachy** pour le projet.

Trois planches de plus viennent des mêmes jeux, à la même révision : un éclat de glitch quand elle
est assise (`perch_glitch.png`) et une variante à glitch de son clignement, assise
(`perch_blink_glitch.png`) et debout (`idle_blink_glitch.png`). Les commits qui les ont mises à
jour les rattachent aux dessins de son artiste (`163bd619` : « her perch uses her artist's 30 oct
frames… », `07320d71` : « she blinks on her perch with her artist's own frames 29 to 36… »,
`cf3fa560`, déjà cité).

Quand les planches de sa pose debout manquent, elle garde son dessin debout d'avant
(`capri-cutout.png`, de mochiiwuff/capri-desktop, voir plus haut).

### Les planches de sa vie (`assets/capri/life/`)

Six planches viennent du même projet, à la même révision : elle apparaît et s'efface
(`arrive.png`, `leave.png`), marche (`walk.png`), pend quand on la tient (`dangle.png`), se relève
(`glitch.png`) et sourit (`grin.png`). **Ce dépôt ne nomme pas leur auteur.** Elles y sont
arrivées sans crédit, par le commit `79e707d5` du 25 octobre 2026 (« added her animations and
kept her above games » : `dangle`, `glitch`, `grin`) et par le commit `eebe1854` du 27 octobre
2026 (« released 0.7.0 … » : `arrive`, `leave`, `walk`, et la dernière version de `dangle`). Sa
licence dit que « Capri's art and animations are by her artist », et que les parties faites par
d'autres sont listées dans son README : elles n'y figurent pas. Elles montrent un dessin plus
ancien de Capri que ses poses debout et assise (de face, un bras tendu), et font partie d'un jeu
de cases de 240 px dont ce projet n'utilise que ces six planches.

Jusqu'au 23 novembre 2026, ce projet écartait ce jeu de cases, faute d'auteur nommé. Il les prend
depuis, par décision de son auteur, aux mêmes conditions que ses poses : la même licence, les
mêmes mentions obligatoires, le même usage non commercial. Si leur auteur souhaite être nommé, ou
leur retrait, ouvrez une issue : ce sera fait aussitôt.

### Son intro au démarrage (`assets/capri/intro/`)

Les 22 images de l'intro en plein écran (« Up close » : elle surgit dans l'écran et y colle son
visage) viennent du même projet, à la même révision (`assets/intros/up-close/`). **Ce dépôt ne
nomme pas leur auteur** non plus : elles y sont arrivées par le commit `49c2ff5b` du 21 novembre
2026, dont la demande de fusion parle de « Two new hand-drawn startup animations, Peek and Up
close », sans crédit, et son README ne les range pas parmi les parties faites par d'autres. Ce
projet les prend aux mêmes conditions que les planches de sa vie, par la même décision du 23
novembre 2026.

**Restent écartées** les images que ce dépôt crédite à quelqu'un d'autre que son artiste : l'effet
de clic (mixmix312352), l'animation de transformation et les deux emotes (Qewiw), et l'intro
« Peek », dont les images actuelles sont de Qewiw d'après la demande de fusion qui les a apportées.

### Leur licence

Les images de ces quatre dossiers sont sous la licence du projet Capri, la
[PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0), dont
voici les mentions obligatoires, recopiées telles quelles :

```text
Required Notice: Copyright 2026 Lachy (https://github.com/lachydotmcg)
Required Notice: Anyone who uses Capri's code, art, animations or any of its other assets must credit the Capri project (https://github.com/lachydotmcg/capri-desktop). Capri's art and animations are by her artist, who prefers to stay private. Parts made by others keep their own licences, listed in its README.
```

`tools/fetch-assets.sh` (ou `.ps1`) les télécharge depuis ce dépôt et vérifie l'empreinte SHA-256
de chacune ; elles ne sont pas dans ce dépôt-ci. Elles ne servent qu'à un usage **non commercial**,
le seul que cette licence permette, et ce projet ne revendique aucun droit sur elles : elles ne sont
couvertes par aucune licence de ce projet. Aucune autre image de ce dépôt n'est utilisée (ni celles
créditées à d'autres auteurs, ni ses autres animations), et aucune ligne de son code n'est
reprise : le découpage des planches, la place du bord de la fenêtre, sa hauteur et ses pieds ont
été mesurés sur les images elles-mêmes (`crates/capri-core/src/sheets.rs`, `sheets/stand.rs` et
`sheets/life.rs`) ; seuls leur cadence d'origine, remplacée ensuite par la nôtre, le rapport
d'échelle entre ses deux poses et le décalage entre ses cases de 240 px et celles de sa pose
debout ont été lus dans son code.

### L'icône de l'app

L'icône de `Capri.app` (`Capri.icns`) est tirée de `capri-face.png`, l'une des images de
mochiiwuff/capri-desktop ci-dessus : `tools/bundle-macos.sh` la fabrique sur ta machine, par des
réductions de l'image que tu as téléchargée, et la range dans l'app (elle n'est pas dans ce dépôt).
Elle suit donc les mêmes conditions que ces images : aucune licence de ce projet, retrait
immédiat sur demande, et une app qui la contient ne se distribue pas.

## Le code

Écrit de zéro en Rust. L'idée (une compagne de bureau qui commente un assistant de code) vient de
[mochiiwuff/capri-desktop](https://github.com/mochiiwuff/capri-desktop) et de son portage macOS
[CyberMoonie/capri-macos](https://github.com/CyberMoonie/capri-macos), mais **aucune ligne de leur
code n'est reprise**. L'idée qu'elle se balance quand de la musique joue vient de
[lachydotmcg/capri-desktop](https://github.com/lachydotmcg/capri-desktop) (PolyForm Noncommercial
1.0.0), dont aucune ligne de code n'est reprise non plus.

Le bandeau doit plusieurs idées à [CapriChooms/CapriCore](https://github.com/CapriChooms/CapriCore)
(GPL-3.0) : un dialogue flottant qui se retire tout seul, un état affiché sans ambiguïté, une
cadence décidée par des règles locales avant tout appel de modèle, et le silence comme choix
valide. Là encore, **aucune ligne de son code n'est reprise**, et aucune de ses images.

L'icône de la barre des menus est dessinée par le programme lui-même et ne reprend aucune image de
la série. Le texte du bandeau est tracé avec les polices déjà présentes sur ta machine : aucune
police n'est distribuée avec ce projet.
