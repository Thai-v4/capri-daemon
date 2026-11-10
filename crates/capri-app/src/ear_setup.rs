//! Son oreille côté app : ce qu'elle dit quand elle ne peut pas écouter, et `capri ear status`.

use capri_platform::{Access, Ear, EarError, Permission};

/// Le raccourci à maintenir pour lui parler, tel qu'on l'écrit à l'utilisateur.
pub const SHORTCUT: &str = "⌃⌥M";

/// Ce que Capri dit quand l'écoute ne démarre pas.
pub fn explain(error: &EarError) -> &'static str {
    match error {
        EarError::NeedsApp => "Pour m'écouter, lance-moi depuis Capri.app",
        EarError::Asking => "Autorise le micro et la dictée, puis recommence",
        EarError::Denied(Permission::Microphone) => {
            "Micro refusé : Réglages Système > Confidentialité et sécurité"
        }
        EarError::Denied(Permission::SpeechRecognition) => {
            "Dictée refusée : Réglages Système > Confidentialité et sécurité"
        }
        EarError::Unavailable => "Pas de reconnaissance vocale en français sur cet appareil",
        EarError::NoMicrophone => "Je ne trouve pas de micro",
    }
}

/// La commande `capri ear <action>`. Rend le code de sortie du processus.
pub fn run_cli(args: &[String]) -> i32 {
    if args.len() != 1 || args[0] != "status" {
        eprintln!("usage : capri ear status    si elle peut t'écouter au micro, et sinon pourquoi");
        return 2;
    }
    let Some(status) = Ear::status() else {
        println!("Capri ne sait pas encore écouter sur ce système.");
        return 0;
    };
    let access = |access: Access| match access {
        Access::Granted => "accordé",
        Access::Denied => "REFUSÉ (Réglages Système > Confidentialité et sécurité)",
        Access::NotAsked => "pas encore demandé : le système posera la question à la première écoute",
    };
    println!("raccourci             : maintenir {SHORTCUT} pour lui parler, relâcher pour envoyer");
    println!(
        "transcription         : {}",
        if status.on_device {
            "en français, sur l'appareil (le son ne quitte pas la machine)"
        } else {
            "INDISPONIBLE en français sur cet appareil : Capri ne t'écoutera pas"
        }
    );
    println!(
        "lancée depuis son app : {}",
        if status.in_app {
            "oui"
        } else {
            "non : le micro ne s'ouvre que depuis Capri.app (tools/bundle-macos.sh)"
        }
    );
    println!("micro                 : {}", access(status.microphone));
    println!("reconnaissance vocale : {}", access(status.speech_recognition));
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_not_to_listen_fits_in_her_banner() {
        for error in [
            EarError::NeedsApp,
            EarError::Asking,
            EarError::Denied(Permission::Microphone),
            EarError::Denied(Permission::SpeechRecognition),
            EarError::Unavailable,
            EarError::NoMicrophone,
        ] {
            let said = explain(&error);
            assert!(!said.is_empty() && said.chars().count() <= 64, "{said}");
        }
    }
}
