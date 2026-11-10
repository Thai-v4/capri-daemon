//! Cœur de Capri : tout ce qui ne dépend ni du système, ni du GPU.
//!
//! Unités : pixels physiques de la fenêtre overlay, origine en haut à gauche, y vers le bas.
//! Temps : la base est l'image d'animation à 24 i/s ; les dessins sont tenus « en deux »
//! (12 dessins par seconde), comme en anime.

pub mod anim;
pub mod banner;
pub mod body;
pub mod geom;
pub mod input;
pub mod mask;
pub mod speech;

pub use anim::{FRAME_RATE, Sheet, frame_at};
pub use banner::{Banner, BannerPose, place_banner, place_input};
pub use body::{Capri, Pose, Stance};
pub use geom::{Rect, Vec2};
pub use input::{Button, Press, interpret_press};
pub use mask::AlphaMask;
pub use speech::{Speech, SpeechView};
