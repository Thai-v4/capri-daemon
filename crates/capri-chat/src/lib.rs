//! La conversation avec Capri, sans rien toucher au système ni au réseau.
//!
//! - [`conversation`] : qui a dit quoi, avec un historique borné ;
//! - [`reply`] : une réponse arrive par morceaux, comme un flux ;
//! - [`echo`] : le cerveau provisoire, qui répète ce qu'on lui dit. Il sert à mettre au point la
//!   bulle et la saisie avant de brancher un vrai modèle.

pub mod conversation;
pub mod echo;
pub mod reply;

pub use conversation::{Conversation, Role, Turn};
pub use reply::ReplyEvent;
