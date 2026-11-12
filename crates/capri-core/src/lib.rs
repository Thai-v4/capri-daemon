//! Cœur de Capri : tout ce qui ne dépend ni du système, ni du GPU.
//!
//! Unités : pixels physiques de la fenêtre overlay, origine en haut à gauche, y vers le bas.
//! Temps : la base est l'image d'animation à 24 i/s ; les dessins sont tenus « en deux »
//! (12 dessins par seconde), comme en anime.
//!
//! [`form`] décrit un formulaire de réglages (rubriques, lignes, événements) sans rien savoir de
//! l'écran : le modèle des réglages le produit, la fenêtre de `capri-platform` l'affiche.
//! [`sound`] décrit de même comment dire une phrase (voix, hauteur, débit, traitements du son)
//! sans rien savoir du système : `capri-chat` la compose, la voix de `capri-platform` l'applique.
//! [`perch`] décide où elle se pose sur les fenêtres des autres apps, d'après leurs seuls
//! rectangles, que la liste des fenêtres de `capri-platform` lui décrit. [`sheets`] décrit les
//! planches de ses poses assise et debout (cases, dessins, où passe le bord de la fenêtre), et
//! [`look`] dit lequel montrer à chaque image, où, et ce qui l'attrape. [`music`] décide,
//! d'après l'état que les lecteurs connus ont annoncé, si de la musique joue.
//! [`pace`] dit quand la boucle se réveille : ses dessins restent à 24 i/s, mais sa position suit
//! le curseur ou sa fenêtre à la cadence de l'écran.
//! [`scale`] dit sa taille, choisie par l'utilisateur : une échelle rapportée à sa hauteur debout,
//! que tous ses dessins partagent.
//! [`login`] dit le texte de son inscription à l'ouverture de session (un agent de lancement de
//! macOS) et reconnaît la nôtre : `capri-platform` l'écrit et la retire.

pub mod anim;
pub mod banner;
pub mod body;
pub mod form;
pub mod geom;
pub mod input;
pub mod login;
pub mod look;
pub mod mask;
pub mod music;
pub mod pace;
pub mod perch;
pub mod scale;
pub mod sheets;
pub mod sound;
pub mod speech;

pub use anim::{FRAME_RATE, Sheet, frame_at};
pub use banner::{Banner, BannerPose, place_banner, place_input};
pub use body::{Capri, Pose, Stance};
pub use geom::{Rect, Vec2};
pub use input::{Button, Press, interpret_press};
pub use mask::AlphaMask;
pub use scale::Scale;
pub use speech::{Speech, SpeechView};
