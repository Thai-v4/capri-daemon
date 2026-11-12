//! La conversation avec Capri, sans rien toucher au système ni au réseau.
//!
//! - [`conversation`] : qui a dit quoi, avec un historique borné ;
//! - [`reply`] : une réponse arrive par morceaux, comme un flux ;
//! - [`brain`] : quel cerveau elle utilise (fournisseur, adresse, modèle) et son fichier de
//!   réglages ;
//! - [`endpoint`] : à quelle adresse elle a le droit d'envoyer une conversation, et une clé ;
//! - [`key`] : la clé d'API d'un fournisseur, qui ne s'affiche jamais, ne part jamais en clair, et
//!   seulement vers l'adresse pour laquelle elle a été donnée ;
//! - [`phrases`] : ce que disent de son cerveau la ligne de commande et la fenêtre de réglages,
//!   à commencer par où part ce qu'on lui dit ;
//! - [`persona`] : qui elle est quand elle parle ;
//! - [`wire`] : les deux formats que parlent les fournisseurs, et lequel parle chacun ;
//! - [`openai`] : le format « chat completions », ce qu'on envoie et ce qu'on lit ;
//! - [`anthropic`] : le format de l'API d'Anthropic, ce qu'on envoie et ce qu'on lit ;
//! - [`plain`] : du texte brut à partir de ce qu'écrit un modèle, Markdown retiré ;
//! - [`sse`] : le décodeur du flux dans lequel la réponse arrive ;
//! - [`dictation`] : lui parler au micro, le déroulé d'une prise de parole ;
//! - [`voice`] : de quoi dire ses réponses à voix haute (phrases, texte prononçable, choix de la
//!   voix, réglages) ;
//! - [`settings`] : le modèle de la fenêtre de réglages, ses rubriques (« Cerveau », « Voix »),
//!   ce qu'elle affiche et ce qu'elle demande à l'app de faire ;
//! - [`startup`] : se lancer à l'ouverture de session avec Ollama, quand c'est son cerveau sur
//!   cette machine : faut-il le lancer, où le chercher, ce qu'elle en dit ;
//! - [`echo`] : le cerveau de secours, qui répète ce qu'on lui dit tant qu'aucun modèle n'est
//!   choisi.
//!
//! Se connecter est le travail de `capri-net`, le seul crate qui parle au réseau.

pub mod anthropic;
pub mod brain;
pub mod conversation;
pub mod dictation;
pub mod echo;
pub mod endpoint;
pub mod key;
pub mod openai;
pub mod persona;
pub mod phrases;
pub mod place;
pub mod plain;
pub mod reply;
pub mod settings;
pub mod sse;
pub mod startup;
pub mod voice;
pub mod wire;

pub use brain::{Brain, BrainError, KeyNeed, Provider};
pub use conversation::{Conversation, Role, Turn};
pub use endpoint::{Endpoint, EndpointError, Reach};
pub use key::{ApiKey, BoundKey, KeyAccount, KeyError};
pub use place::{KeyJob, Place};
pub use reply::ReplyEvent;
pub use wire::Wire;
