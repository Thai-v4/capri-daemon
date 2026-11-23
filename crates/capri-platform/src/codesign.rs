//! Signer l'app qu'une mise à jour a refaite, comme l'app installée, et vérifier une signature :
//! `/usr/bin/codesign` par son chemin absolu, avec les arguments de `capri_core::codesign`
//! (toujours `--timestamp=none` : rien n'est envoyé ; jamais de notarisation). Et, **seulement
//! pour une app installée signée par une identité**, la liste des identités de signature valides
//! du trousseau (`/usr/bin/security find-identity -v -p codesigning`), dont seuls les noms et les
//! empreintes sont lus.
//!
//! Ce qu'écrivent `codesign` et `security` n'est jamais recopié, ni dans le journal ni dans un
//! message : leur sortie nomme l'identité, c'est-à-dire une personne. Seule la conclusion sort
//! d'ici (« ad hoc », une identité masquée, une signature valide ou non).
//!
//! Ailleurs que sur macOS : [`CodesignError::Unavailable`].

use std::path::Path;

use capri_core::codesign::{IdentityHash, IdentityName, Signature, SignatureError, Signer};

/// Ce système sait-il signer une app ?
pub const AVAILABLE: bool = cfg!(target_os = "macos");

#[derive(Debug, thiserror::Error)]
pub enum CodesignError {
    #[error("elle ne sait pas signer une app sur ce système")]
    Unavailable,
    #[error("{program} ne se lance pas : {source}")]
    Io { program: &'static str, source: std::io::Error },
    /// Le programme a refusé (sa sortie n'est pas recopiée : elle nommerait l'identité).
    #[error("{program} a refusé ({status})")]
    Refused { program: &'static str, status: std::process::ExitStatus },
    #[error("la signature de l'app n'est pas lisible")]
    Unreadable(SignatureError),
}

/// Comment l'app `app` est signée (`codesign -d --verbose=2`, lu par
/// `capri_core::codesign::signature`).
pub fn describe(app: &Path) -> Result<Signature, CodesignError> {
    #[cfg(target_os = "macos")]
    {
        use capri_core::codesign::{CODESIGN, describe_args, signature};
        let output = run(CODESIGN, &describe_args(app), true)?;
        // la description est sur la sortie d'erreur, que le code de sortie soit nul ou non (une
        // app non signée : 1, et « code object is not signed at all »)
        let text = String::from_utf8_lossy(&output.stderr);
        signature(&text).map_err(CodesignError::Unreadable)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err(CodesignError::Unavailable)
    }
}

/// Les identités de signature valides du trousseau : leurs empreintes et leurs noms, rien d'autre.
pub fn identities() -> Result<Vec<(IdentityHash, IdentityName)>, CodesignError> {
    #[cfg(target_os = "macos")]
    {
        use capri_core::codesign::{FIND_IDENTITY, SECURITY, identities};
        let args: Vec<std::ffi::OsString> = FIND_IDENTITY.iter().map(Into::into).collect();
        let output = run(SECURITY, &args, false)?;
        if !output.status.success() {
            return Err(CodesignError::Refused { program: SECURITY, status: output.status });
        }
        Ok(identities(&String::from_utf8_lossy(&output.stdout)))
    }
    #[cfg(not(target_os = "macos"))]
    Err(CodesignError::Unavailable)
}

/// Signe `path` (un programme, ou l'app entière) avec `signer`. Si c'est une identité, macOS
/// peut demander à l'utilisateur d'autoriser `codesign` à se servir de sa clé.
pub fn sign(path: &Path, signer: &Signer) -> Result<(), CodesignError> {
    #[cfg(target_os = "macos")]
    {
        use capri_core::codesign::{CODESIGN, sign_args};
        let output = run(CODESIGN, &sign_args(signer, path), true)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(CodesignError::Refused { program: CODESIGN, status: output.status })
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (path, signer);
        Err(CodesignError::Unavailable)
    }
}

/// La signature de `path` se vérifie-t-elle, strictement (`codesign --verify --strict`) ?
pub fn verify(path: &Path) -> Result<(), CodesignError> {
    #[cfg(target_os = "macos")]
    {
        use capri_core::codesign::{CODESIGN, verify_args};
        let output = run(CODESIGN, &verify_args(path), true)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(CodesignError::Refused { program: CODESIGN, status: output.status })
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err(CodesignError::Unavailable)
    }
}

/// Lance `program` (un chemin absolu), sans entrée, et attend sa fin. Sa sortie d'erreur n'est
/// gardée que si `stderr` (celle de `codesign -d`, à lire) ; sinon elle est jetée.
#[cfg(target_os = "macos")]
fn run(
    program: &'static str,
    args: &[std::ffi::OsString],
    stderr: bool,
) -> Result<std::process::Output, CodesignError> {
    use std::process::{Command, Stdio};
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(if stderr { Stdio::piped() } else { Stdio::null() })
        .output()
        .map_err(|source| CodesignError::Io { program, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn elsewhere_nothing_is_signed() {
        let app = Path::new("/tmp/Capri.app");
        assert!(matches!(describe(app), Err(CodesignError::Unavailable)));
        assert!(matches!(identities(), Err(CodesignError::Unavailable)));
        assert!(matches!(sign(app, &Signer::AdHoc), Err(CodesignError::Unavailable)));
        assert!(matches!(verify(app), Err(CodesignError::Unavailable)));
    }

    /// Le vrai `codesign`, en « ad hoc » seulement (le trousseau n'est pas lu), sur une petite app
    /// d'essai dans un dossier temporaire : signée, vérifiée, décrite ; une ressource changée
    /// après la signature, et la vérification échoue.
    #[cfg(target_os = "macos")]
    #[test]
    fn an_ad_hoc_signature_is_made_verified_and_read() {
        let directory = std::env::temp_dir().join(format!("capri-test-signature-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        // un nom qui ne finit pas par .app, comme la nouvelle app d'une mise à jour
        let app = directory.join("next");
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::create_dir_all(app.join("Contents/Resources")).unwrap();
        let info = include_str!("../../../tools/macos/Info.plist").replace("@VERSION@", "9.0.0");
        std::fs::write(app.join("Contents/Info.plist"), info).unwrap();
        std::fs::write(app.join("Contents/Resources/CREDITS.md"), "essai\n").unwrap();
        // un programme qui n'est signé par personne : « not signed at all »
        std::fs::write(app.join("Contents/MacOS/capri"), "#!/bin/sh\n").unwrap();
        assert!(matches!(describe(&app), Err(CodesignError::Unreadable(SignatureError::Unsigned))));
        assert!(verify(&app).is_err(), "pas signée");
        // des programmes du système, que l'on signe de nouveau, « ad hoc »
        std::fs::remove_file(app.join("Contents/MacOS/capri")).unwrap();
        std::fs::copy("/usr/bin/true", app.join("Contents/MacOS/capri")).unwrap();
        std::fs::copy("/usr/bin/true", app.join("Contents/MacOS/capri-hook")).unwrap();
        assert!(verify(&app).is_err(), "pas encore signée en entier");
        sign(&app.join("Contents/MacOS/capri-hook"), &Signer::AdHoc).expect("le second programme");
        sign(&app, &Signer::AdHoc).expect("l'app");
        verify(&app).expect("signature valide");
        assert_eq!(describe(&app).expect("décrite"), Signature::AdHoc);
        // une ressource changée après coup : la signature ne se vérifie plus
        std::fs::write(app.join("Contents/Resources/CREDITS.md"), "changé\n").unwrap();
        assert!(matches!(verify(&app), Err(CodesignError::Refused { .. })));
        let _ = std::fs::remove_dir_all(&directory);
    }
}
