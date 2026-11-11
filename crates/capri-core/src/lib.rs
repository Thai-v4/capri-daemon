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
//! rectangles, que la liste des fenêtres de `capri-platform` lui décrit.

pub mod anim;
pub mod banner;
pub mod body;
pub mod form;
pub mod geom;
pub mod input;
pub mod mask;
pub mod perch;
pub mod sound;
pub mod speech;

pub use anim::{FRAME_RATE, Sheet, frame_at};
pub use banner::{Banner, BannerPose, place_banner, place_input};
pub use body::{Capri, Pose, Stance};
pub use geom::{Rect, Vec2};
pub use input::{Button, Press, interpret_press};
pub use mask::AlphaMask;
pub use speech::{Speech, SpeechView};
