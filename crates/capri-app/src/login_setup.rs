//! La case « Se lancer à l'ouverture de session », côté app : son état, lu dans le système (notre
//! agent de lancement est-il là ?), la bascule, et la réparation au lancement.
//!
//! Le texte de l'agent et la façon de reconnaître le nôtre sont dans `capri_core::login` ; l'écrire
//! et le retirer, dans `capri_platform::login`. Ici, on décide quelle app inscrire : celle qui
//! tourne, si elle tourne depuis une app (`Capri.app`). Lancée hors d'une app (le binaire de
//! `cargo run`), Capri n'inscrit rien : ses permissions (le micro) ne seraient pas celles de l'app.
//! Aucun fichier de réglages à part : c'est l'agent lui-même qui dit si la case est cochée.

use std::path::Path;

#[cfg(any(target_os = "macos", windows, test))]
use capri_platform::login::LoginError;
use capri_platform::login::{self, LoginState};

/// L'app d'où tourne Capri (`/…/Capri.app`), si elle tourne depuis une app.
pub fn this_app() -> Option<String> {
    let executable = std::env::current_exe().ok()?;
    let executable = executable.canonicalize().unwrap_or(executable);
    capri_core::login::app_bundle(&executable)?.to_str().map(str::to_owned)
}

/// Au lancement : Capri est-elle inscrite ? Si c'est le cas et que notre agent ne mène plus à une
/// app qui existe (elle a changé de place), il est réparé pour celle qui tourne ; s'il est d'une
/// autre version, il est réécrit. Rien n'est jamais créé ici : seulement réparé.
pub fn at_launch(directory: &Path, this_app: Option<&str>) -> bool {
    let state = match login::status(directory) {
        Ok(state) => state,
        Err(err) => {
            log::warn!("inscription à l'ouverture de session illisible : {err}");
            return false;
        }
    };
    let registered_exists =
        matches!(&state, LoginState::Ours { app: Some(app), .. } if Path::new(app).exists());
    if let Some(app) = capri_core::login::repair(&state, this_app, registered_exists) {
        match login::register(directory, &app) {
            Ok(done) => log::info!("inscription à l'ouverture de session réparée ({done:?}) : {app}"),
            Err(err) => log::warn!("inscription à l'ouverture de session pas réparée : {err}"),
        }
    }
    if state == LoginState::Foreign {
        log::warn!("{} n'est pas de Capri : elle n'y touche pas", login::agent_file(directory).display());
    }
    state.is_registered()
}

/// Ce qu'a fait la bascule (la case n'existe que dans les menus de macOS et de Windows).
#[cfg(any(target_os = "macos", windows, test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggled {
    /// Inscrite (ou déjà inscrite).
    Registered,
    /// Plus inscrite.
    Unregistered,
}

/// Pourquoi la bascule n'a rien fait.
#[cfg(any(target_os = "macos", windows, test))]
#[derive(Debug)]
pub enum ToggleError {
    /// Lancée hors d'une app : elle n'inscrit pas un binaire nu.
    NotFromApp,
    Login(LoginError),
}

/// Coche la case si elle ne l'est pas (inscrit `this_app`), la décoche sinon (retire notre agent,
/// et rien d'autre).
#[cfg(any(target_os = "macos", windows, test))]
pub fn toggle(directory: &Path, this_app: Option<&str>) -> Result<Toggled, ToggleError> {
    let state = login::status(directory).map_err(ToggleError::Login)?;
    if state.is_registered() {
        let done = login::unregister(directory).map_err(ToggleError::Login)?;
        log::info!("inscription à l'ouverture de session retirée ({done:?})");
        return Ok(Toggled::Unregistered);
    }
    let app = this_app.ok_or(ToggleError::NotFromApp)?;
    let done = login::register(directory, app).map_err(ToggleError::Login)?;
    log::info!("inscrite à l'ouverture de session ({done:?}) : {app}");
    Ok(Toggled::Registered)
}

/// Ce qu'elle dit d'un refus, dans son bandeau.
#[cfg(any(target_os = "macos", windows, test))]
pub fn refusal(error: &ToggleError) -> &'static str {
    use capri_chat::startup::say;
    match error {
        ToggleError::NotFromApp => say::ONLY_FROM_APP,
        ToggleError::Login(LoginError::Unavailable) => say::UNAVAILABLE,
        ToggleError::Login(LoginError::Foreign(_)) => say::NOT_OURS,
        ToggleError::Login(_) => say::FAILED,
    }
}

