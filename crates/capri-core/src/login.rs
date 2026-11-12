//! Se lancer à l'ouverture de session : le texte de son inscription, et comment reconnaître la
//! nôtre.
//!
//! Sur macOS, l'inscription est un **agent de lancement** de l'utilisateur, un fichier
//! `launchd.plist(5)` rangé dans `~/Library/LaunchAgents`, que `launchd` charge à chaque ouverture
//! de session. Le nôtre ne lance qu'une chose : `/usr/bin/open -g -a <Capri.app>`, c'est-à-dire
//! l'app, comme un double-clic (ses permissions, le micro par exemple, restent celles de l'app),
//! sans la mettre au premier plan. Il est lié à un chemin : il survit aux reconstructions d'une app
//! signée « ad hoc », et une app déplacée se répare au lancement suivant ([`repair`]).
//!
//! Les clés viennent de `launchd.plist(5)` (relu le 12 novembre 2026, sur macOS 27.2) :
//! - `Label`, obligatoire, et le fichier nommé `<Label>.plist` comme le veut la convention ;
//! - `ProgramArguments` : le premier élément est un chemin absolu ;
//! - `RunAtLoad` : lancé une fois, quand l'agent est chargé, donc à l'ouverture de session ;
//! - `LimitLoadToSessionType` = `Aqua` : seulement dans une session graphique (pas en SSH) ;
//! - `AssociatedBundleIdentifiers` : « If an app installs a legacy plist the plist should include
//!   this key with a value of the app's bundle identifier », pour que Réglages Système l'affiche
//!   sous le nom de l'app parmi les éléments d'ouverture.
//!
//! Pas de `KeepAlive` : `open` rend la main aussitôt, et l'app lancée par LaunchServices vit dans
//! sa propre tâche ; quitter Capri la quitte pour de bon, jusqu'à la session suivante.
//!
//! Ce module ne lit ni n'écrit aucun fichier : il dit quoi écrire et, d'après le contenu d'un
//! fichier déjà là, si c'est le nôtre. Un fichier est le nôtre s'il porte, avant son élément
//! `<plist>`, le commentaire qui commence par [`MARK`] ; on n'écrase ni ne retire jamais un autre
//! fichier. `capri-platform` l'écrit, d'un seul coup.

use std::path::Path;

/// L'étiquette de l'agent, propre à Capri. À ne jamais changer : une inscription faite par une
/// version précédente ne serait plus reconnue, ni donc retirée.
pub const LABEL: &str = "local.capri-daemon.capri.login";
/// Le fichier de l'agent : `<Label>.plist`.
pub const FILE_NAME: &str = "local.capri-daemon.capri.login.plist";
/// L'identifiant de bundle de `Capri.app` (`tools/macos/Info.plist`).
pub const BUNDLE_ID: &str = "local.capri-daemon.capri";
/// Le début du commentaire qui signe notre fichier.
pub const MARK: &str = "<!-- capri-managed";

/// Le fichier, avec `{APP}` à la place du chemin de l'app.
const TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<!-- capri-managed : écrit par Capri (« Se lancer à l'ouverture de session », dans son menu), qui le retire quand on décoche la case. -->
<plist version=\"1.0\">
<dict>
\t<key>Label</key>
\t<string>local.capri-daemon.capri.login</string>
\t<key>AssociatedBundleIdentifiers</key>
\t<string>local.capri-daemon.capri</string>
\t<key>ProgramArguments</key>
\t<array>
\t\t<string>/usr/bin/open</string>
\t\t<string>-g</string>
\t\t<string>-a</string>
\t\t<string>{APP}</string>
\t</array>
\t<key>RunAtLoad</key>
\t<true/>
\t<key>LimitLoadToSessionType</key>
\t<string>Aqua</string>
</dict>
</plist>
";

/// La ligne qui précède celle du chemin de l'app.
const BEFORE_APP: &str = "<string>-a</string>";

/// Pourquoi un chemin ne peut pas être inscrit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentError {
    /// Ce n'est pas le chemin absolu d'une app (`/…/Capri.app`).
    NotAnApp,
    /// Il contient un caractère de contrôle, qu'un fichier XML ne peut pas porter.
    Unprintable,
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotAnApp => "ce n'est pas le chemin absolu d'une app",
            Self::Unprintable => "le chemin de l'app contient un caractère de contrôle",
        })
    }
}

