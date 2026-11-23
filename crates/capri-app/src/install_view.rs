//! Installer une version, sur un geste : ce que l'app en fait, entre la ligne de son menu, ce
//! qu'elle dit au-dessus de sa tête et le travail.
//!
//! Le déroulé est décidé par `capri_chat::update::install::Installation`, pur et essayé sans
//! système ; ici, on exécute ce qu'il ordonne : la préparation sur son thread
//! (`update_install::Preparation`), puis, l'app prête et la ligne choisie de nouveau, le relais
//! (`update_install::start_relay`), attendu sans bloquer la boucle. Pris, Capri se quitte par le
//! même chemin que « Quitter » ; sinon, l'app prête reste, et la ligne peut être choisie encore.
//!
//! Ailleurs que sur macOS, la ligne reste une information : la choisir dit qu'elle ne sait pas
//! s'installer seule sur ce système, et rien n'est lancé.

use capri_chat::update::Verdict;
use capri_chat::update::install::{Here, Installation, Order, Refusal};
use capri_claude::Tone;

use crate::update_view::Found;

/// Ce système sait-il l'installer ?
const SUPPORTED: bool = capri_platform::codesign::AVAILABLE;

pub struct InstallView {
    installation: Installation,
    /// La préparation en cours.
    #[cfg(target_os = "macos")]
    preparation: Option<crate::update_install::Preparation>,
    /// L'app prête, à côté de l'installée, en attendant le geste qui redémarre.
    #[cfg(target_os = "macos")]
    prepared: Option<crate::update_install::Staging>,
    /// Le programme de fin lancé, et depuis quand on attend qu'il prenne le relais.
    #[cfg(target_os = "macos")]
    relay: Option<(capri_platform::launch::Relay, std::time::Instant)>,
}

/// Ce que le travail a rapporté depuis le dernier pas.
#[derive(Default)]
pub struct Heard {
    /// Ce qu'elle en dit au-dessus de sa tête, à son tour.
    pub said: Vec<(String, Tone)>,
    /// Le programme de fin a pris le relais : elle se quitte.
    pub quit: bool,
}

impl Default for InstallView {
    fn default() -> Self {
        Self::new()
    }
}

impl InstallView {
    pub fn new() -> Self {
        Self {
            installation: Installation::new(),
            #[cfg(target_os = "macos")]
            preparation: None,
            #[cfg(target_os = "macos")]
            prepared: None,
            #[cfg(target_os = "macos")]
            relay: None,
        }
    }

    /// La ligne de son menu, d'après la version trouvée et où en est l'installation.
    pub fn menu_line(&self, found: Option<&Verdict>) -> Option<String> {
        self.installation.menu_line(found, SUPPORTED)
    }

    /// La ligne du menu vient d'être choisie : ce qu'elle en dit.
    pub fn chosen(&mut self, found: Option<&Found>) -> Option<(String, Tone)> {
        let here = Here {
            supported: SUPPORTED,
            in_app: crate::login_setup::this_app().is_some(),
            installed: crate::update_setup::installed(),
        };
        let reply = self.installation.chosen(found.map(|found| (&found.verdict, &found.manifest)), &here);
        let refused = match reply.order {
            Order::Nothing => None,
            Order::Prepare(plan) => self.prepare(plan).err(),
            Order::Restart => self.restart().err(),
        };
        match refused {
            Some(refusal) => {
                log::warn!("mise à jour : rien n'est installé ({})", refusal.for_log());
                self.installation.failed(&refusal).map(|said| (said, Tone::Failed))
            }
            None => reply.say.map(|said| (said, Tone::Done)),
        }
    }

    #[cfg(target_os = "macos")]
    fn prepare(&mut self, plan: capri_chat::update::install::Plan) -> Result<(), Refusal> {
        let source = crate::update_setup::source(|name| std::env::var_os(name)).map_err(|_| Refusal::Disk)?;
        let app = crate::login_setup::this_app().ok_or(Refusal::NotInApp)?;
        log::info!("mise à jour : installation de la {} demandée", plan.version);
        let preparation = crate::update_install::Preparation::start(plan, source, app.into())?;
        self.preparation = Some(preparation);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn prepare(&mut self, _plan: capri_chat::update::install::Plan) -> Result<(), Refusal> {
        Err(Refusal::Unsupported)
    }

    #[cfg(target_os = "macos")]
    fn restart(&mut self) -> Result<(), Refusal> {
        let app = crate::login_setup::this_app().ok_or(Refusal::NotInApp)?;
        let staging = self.prepared.as_ref().ok_or(Refusal::Handover)?;
        log::info!("mise à jour : redémarrage demandé, programme de fin lancé");
        let relay = crate::update_install::start_relay(staging, std::path::Path::new(&app))?;
        self.relay = Some((relay, std::time::Instant::now()));
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn restart(&mut self) -> Result<(), Refusal> {
        Err(Refusal::Unsupported)
    }

    /// À chaque pas de la boucle : ce que la préparation et le relais ont rapporté.
    pub fn pump(&mut self) -> Heard {
        #[cfg(target_os = "macos")]
        {
            use crate::update_install::{HANDOVER_WAIT, Report, complete_relay};
            let mut heard = Heard::default();
            let reports =
                self.preparation.as_ref().map(crate::update_install::Preparation::heard).unwrap_or_default();
            let done = |said: Option<String>| said.map(|said| (said, Tone::Done));
            let failed = |said: Option<String>| said.map(|said| (said, Tone::Failed));
            for report in reports {
                match report {
                    Report::Step(step) => heard.said.extend(done(self.installation.progressed(step))),
                    Report::Done(finished) => {
                        self.preparation = None;
                        match finished {
                            Ok(prepared) => {
                                heard.said.extend(done(self.installation.prepared(prepared.signed)));
                                self.prepared = Some(prepared.staging);
                            }
                            Err(refusal) => heard.said.extend(failed(self.installation.failed(&refusal))),
                        }
                    }
                }
            }
            let answer =
                self.relay.as_ref().map(|(relay, since)| (relay.ready(), since.elapsed() > HANDOVER_WAIT));
            match answer {
                Some((Some(true), _)) => {
                    let (relay, _) = self.relay.take().expect("là");
                    let done = match self.prepared.take() {
                        Some(staging) => complete_relay(relay, staging),
                        None => Err(Refusal::Handover),
                    };
                    match done {
                        Ok(()) => heard.quit = true,
                        Err(refusal) => heard.said.extend(failed(self.installation.failed(&refusal))),
                    }
                }
                Some((Some(false), _) | (None, true)) => {
                    // lâché sans « go » : le programme de fin renonce, et l'app prête reste
                    self.relay = None;
                    log::warn!("mise à jour : le programme de fin n'a pas pris le relais");
                    heard.said.extend(failed(self.installation.failed(&Refusal::Handover)));
                }
                Some((None, false)) | None => {}
            }
            heard
        }
        #[cfg(not(target_os = "macos"))]
        Heard::default()
    }

    /// Tout est abandonné (case décochée, ou Capri quittée) : la préparation arrêtée, l'app prête
    /// retirée, le relais lâché sans « go » (le programme de fin renonce) ; il ne reste rien à côté
    /// de l'app.
    pub fn abandon(&mut self) {
        #[cfg(target_os = "macos")]
        {
            if let Some(preparation) = self.preparation.take() {
                preparation.stop(crate::update_install::STOP_WAIT);
            }
            self.relay = None;
            self.prepared = None;
        }
        self.installation.abandon();
    }
}
