//! Ce que Capri sait de Claude Code, sans rien toucher au système.
//!
//! - [`hook_input`] : lire ce que Claude Code écrit sur l'entrée standard d'un hook ;
//! - [`protocol`] : les messages que `capri-hook` envoie à Capri, une ligne JSON chacun ;
//! - [`sessions`] : à partir de ces messages, quoi afficher dans le bandeau ;
//! - [`settings`] : brancher et débrancher le hook dans `settings.json`, de texte à texte ;
//! - [`places`] : où vivent les réglages de Capri et ceux de Claude Code, pour que le hook et
//!   l'app trouvent les mêmes ;
//! - [`style`] : le style de sortie « Capri », que l'utilisateur installe et choisit dans Claude
//!   Code ;
//! - [`mirror`] : le miroir, désactivé par défaut, qui fait dire à Capri la ligne que Claude a
//!   écrite pour elle, et rien d'autre de sa réponse.
//!
//! Les noms d'événements et de champs viennent de la référence des hooks de Claude Code
//! (<https://code.claude.com/docs/en/hooks>). Tout ce qui est inconnu est ignoré en silence :
//! un hook ne doit jamais gêner la session qu'il observe.

pub mod hook_input;
pub mod mirror;
pub mod places;
pub mod protocol;
pub mod sessions;
pub mod settings;
pub mod style;
pub mod text;

pub use protocol::{Event, Message};
pub use sessions::{Headline, Tone, Tracker};