impl std::error::Error for AgentError {}

/// Le fichier de l'agent qui lance l'app `app` (son chemin absolu, `/…/Capri.app`).
pub fn agent_text(app: &str) -> Result<String, AgentError> {
    if !app.starts_with('/') || !app.ends_with(".app") {
        return Err(AgentError::NotAnApp);
    }
    if app.chars().any(char::is_control) {
        return Err(AgentError::Unprintable);
    }
    Ok(TEMPLATE.replace("{APP}", &escape(app)))
}

/// Ce qu'on trouve à la place du fichier de l'agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginState {
    /// Rien : Capri n'est pas inscrite.
    Absent,
    /// Le nôtre. `app` : l'app qu'il lance, s'il se lit ; `current` : tel que cette version de
    /// Capri l'écrit pour cette app.
    Ours { app: Option<String>, current: bool },
    /// Un fichier qui n'est pas de Capri : on n'y touche jamais.
    Foreign,
}

impl LoginState {
    /// Ce que dit un fichier déjà là (`None` : il n'y en a pas). Un contenu qui n'est pas de
    /// l'UTF-8 n'est pas le nôtre.
    pub fn of(existing: Option<&[u8]>) -> Self {
        let Some(bytes) = existing else { return Self::Absent };
        match std::str::from_utf8(bytes) {
            Ok(text) if is_ours(text) => {
                let app = registered_app(text);
                let current =
                    app.as_deref().and_then(|app| agent_text(app).ok()).is_some_and(|ours| ours == text);
                Self::Ours { app, current }
            }
            _ => Self::Foreign,
        }
    }

    /// Capri est-elle inscrite ? C'est ce que montre la case de son menu.
    pub fn is_registered(&self) -> bool {
        matches!(self, Self::Ours { .. })
    }
}

/// Ce texte est-il un agent écrit par Capri ? Il faut le commentaire [`MARK`] en tête d'une ligne
/// avant l'élément `<plist>` : ailleurs, ce pourrait être le commentaire de quelqu'un d'autre.
pub fn is_ours(text: &str) -> bool {
    text.lines()
        .take_while(|line| !line.trim_start().starts_with("<plist"))
        .any(|line| line.trim_start().starts_with(MARK))
}

/// L'app que lance notre agent, lue à sa place dans le texte que Capri écrit. `None` si le
/// fichier a été changé au point de ne plus la dire là.
pub fn registered_app(text: &str) -> Option<String> {
    let mut lines = text.lines().map(str::trim);
    lines.by_ref().find(|line| *line == BEFORE_APP)?;
    let app = lines.next()?.strip_prefix("<string>")?.strip_suffix("</string>")?;
    let app = unescape(app)?;
    agent_text(&app).is_ok().then_some(app)
}

/// Faut-il réécrire notre agent, et pour quelle app ? Au lancement, case cochée :
/// - l'app inscrite n'existe plus (`registered_exists` faux : déplacée, ou retirée) : on inscrit
///   celle qui tourne, si elle tourne depuis une app (`this_app`) ;
/// - le fichier ne dit plus quelle app lancer : de même ;
/// - le fichier est d'une autre version de Capri : réécrit, pour la même app.
///
/// Une app inscrite qui existe encore reste inscrite, même si c'est une autre copie qui tourne :
/// on ne change de copie qu'en décochant puis recochant la case depuis celle qu'on veut. Lancée
/// hors d'une app (le binaire de `cargo run`), Capri n'inscrit jamais le binaire.
pub fn repair(state: &LoginState, this_app: Option<&str>, registered_exists: bool) -> Option<String> {
    let LoginState::Ours { app, current } = state else { return None };
    let target = match app {
        Some(app) if registered_exists => app.clone(),
        _ => this_app?.to_owned(),
    };
    (app.as_deref() != Some(target.as_str()) || !current).then_some(target)
}

/// L'app qui contient ce programme, s'il vit dans une app : `/…/Capri.app` pour
/// `/…/Capri.app/Contents/MacOS/capri`. `None` pour un binaire nu (`target/release/capri`).
pub fn app_bundle(executable: &Path) -> Option<&Path> {
    let macos = executable.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    let named = |path: &Path, name: &str| path.file_name().is_some_and(|found| found == name);
    let is_app = app.extension().is_some_and(|extension| extension == "app");
    (named(macos, "MacOS") && named(contents, "Contents") && is_app).then_some(app)
}

