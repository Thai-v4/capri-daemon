# Crédits

## Le personnage

**Capri** (*Project Capricorn*) est un personnage de *Cyberpunk: Edgerunners* saison 2.
Capri, Cyberpunk et Cyberpunk: Edgerunners appartiennent à **CD PROJEKT RED**, **Studio Trigger**
et **Netflix**. Ce projet est une création de fan, **non commerciale**, sans aucune affiliation
avec eux.

## Les images (`assets/capri/`)

Les images ne sont pas incluses dans ce dépôt : `tools/fetch-assets.sh` (ou `.ps1`) télécharge
celles de `assets/capri/` (sauf `perch/` et `stand/`, voir plus bas) depuis le dépôt [mochiiwuff/capri-desktop](https://github.com/mochiiwuff/capri-desktop), à la
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
`6f719c4269b0fa0e841587f072a1b1594eebcb91`. Son README attribue les premières ainsi : « Window perch
and music perch animations: her artist, who prefers to stay private. » Les secondes y sont arrivées
ensemble, par le commit `cf3fa56` (« she stands in her artist's new loop at 24 fps… »), et sa
licence dit que « Capri's art and animations are by her artist » ; son README ne les range pas parmi
les parties faites par d'autres. Merci à cette artiste, et à **Lachy** pour le projet.

Quand les planches de sa pose debout manquent, elle garde son dessin debout d'avant
(`capri-cutout.png`, de mochiiwuff/capri-desktop, voir plus haut).

Elles sont sous la licence de ce projet, la
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
créditées à d'autres auteurs, ni ses autres animations, ni l'ancien jeu de cases de 240 px dont
l'auteur n'est pas nommé), et aucune ligne de son code n'est reprise : le découpage des planches, la
place du bord de la fenêtre, sa hauteur et ses pieds ont été mesurés sur les images elles-mêmes
(`crates/capri-core/src/sheets.rs` et `sheets/stand.rs`) ; seuls leur cadence d'origine, remplacée
ensuite par la nôtre, et le rapport d'échelle entre ses deux poses ont été lus dans son code.

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
