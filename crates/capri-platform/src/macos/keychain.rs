//! macOS : le trousseau de session, par les fonctions `SecItem…` de Security.framework.
//!
//! Des mots de passe génériques (`kSecClassGenericPassword`), désignés par un service et un compte.
//! Le crate `security-framework` enveloppe ces appels sans `unsafe` de notre côté.

use security_framework::base::Error;
use security_framework::item::{ItemClass, ItemSearchOptions};
use security_framework::passwords::{
    PasswordOptions, delete_generic_password, generic_password, set_generic_password,
};

use super::KeychainError;

/// `errSecItemNotFound` : rien sous ce service et ce compte.
const ITEM_NOT_FOUND: i32 = -25300;
/// `errSecAuthFailed` : l'utilisateur a refusé, ou s'est trompé de mot de passe.
const AUTH_FAILED: i32 = -25293;
/// `errSecUserCanceled` : l'utilisateur a fermé la demande d'autorisation.
const USER_CANCELED: i32 = -128;
/// `errSecInteractionNotAllowed` : il faudrait demander, et ce n'est pas possible maintenant.
const INTERACTION_NOT_ALLOWED: i32 = -25308;

fn failure(error: Error) -> KeychainError {
    match error.code() {
        AUTH_FAILED | USER_CANCELED | INTERACTION_NOT_ALLOWED => KeychainError::Denied,
        code => KeychainError::System(code.into()),
    }
}

pub fn save(service: &str, account: &str, secret: &str) -> Result<(), KeychainError> {
    // crée l'élément, ou remplace son contenu s'il existe déjà
    set_generic_password(service, account, secret.as_bytes()).map_err(failure)
}

pub fn load(service: &str, account: &str) -> Result<Option<String>, KeychainError> {
    match generic_password(PasswordOptions::new_generic_password(service, account)) {
        Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|_| KeychainError::NotText),
        Err(error) if error.code() == ITEM_NOT_FOUND => Ok(None),
        Err(error) => Err(failure(error)),
    }
}

pub fn contains(service: &str, account: &str) -> Result<bool, KeychainError> {
    // les attributs seulement : le secret n'est pas lu, et le système ne demande rien
    let found = ItemSearchOptions::new()
        .class(ItemClass::generic_password())
        .service(service)
        .account(account)
        .load_attributes(true)
        .limit(1)
        .search();
    match found {
        Ok(items) => Ok(!items.is_empty()),
        Err(error) if error.code() == ITEM_NOT_FOUND => Ok(false),
        Err(error) => Err(failure(error)),
    }
}

pub fn delete(service: &str, account: &str) -> Result<bool, KeychainError> {
    match delete_generic_password(service, account) {
        Ok(()) => Ok(true),
        Err(error) if error.code() == ITEM_NOT_FOUND => Ok(false),
        Err(error) => Err(failure(error)),
    }
}