/// Le texte d'un élément XML : `&`, `<` et `>` échappés.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// L'inverse de [`escape`], avec les deux autres entités de XML. Une entité inconnue : `None`.
fn unescape(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let (entity, after) = rest[at..].split_once(';')?;
        out.push(match entity {
            "&amp" => '&',
            "&lt" => '<',
            "&gt" => '>',
            "&quot" => '"',
            "&apos" => '\'',
            _ => return None,
        });
        rest = after;
    }
    out.push_str(rest);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "/Users/moi/Github/capri-daemon/target/Capri.app";

    #[test]
    fn the_agent_opens_the_app_at_login_in_a_graphical_session_and_nothing_else() {
        let text = agent_text(APP).expect("une app");
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist"));
        assert!(text.ends_with("</dict>\n</plist>\n"));
        // l'étiquette et le nom du fichier vont ensemble
        assert_eq!(FILE_NAME, format!("{LABEL}.plist"));
        assert!(text.contains(&format!("<key>Label</key>\n\t<string>{LABEL}</string>")));
        assert!(
            text.contains(&format!("<key>AssociatedBundleIdentifiers</key>\n\t<string>{BUNDLE_ID}</string>"))
        );
        // l'app, par open : comme un double-clic, sans la mettre au premier plan
        assert!(text.contains(&format!(
            "<key>ProgramArguments</key>\n\t<array>\n\t\t<string>/usr/bin/open</string>\n\t\t<string>-g</string>\n\t\t\
             <string>-a</string>\n\t\t<string>{APP}</string>\n\t</array>"
        )));
        assert!(text.contains("<key>RunAtLoad</key>\n\t<true/>"));
        assert!(text.contains("<key>LimitLoadToSessionType</key>\n\t<string>Aqua</string>"));
        // rien qui la relance si on la quitte, ni rien d'autre à lancer
        for absent in ["KeepAlive", "<key>Program</key>", "ollama", "claude", "EnvironmentVariables"] {
            assert!(!text.contains(absent), "{absent}");
        }
        assert_eq!(text.matches("<string>").count(), 7);
        // un commentaire XML ne peut pas contenir « -- » ailleurs qu'à sa fin
        let comment = text.lines().find(|line| line.starts_with(MARK)).expect("signé");
        assert!(!comment[4..comment.len() - 3].contains("--"), "{comment}");
    }

    #[test]
    fn only_the_absolute_path_of_an_app_is_registered() {
        for refused in [
            "",
            "Capri.app",
            "target/Capri.app",
            "/usr/local/bin/capri",
            "/Applications/Capri.app/",
            "/x/Capri.apps",
        ] {
            assert_eq!(agent_text(refused), Err(AgentError::NotAnApp), "{refused:?}");
        }
        assert_eq!(agent_text("/x/Ca\npri.app"), Err(AgentError::Unprintable));
        assert_eq!(agent_text("/x/Ca\u{7}pri.app"), Err(AgentError::Unprintable));
        assert!(agent_text("/Applications/Capri.app").is_ok());
    }

    #[test]
    fn an_unusual_path_survives_being_written_and_read_back() {
        for app in
            [APP, "/Volumes/Mon disque/R&D <essai>/Capri.app", "/Users/élodie/Applications/Capri “bis”.app"]
        {
            let text = agent_text(app).expect("une app");
            assert_eq!(registered_app(&text).as_deref(), Some(app), "{text}");
            assert_eq!(
                LoginState::of(Some(text.as_bytes())),
                LoginState::Ours { app: Some(app.to_owned()), current: true }
            );
        }
        let escaped = agent_text("/a/R&D <x>.app").expect("une app");
        assert!(escaped.contains("<string>/a/R&amp;D &lt;x&gt;.app</string>"), "{escaped}");
        assert_eq!(unescape("a&quot;b&apos;c"), Some("a\"b'c".to_owned()));
        assert_eq!(unescape("a&inconnue;b"), None);
        assert_eq!(unescape("a&b"), None);
    }

    #[test]
    fn our_file_is_recognized_by_its_mark_and_nothing_else() {
        assert_eq!(LoginState::of(None), LoginState::Absent);
        assert!(!LoginState::Absent.is_registered());
        let ours = agent_text(APP).expect("une app");
        assert!(LoginState::of(Some(ours.as_bytes())).is_registered());

        // une autre version du nôtre : reconnue, à réécrire
        let older = ours.replace("\t<key>LimitLoadToSessionType</key>\n\t<string>Aqua</string>\n", "");
        assert_eq!(
            LoginState::of(Some(older.as_bytes())),
            LoginState::Ours { app: Some(APP.to_owned()), current: false }
        );
        // le nôtre, touché au point de ne plus dire quelle app lancer
        let broken = ours.replace("<string>-a</string>", "<string>-b</string>");
        assert_eq!(LoginState::of(Some(broken.as_bytes())), LoginState::Ours { app: None, current: false });
        let crlf = ours.replace('\n', "\r\n");
        assert!(LoginState::of(Some(crlf.as_bytes())).is_registered());

        // un agent de quelqu'un d'autre, même sous notre nom de fichier
        let theirs =
            ours.lines().filter(|line| !line.starts_with(MARK)).map(|l| format!("{l}\n")).collect::<String>();
        assert_eq!(LoginState::of(Some(theirs.as_bytes())), LoginState::Foreign);
        // la marque après <plist> : un commentaire dans le dictionnaire, pas une signature
        let inside = theirs.replace("<dict>\n", &format!("<dict>\n{MARK} -->\n"));
        assert_eq!(LoginState::of(Some(inside.as_bytes())), LoginState::Foreign);
        assert_eq!(LoginState::of(Some(b"")), LoginState::Foreign);
        assert_eq!(LoginState::of(Some(&[0xff, 0xfe, b'<'])), LoginState::Foreign);
        // un plist binaire n'est pas le nôtre
        assert_eq!(LoginState::of(Some(b"bplist00\xd1\x01\x02")), LoginState::Foreign);
    }

    #[test]
    fn a_moved_app_is_repaired_and_a_second_copy_does_not_steal_the_registration() {
        let ours = |app: &str| LoginState::Ours { app: Some(app.to_owned()), current: true };
        let other = "/Applications/Capri.app";
        // rien à faire : la bonne app, à jour
        assert_eq!(repair(&ours(APP), Some(APP), true), None);
        // l'app inscrite a disparu : celle qui tourne prend sa place
        assert_eq!(repair(&ours(other), Some(APP), false).as_deref(), Some(APP));
        // une autre copie qui existe encore reste inscrite
        assert_eq!(repair(&ours(other), Some(APP), true), None);
        // hors d'une app, on ne répare qu'en gardant l'app inscrite
        assert_eq!(repair(&ours(other), None, false), None);
        let older = LoginState::Ours { app: Some(other.to_owned()), current: false };
        assert_eq!(repair(&older, None, true).as_deref(), Some(other));
        assert_eq!(repair(&older, Some(APP), true).as_deref(), Some(other));
        // le fichier ne dit plus quelle app : celle qui tourne
        let lost = LoginState::Ours { app: None, current: false };
        assert_eq!(repair(&lost, Some(APP), false).as_deref(), Some(APP));
        assert_eq!(repair(&lost, None, false), None);
        // jamais rien sans notre fichier
        assert_eq!(repair(&LoginState::Absent, Some(APP), false), None);
        assert_eq!(repair(&LoginState::Foreign, Some(APP), false), None);
    }

    #[test]
    fn the_app_is_found_around_its_program() {
        let app = |exe: &str| app_bundle(Path::new(exe)).map(|path| path.to_string_lossy().into_owned());
        assert_eq!(app(&format!("{APP}/Contents/MacOS/capri")).as_deref(), Some(APP));
        assert_eq!(
            app("/Applications/Capri.app/Contents/MacOS/capri").as_deref(),
            Some("/Applications/Capri.app")
        );
        for bare in [
            "/Users/moi/Github/capri-daemon/target/release/capri",
            "/x/Capri.app/Contents/Resources/capri",
            "/x/Capri/Contents/MacOS/capri",
            "/x/Capri.app/MacOS/capri",
            "capri",
            "",
        ] {
            assert_eq!(app(bare), None, "{bare}");
        }
    }
}