#[cfg(any(target_os = "macos", windows, test))]
impl std::fmt::Display for ToggleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFromApp => f.write_str("Capri ne tourne pas depuis une app : rien n'est inscrit"),
            Self::Login(err) => err.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Un dossier d'agents propre à ce test : jamais le vrai `~/Library/LaunchAgents`.
    fn scratch(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("capri-test-inscription-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        base.join("LaunchAgents")
    }

    /// Une fausse app, juste ce qu'il faut pour exister.
    ///
    /// Sous Unix seulement, comme les trois essais qui s'en servent : un agent de lancement nomme
    /// l'app par un chemin qui part de `/`, et sous Windows (où ces essais tournent aussi, dans la
    /// CI) un vrai dossier porte une lettre de lecteur, que `capri_core::login::agent_text` refuse.
    /// Ce refus-là est essayé dans `capri-core` et `capri-platform`, avec des chemins écrits.
    #[cfg(unix)]
    fn fake_app(base: &Path, name: &str) -> String {
        let app = base.join(name);
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        app.to_str().unwrap().to_owned()
    }

    #[cfg(unix)]
    #[test]
    fn the_box_registers_the_running_app_and_unchecking_removes_only_our_agent() {
        let directory = scratch("bascule");
        let base = directory.parent().unwrap().to_owned();
        let app = fake_app(&base, "Capri.app");
        assert!(!at_launch(&directory, Some(&app)), "décochée par défaut : rien n'est créé");
        assert!(!directory.exists());

        assert!(matches!(toggle(&directory, Some(&app)), Ok(Toggled::Registered)));
        assert!(at_launch(&directory, Some(&app)));
        assert_eq!(
            login::status(&directory).unwrap(),
            LoginState::Ours { app: Some(app.clone()), current: true }
        );
        std::fs::write(directory.join("com.exemple.autre.plist"), "à quelqu'un d'autre").unwrap();
        assert!(matches!(toggle(&directory, Some(&app)), Ok(Toggled::Unregistered)));
        assert!(!at_launch(&directory, Some(&app)));
        let left: Vec<_> = std::fs::read_dir(&directory).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, ["com.exemple.autre.plist"], "rien d'autre n'est touché");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_bare_program_registers_nothing_and_says_why() {
        let directory = scratch("binaire");
        let refused = toggle(&directory, None).unwrap_err();
        assert!(matches!(refused, ToggleError::NotFromApp));
        assert_eq!(refusal(&refused), capri_chat::startup::say::ONLY_FROM_APP);
        assert!(!directory.exists());
        let _ = std::fs::remove_dir_all(directory.parent().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn a_moved_app_is_repaired_at_the_next_launch() {
        let directory = scratch("deplacee");
        let base = directory.parent().unwrap().to_owned();
        let before = fake_app(&base, "avant/Capri.app");
        toggle(&directory, Some(&before)).unwrap();
        // l'app a changé de place : l'ancienne n'existe plus
        std::fs::remove_dir_all(base.join("avant")).unwrap();
        let after = fake_app(&base, "après/Capri.app");
        assert!(at_launch(&directory, Some(&after)));
        assert_eq!(
            login::status(&directory).unwrap(),
            LoginState::Ours { app: Some(after.clone()), current: true }
        );
        // une seconde copie, alors que la première existe encore : l'inscription ne change pas
        let copy = fake_app(&base, "copie/Capri.app");
        assert!(at_launch(&directory, Some(&copy)));
        assert_eq!(login::status(&directory).unwrap(), LoginState::Ours { app: Some(after), current: true });
        // lancée hors d'une app, l'app inscrite disparue : rien n'est réécrit, la case reste cochée
        std::fs::remove_dir_all(base.join("après")).unwrap();
        let text = std::fs::read(login::agent_file(&directory)).unwrap();
        assert!(at_launch(&directory, None));
        assert_eq!(std::fs::read(login::agent_file(&directory)).unwrap(), text);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_is_not_ours_leaves_the_box_unchecked_and_untouched() {
        let directory = scratch("etranger");
        let base = directory.parent().unwrap().to_owned();
        std::fs::create_dir_all(&directory).unwrap();
        let file = login::agent_file(&directory);
        std::fs::write(&file, "<plist>le mien</plist>").unwrap();
        let app = fake_app(&base, "Capri.app");
        assert!(!at_launch(&directory, Some(&app)));
        let refused = toggle(&directory, Some(&app)).unwrap_err();
        assert_eq!(refusal(&refused), capri_chat::startup::say::NOT_OURS);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "<plist>le mien</plist>");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_program_outside_an_app_is_not_an_app() {
        // les essais tournent hors d'une app : rien à inscrire
        assert_eq!(this_app(), None);
    }
}
