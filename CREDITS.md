# Crédits

## Le personnage

**Capri** (*Project Capricorn*) est un personnage de *Cyberpunk: Edgerunners* saison 2.
Capri, Cyberpunk et Cyberpunk: Edgerunners appartiennent à **CD PROJEKT RED**, **Studio Trigger**
et **Netflix**. Ce projet est une création de fan, **non commerciale**, sans aucune affiliation
avec eux.

## Les images (`assets/capri/`)

Les images ne sont pas incluses dans ce dépôt : `tools/fetch-assets.sh` (ou `.ps1`) les télécharge
depuis le dépôt [mochiiwuff/capri-desktop](https://github.com/mochiiwuff/capri-desktop), à la
révision `c011764ca1fdbd191925321e3aaa8af5b84babe8`, et vérifie l'empreinte SHA-256 de chacune.

D'après ce dépôt, elles proviennent de captures de la série, d'un détourage réalisé par un fan et
de fan art (les gros plans de l'introduction). Merci à **mochiiwuff** pour leur sélection et leur
préparation, ainsi qu'aux fans qui les ont créées.

**Ces images ne sont couvertes par aucune licence de ce projet**, et ce projet ne revendique aucun
droit sur elles. Si un ayant droit souhaite leur retrait, ouvrez une issue : elles seront retirées
immédiatement. Le code les charge au lancement, sans les intégrer au binaire, pour qu'on puisse les
remplacer sans recompiler.

## Le code

Écrit de zéro en Rust. L'idée (une compagne de bureau qui commente un assistant de code) vient de
[mochiiwuff/capri-desktop](https://github.com/mochiiwuff/capri-desktop) et de son portage macOS
[CyberMoonie/capri-macos](https://github.com/CyberMoonie/capri-macos), mais **aucune ligne de leur
code n'est reprise**.

L'icône de la barre des menus est dessinée par le programme lui-même et ne reprend aucune image de
la série.
