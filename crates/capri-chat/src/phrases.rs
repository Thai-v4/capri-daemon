//! Les phrases sur le cerveau que disent la ligne de commande (`capri chat`) et la fenêtre de
//! réglages : écrites une seule fois, pour que les deux disent toujours la même chose.
//!
//! La plus importante est [`destination`] : où part ce qu'on dit à Capri. Elle doit être vraie,
//! quitte à prévenir pour rien : dans le doute, elle dit que ça sort de la machine.

use crate::brain::Provider;
use crate::endpoint::{Endpoint, Reach};

/// Ce qui ne part jamais, quel que soit le cerveau.
const NOTHING_ELSE: &str = "Rien d'autre de ta machine : ni ce que fait Claude Code, ni tes fichiers.";

/// Le serveur, pour une phrase : « OpenRouter (openrouter.ai:443) », ou l'adresse seule d'un
/// serveur personnalisé.
pub fn server(provider: Provider, endpoint: &Endpoint) -> String {
    match provider {
        Provider::Custom => endpoint.display_host(),
        _ => format!("{} ({})", provider.name(), endpoint.display_host()),
    }
}

/// « d'OpenRouter », « de DeepSeek », « du serveur personnalisé ».
pub fn of_provider(provider: Provider) -> String {
    match provider {
        Provider::Custom => "du serveur personnalisé".to_owned(),
        _ if provider.name().starts_with(['A', 'E', 'I', 'O', 'U']) => format!("d'{}", provider.name()),
        _ => format!("de {}", provider.name()),
    }
}

/// Un modèle « cloud » d'Ollama : servi par les serveurs d'Ollama, même demandé à un Ollama qui
/// tourne sur cette machine. D'après sa documentation (relue le 10 novembre 2026), l'app et la
/// commande `ollama` les nomment `…:cloud` (`gemma4:cloud`) ou `…-cloud` (`gpt-oss:120b-cloud`),
/// et l'Ollama local, une fois connecté (`ollama signin`), les relaie vers ses serveurs. Un nom
/// qui finit ainsi compte pour « cloud », même sans étiquette : mieux vaut prévenir pour rien.
pub fn is_ollama_cloud_model(model: &str) -> bool {
    let model = model.trim().to_ascii_lowercase();
    model.ends_with(":cloud") || model.ends_with("-cloud")
}

