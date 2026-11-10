//! La conversation avec Capri, sans rien toucher au système ni au réseau.
//!
//! - [`conversation`] : qui a dit quoi, avec un historique borné ;
//! - [`reply`] : une réponse arrive par morceaux, comme un flux ;
//! - [`brain`] : quel cerveau elle utilise (fournisseur, adresse, modèle) et son fichier de
//!   réglages ;
//! - [`endpoint`] : à quelle adresse elle a le droit d'envoyer une conversation, et une clé ;
//! - [`key`] : la clé d'API d'un fournisseur, qui ne s'affiche jamais, ne part jamais en clair, et
//!   seulement vers l'adresse pour laquelle elle a été donnée ;
//! - [`persona`] : qui elle est quand elle parle ;
//! - [`openai`] : le format « chat completions », ce qu'on envoie et ce qu'on lit ;
//! - [`plain`] : du texte brut à partir de ce qu'écrit un modèle, Markdown retiré ;
//! - [`sse`] : le décodeur du flux dans lequel la réponse arrive ;
//! - [`dictation`] : lui parler au micro, le déroulé d'une prise de parole ;
//! - [`voice`] : de quoi dire ses réponses à voix haute (phrases, texte prononçable, choix de la
//!   voix, réglages) ;
//! - [`echo`] : le cerveau de secours, qui répète ce qu'on lui dit tant qu'aucun modèle n'est
//!   choisi.
//!
//! Se connecter est le travail de `capri-net`, le seul crate qui parle au réseau.

pub mod brain;
pub mod conversation;
pub mod dictation;
pub mod echo;
pub mod endpoint;
pub mod key;
pub mod openai;
pub mod persona;
pub mod plain;
pub mod reply;
pub mod sse;
pub mod voice;

pub use brain::{Brain, BrainError, KeyNeed, Provider};
pub use conversation::{Conversation, Role, Turn};
pub use endpoint::{Endpoint, EndpointError, Reach};
pub use key::{ApiKey, BoundKey, KeyAccount, KeyError};
pub use reply::ReplyEvent;
