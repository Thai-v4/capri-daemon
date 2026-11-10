//! Windows : le Gestionnaire d'identification, par les fonctions `Cred…W` d'advapi32.
//!
//! Des identifiants génériques (`CRED_TYPE_GENERIC`), nommés `<service>:<compte>`, gardés pour
//! cette session d'utilisateur sur cette machine (`CRED_PERSIST_LOCAL_MACHINE` : ils ne suivent pas
//! un profil itinérant). Le secret est rangé tel quel, en UTF-8.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_NO_SUCH_LOGON_SESSION, ERROR_NOT_FOUND, GetLastError, WIN32_ERROR,
};
use windows_sys::Win32::Security::Credentials::{
    CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW,
    CredFree, CredReadW, CredWriteW,
};

use super::KeychainError;

/// Une chaîne pour Windows : UTF-16, terminée par un zéro.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn target(service: &str, account: &str) -> Vec<u16> {
    wide(&format!("{service}:{account}"))
}

fn failure(code: WIN32_ERROR) -> KeychainError {
    match code {
        ERROR_ACCESS_DENIED => KeychainError::Denied,
        ERROR_NO_SUCH_LOGON_SESSION => KeychainError::Unavailable,
        code => KeychainError::System(code.into()),
    }
}

fn last_error() -> WIN32_ERROR {
    // SAFETY: lit l'erreur du dernier appel de ce thread ; aucune condition.
    unsafe { GetLastError() }
}

/// Un identifiant rendu par `CredReadW`, libéré par `CredFree` quoi qu'il arrive.
struct Read(*mut CREDENTIALW);

impl Drop for Read {
    fn drop(&mut self) {
        // SAFETY: le pointeur vient d'un `CredReadW` réussi, et n'est libéré qu'ici, une fois.
        unsafe { CredFree(self.0 as *const c_void) };
    }
}

/// Lit l'identifiant nommé `target`. `Ok(None)` : il n'existe pas.
fn read(target: &[u16]) -> Result<Option<Read>, KeychainError> {
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `target` est une chaîne UTF-16 terminée par un zéro, qui vit le temps de l'appel ;
    // `credential` reçoit, en cas de succès, une structure allouée par le système, que `Read`
    // libère.
    let ok = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) };
    if ok == 0 {
        return match last_error() {
            ERROR_NOT_FOUND => Ok(None),
            code => Err(failure(code)),
        };
    }
    Ok(Some(Read(credential)))
}

pub fn save(service: &str, account: &str, secret: &str) -> Result<(), KeychainError> {
    let mut blob = secret.as_bytes().to_vec();
    if blob.len() > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
        return Err(KeychainError::TooLong);
    }
    let (mut target, mut user) = (target(service, account), wide(account));
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: user.as_mut_ptr(),
        ..CREDENTIALW::default()
    };
    // SAFETY: toutes les chaînes et le secret pointés par `credential` vivent jusqu'à la fin de la
    // fonction ; `CredWriteW` les copie et n'en garde aucun pointeur. Les champs laissés à zéro
    // (commentaire, attributs, alias) sont permis.
    let ok = unsafe { CredWriteW(&credential, 0) };
    if ok == 0 {
        return Err(failure(last_error()));
    }
    Ok(())
}

pub fn load(service: &str, account: &str) -> Result<Option<String>, KeychainError> {
    let Some(read) = read(&target(service, account))? else { return Ok(None) };
    // SAFETY: `read.0` pointe sur la structure rendue par un `CredReadW` réussi, encore vivante
    // (elle n'est libérée qu'avec `read`) ; son secret fait `CredentialBlobSize` octets.
    let bytes = unsafe {
        let credential = &*read.0;
        if credential.CredentialBlob.is_null() || credential.CredentialBlobSize == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(credential.CredentialBlob, credential.CredentialBlobSize as usize)
                .to_vec()
        }
    };
    drop(read);
    String::from_utf8(bytes).map(Some).map_err(|_| KeychainError::NotText)
}

pub fn contains(service: &str, account: &str) -> Result<bool, KeychainError> {
    Ok(read(&target(service, account))?.is_some())
}

pub fn delete(service: &str, account: &str) -> Result<bool, KeychainError> {
    let target = target(service, account);
    // SAFETY: `target` est une chaîne UTF-16 terminée par un zéro, qui vit le temps de l'appel.
    let ok = unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) };
    if ok == 0 {
        return match last_error() {
            ERROR_NOT_FOUND => Ok(false),
            code => Err(failure(code)),
        };
    }
    Ok(true)
}