/// Où part ce qu'on dit à Capri avec ce cerveau, en une phrase. `model` : le modèle choisi, s'il
/// l'est déjà ; avec Ollama, un modèle « cloud » part sur Internet même si le serveur est ici.
pub fn destination(provider: Provider, endpoint: &Endpoint, model: Option<&str>) -> String {
    let host = endpoint.display_host();
    let how = if endpoint.secure { "chiffré (https)" } else { "en clair (http)" };
    let relays = provider == Provider::Ollama && endpoint.reach() != Reach::Internet;
    let cloud = relays && model.is_some_and(is_ollama_cloud_model);
    let maybe_cloud = relays && model.is_none();
    match endpoint.reach() {
        Reach::ThisMachine if cloud => format!(
            "Attention : avec ce modèle « cloud », l'Ollama de cette machine relaie ce que tu lui dis \
             vers les serveurs d'Ollama, sur Internet, à chaque message, avec sa personnalité. {NOTHING_ELSE}"
        ),
        Reach::PrivateNetwork if cloud => format!(
            "Attention : ce que tu lui dis part vers {host}, sur ton réseau privé, {how}, puis, avec ce modèle \
             « cloud », l'Ollama de ce serveur le relaie vers les serveurs d'Ollama, sur Internet, à chaque \
             message, avec sa personnalité. {NOTHING_ELSE}"
        ),
        Reach::ThisMachine if maybe_cloud => "Ce que tu lui dis reste sur cette machine, sauf avec un modèle \
                                              « cloud » (son nom finit par :cloud ou -cloud) : Ollama le relaie \
                                              alors vers ses serveurs, sur Internet."
            .to_owned(),
        Reach::PrivateNetwork if maybe_cloud => format!(
            "Ce que tu lui dis part vers {host}, sur ton réseau privé, {how}. Avec un modèle « cloud » (son nom \
             finit par :cloud ou -cloud), Ollama le relaie en plus vers ses serveurs, sur Internet."
        ),
        Reach::ThisMachine => "Ce que tu lui dis reste sur cette machine.".to_owned(),
        Reach::PrivateNetwork => format!("Ce que tu lui dis part vers {host}, sur ton réseau privé, {how}."),
        Reach::Internet => {
            let whom = server(provider, endpoint);
            format!(
                "Attention : ce que tu lui dis partira chez {whom}, sur Internet, à chaque message, avec sa \
                 personnalité. {NOTHING_ELSE}"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(url: &str) -> Endpoint {
        Endpoint::parse(url).expect("adresse valable")
    }

    #[test]
    fn choosing_a_brain_says_where_the_conversation_goes() {
        let model = Some("un-modele:8b");
        assert_eq!(
            destination(Provider::Ollama, &at("http://localhost:11434/v1"), model),
            "Ce que tu lui dis reste sur cette machine."
        );
        assert_eq!(
            destination(Provider::Ollama, &at("http://192.168.1.20:11434/v1"), model),
            "Ce que tu lui dis part vers 192.168.1.20:11434, sur ton réseau privé, en clair (http)."
        );
        assert_eq!(
            destination(Provider::OpenRouter, &at("https://openrouter.ai/api/v1"), model),
            "Attention : ce que tu lui dis partira chez OpenRouter (openrouter.ai:443), sur Internet, à chaque message, \
             avec sa personnalité. Rien d'autre de ta machine : ni ce que fait Claude Code, ni tes fichiers."
        );
        assert_eq!(
            destination(Provider::OllamaCloud, &at("https://ollama.com/v1"), None),
            "Attention : ce que tu lui dis partira chez Ollama en ligne (ollama.com:443), sur Internet, à chaque \
             message, avec sa personnalité. Rien d'autre de ta machine : ni ce que fait Claude Code, ni tes fichiers."
        );
        assert!(
            destination(Provider::Custom, &at("https://llm.example.com/v1"), model)
                .contains("partira chez llm.example.com:443,")
        );
        // un nom qui mène peut-être chez soi compte pour Internet : rien ne le garantit
        assert!(
            destination(Provider::Custom, &at("https://tesseract.local:8443/v1"), None)
                .starts_with("Attention")
        );
    }

    #[test]
    fn a_cloud_model_of_a_local_ollama_leaves_the_machine_and_it_is_said() {
        let here = at("http://localhost:11434/v1");
        for model in ["gpt-oss:120b-cloud", "gemma4:cloud", " GEMMA4:CLOUD ", "un-modele-cloud"] {
            let said = destination(Provider::Ollama, &here, Some(model));
            assert!(
                said.starts_with("Attention : avec ce modèle « cloud », l'Ollama de cette machine relaie"),
                "{model} : {said}"
            );
            assert!(said.contains("sur Internet") && !said.contains("reste sur cette machine"), "{said}");
        }
        let lan = destination(Provider::Ollama, &at("http://192.168.1.20:11434/v1"), Some("gemma4:cloud"));
        assert!(
            lan.starts_with(
                "Attention : ce que tu lui dis part vers 192.168.1.20:11434, sur ton réseau privé"
            ) && lan.contains("vers les serveurs d'Ollama, sur Internet"),
            "{lan}"
        );
        // un modèle pas encore choisi : la phrase ne promet rien qu'un modèle « cloud » démentirait
        let unknown = destination(Provider::Ollama, &here, None);
        assert!(
            unknown.starts_with("Ce que tu lui dis reste sur cette machine, sauf avec un modèle « cloud »")
        );
        assert!(unknown.contains("sur Internet"), "{unknown}");
        let unknown = destination(Provider::Ollama, &at("http://10.0.0.5:11434/v1"), None);
        assert!(unknown.contains("Avec un modèle « cloud »"), "{unknown}");
        // ailleurs qu'Ollama, ce nom ne veut rien dire de particulier
        assert_eq!(
            destination(Provider::LmStudio, &at("http://localhost:1234/v1"), Some("x:cloud")),
            "Ce que tu lui dis reste sur cette machine."
        );
        assert_eq!(
            destination(Provider::LmStudio, &at("http://localhost:1234/v1"), None),
            "Ce que tu lui dis reste sur cette machine."
        );
    }

    #[test]
    fn a_cloud_model_is_recognised_by_its_name() {
        for model in ["gpt-oss:120b-cloud", "gemma4:cloud", "qwen3-coder:480b-CLOUD", "x-cloud"] {
            assert!(is_ollama_cloud_model(model), "{model}");
        }
        for model in ["gemma4:31b", "cloudy:7b", "cloud-model:8b", "un:cloudless", "", "cloud"] {
            assert!(!is_ollama_cloud_model(model), "{model}");
        }
    }

    #[test]
    fn a_provider_is_named_in_a_sentence() {
        assert_eq!(of_provider(Provider::OpenRouter), "d'OpenRouter");
        assert_eq!(of_provider(Provider::OllamaCloud), "d'Ollama en ligne");
        assert_eq!(of_provider(Provider::DeepSeek), "de DeepSeek");
        assert_eq!(of_provider(Provider::Custom), "du serveur personnalisé");
        assert_eq!(
            server(Provider::OllamaCloud, &at("https://ollama.com/v1")),
            "Ollama en ligne (ollama.com:443)"
        );
        assert_eq!(server(Provider::Custom, &at("http://127.0.0.1:8080/v1")), "127.0.0.1:8080");
    }
}
