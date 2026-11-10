//! La conversation avec Capri, sans rien toucher au système ni au réseau.
//!
//! - [`conversation`] : qui a dit quoi, avec un historique borné ;
//! - [`reply`] : une réponse arrive par morceaux, comme un flux ;
//! - [`brain`] : quel cerveau elle utilise (fournisseur, adresse, modèle) et son fichier de
//!   réglages ;
//! - [`endpoint`] : à quelle adresse elle a le droit d'envoyer une conversation ;
//! - [`persona`] : qui elle est quand elle parle ;
//! - [`openai`] : le format « chat completions », ce qu'on envoie et ce qu'on lit ;
//! - [`plain`] : du texte brut à partir de ce qu'écrit un modèle, Markdown retiré ;
//! - [`sse`] : le décodeur du flux dans lequel la réponse arrive ;
//! - [`echo`] : le cerveau de secours, qui répète ce qu'on lui dit tant qu'aucun modèle n'est
//!   choisi.
//!
//! Se connecter est le travail de `capri-net`, le seul crate qui parle au réseau.

pub mod brain;
pub mod conversation;
pub mod echo;
pub mod endpoint;
pub mod openai;
pub mod persona;
pub mod plain;
pub mod reply;
pub mod sse;

pub use brain::{Brain, BrainError, Provider};
pub use conversation::{Conversation, Role, Turn};
pub use endpoint::{Endpoint, EndpointError};
pub use reply::ReplyEvent;
