//! Le trousseau du système : là où vivent les clés d'API, et nulle part ailleurs.
//!
//! - macOS : le trousseau de session (Trousseau d'accès), des mots de passe d'application
//!   rangés par service et par compte (`SecItem…`, par le crate `security-framework`, déjà utilisé
//!   par le TLS du système). macOS rattache chaque élément à l'app qui l'a créé : une autre app
//!   (ou la même, reconstruite) doit demander la permission de le lire, et le système pose la
//!   question à l'utilisateur ;
//! - Windows : le Gestionnaire d'identification, des identifiants génériques (`Cred…W`) ;
//! - ailleurs : pas de trousseau, et chaque fonction le dit ([`KeychainError::Unavailable`]).
//!
//! Ce module ne garde rien en mémoire, n'écrit rien dans le journal, et ne met jamais un secret dans
//! un message d'erreur : ses erreurs ne portent qu'un code du système.

#[cfg(target_os = "macos")]
#[path = "macos/keychain.rs"]
mod imp;

#[cfg(windows)]
#[path = "windows/keychain.rs"]
mod imp;

#[cfg(not(any(target_os = "macos", windows)))]
mod imp {
    //! Pas de trousseau sur ce système : rien n'est rangé, rien n'est lu.

    use super::KeychainError;

    pub fn save(_service: &str, _account: &str, _secret: &str) -> Result<(), KeychainError> {
        Err(KeychainError::Unavailable)
    }

    pub fn load(_service: &str, _account: &str) -> Result<Option<String>, KeychainError> {
        Err(KeychainError::Unavailable)
    }

    pub fn contains(_service: &str, _account: &str) -> Result<bool, KeychainError> {
        Err(KeychainError::Unavailable)
    }

    pub fn delete(_service: &str, _account: &str) -> Result<bool, KeychainError> {
        Err(KeychainError::Unavailable)
    }
}

/// Ce système a-t-il un trousseau dont Capri sait se servir ?
pub const AVAILABLE: bool = cfg!(any(target_os = "macos", windows));

/// Le nom du trousseau, pour les messages.
pub const NAME: &str = if cfg!(target_os = "macos") {
    "le trousseau de session"
} else if cfg!(windows) {
    "le Gestionnaire d'identification"
} else {
    "le trousseau"
};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeychainError {
    #[error("pas de trousseau sur ce système")]
    Unavailable,
    #[error("le système refuse l'accès au trousseau")]
    Denied,
    #[error("le secret est trop long pour le trousseau")]
    TooLong,
    #[error("ce qui est rangé dans le trousseau n'est pas du texte")]
    NotText,
    #[error("le trousseau répond par une erreur (code {0})")]
    System(i64),
}

/// Range `secret` sous ce service et ce compte, à la place de ce qui y était.
pub fn save(service: &str, account: &str, secret: &str) -> Result<(), KeychainError> {
    imp::save(service, account, secret)
}

/// Le secret rangé sous ce service et ce compte, s'il y en a un.
///
/// Sur macOS, une app qui n'a pas créé l'élément déclenche une demande d'autorisation du système,
/// et cet appel attend la réponse de l'utilisateur.
pub fn load(service: &str, account: &str) -> Result<Option<String>, KeychainError> {
    imp::load(service, account)
}

/// Un secret est-il rangé sous ce service et ce compte ? Ne le lit pas : sur macOS, aucune
/// autorisation n'est demandée.
pub fn contains(service: &str, account: &str) -> Result<bool, KeychainError> {
    imp::contains(service, account)
}

/// Retire le secret rangé sous ce service et ce compte. Rend `false` s'il n'y en avait pas.
pub fn delete(service: &str, account: &str) -> Result<bool, KeychainError> {
    imp::delete(service, account)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touche au vrai trousseau de l'utilisateur, sous un service réservé aux tests, et nettoie
    /// derrière lui. À lancer à la main : `cargo test -p capri-platform -- --ignored`.
    #[test]
    #[ignore = "touche au vrai trousseau du système"]
    fn a_secret_is_saved_read_replaced_and_deleted() {
        let service = format!("Capri (test {})", std::process::id());
        let account = "compte-de-test";
        // si un essai précédent a été interrompu
        let _ = delete(&service, account);

        assert_eq!(contains(&service, account), Ok(false));
        assert_eq!(load(&service, account), Ok(None));
        assert_eq!(delete(&service, account), Ok(false));

        save(&service, account, "premier-secret-de-test").expect("rangé");
        assert_eq!(contains(&service, account), Ok(true));
        assert_eq!(load(&service, account), Ok(Some("premier-secret-de-test".to_owned())));
        assert_eq!(load(&service, "autre-compte"), Ok(None), "un autre compte n'en voit rien");

        save(&service, account, "second-secret-de-test").expect("remplacé");
        assert_eq!(load(&service, account), Ok(Some("second-secret-de-test".to_owned())));

        assert_eq!(delete(&service, account), Ok(true));
        assert_eq!(contains(&service, account), Ok(false));
        assert_eq!(load(&service, account), Ok(None));
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    #[test]
    fn without_a_keychain_every_call_says_so() {
        assert_eq!(save("Capri (test)", "compte", "x"), Err(KeychainError::Unavailable));
        assert_eq!(load("Capri (test)", "compte"), Err(KeychainError::Unavailable));
        assert_eq!(contains("Capri (test)", "compte"), Err(KeychainError::Unavailable));
        assert_eq!(delete("Capri (test)", "compte"), Err(KeychainError::Unavailable));
    }
}
