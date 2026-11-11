//! Le format de l'API d'Anthropic (« Messages ») : ce qu'on envoie, et comment lire ce qui revient.
//!
//! Tout est du texte : ce module ne se connecte à rien. Ce qu'il suit vient de la documentation
//! d'Anthropic, relue le 11 novembre 2026 :
//! - la requête (`POST /v1/messages`, `model`, `max_tokens` obligatoire, `system` à la racine,
//!   `messages` aux rôles `user` et `assistant`, `stream`), les raisons d'arrêt (`stop_reason`) :
//!   <https://platform.claude.com/docs/en/api/messages/create> ;
//! - l'en-tête `anthropic-version` et ce qu'apporte `2026-10-21` (plus de `temperature`, `top_p`
//!   ni `top_k` : une requête qui en envoie un est refusée) : <https://platform.claude.com/docs/en/api/versioning> ;
//! - le flux (`message_start`, `content_block_start`, `content_block_delta`, `content_block_stop`,
//!   `message_delta`, `message_stop`, des `ping` n'importe où, un `error` possible en plein flux,
//!   et des types nouveaux à tolérer) : <https://platform.claude.com/docs/en/build-with-claude/streaming> ;
//! - la forme des erreurs (`{"type": "error", "error": {"type": …, "message": …}, "request_id": …}`) :
//!   <https://platform.claude.com/docs/en/api/errors> ;
//! - la liste des modèles (`GET /v1/models`, `limit` de 1 à 1000, `has_more`, `last_id`,
//!   `after_id`) : <https://platform.claude.com/docs/en/api/models/list> ;
//! - le refus du modèle (`stop_reason: "refusal"`, réponse 200, ce qui a été écrit avant ne compte
//!   pas) : <https://platform.claude.com/docs/en/build-with-claude/refusals-and-fallback>.
//!
//! On n'envoie ni `temperature`, ni `top_p`, ni `top_k`, ni `thinking` (les modèles récents
//! réfléchissent d'eux-mêmes, et certains refusent qu'on le leur interdise), ni `fallbacks` : la
//! page des refus ne décrit ce repli côté serveur que pour quelques modèles (les Fable, Opus 5 et
//! 5.5, Sonnet 5.5 ; Haiku 5.5 n'en a pas et refuse une liste de replis), pas pour tous ceux que
//! la liste des modèles peut rendre.

use serde_json::{Value, json};

use crate::conversation::{Role, Turn};
use crate::wire::{End, ModelPage, Piece};

/// Où envoyer la conversation, sous l'adresse de base (`https://api.anthropic.com/v1`).
pub const MESSAGES_PATH: &str = "/messages";

/// L'en-tête qui dit quelle version de l'API on parle, obligatoire.
pub const VERSION_HEADER: &str = "anthropic-version";

/// La version de l'API dont ce module suit le format : la plus récente le 11 novembre 2026.
pub const API_VERSION: &str = "2026-10-21";

/// L'en-tête qui porte la clé d'API (pas `Authorization`) : voir [`crate::key::key_header`].
pub const KEY_HEADER: &str = "x-api-key";

/// Le plafond de jetons d'une réponse (`max_tokens`, obligatoire).
///
/// La règle : un nombre fixe, assez grand pour que la réflexion du modèle, qui compte dedans, ne
/// mange pas toute la place de sa réponse, et plus petit que le plafond de chacun des modèles que
/// la liste rend aujourd'hui (le plus petit est de 64 000 jetons, pour Haiku 4.5, Sonnet 4.5 et
/// Opus 4.5 ; 128 000 pour les autres). Ce n'est pas ce qui borne la longueur d'une réponse :
/// `capri-net` arrête de lire au bout de quelques milliers de caractères, et la personnalité
/// demande d'en écrire bien moins. Seuls les jetons produits sont facturés, pas ce plafond. Un
/// modèle dont le plafond serait plus bas refuserait la requête (400), avec son explication citée.
pub const MAX_TOKENS: u32 = 16_000;

/// Le nombre de modèles demandés par page, le plus grand que permet l'API.
const MODELS_PER_PAGE: u32 = 1000;

/// Le corps de la requête : la personnalité de Capri à la racine (`system`), puis la conversation.
///
/// La conversation envoyée est toujours valable pour l'API, quel que soit l'historique :
/// - deux prises de parole de suite de la même personne (Capri coupée avant d'avoir répondu) sont
///   fusionnées, comme pour les autres fournisseurs ;
/// - elle commence par l'utilisateur : une réponse de Capri restée en tête après qu'on a oublié sa
///   question est laissée de côté ;
/// - elle finit par l'utilisateur : une réponse de Capri en dernier serait prise pour un début de
///   réponse à continuer (le « prefill »), que les modèles récents refusent.
pub fn request_body(model: &str, persona: &str, turns: &[Turn]) -> String {
    let mut messages: Vec<(Role, String)> = Vec::with_capacity(turns.len());
    for turn in turns {
        match messages.last_mut() {
            Some((role, text)) if *role == turn.role => {
                text.push_str("\n\n");
                text.push_str(&turn.text);
            }
            None if turn.role == Role::Capri => {}
            _ => messages.push((turn.role, turn.text.clone())),
        }
    }
    if messages.last().is_some_and(|(role, _)| *role == Role::Capri) {
        messages.pop();
    }
    let messages: Vec<Value> = messages
        .into_iter()
        .map(|(role, content)| {
            let role = match role {
                Role::User => "user",
                Role::Capri => "assistant",
            };
            json!({ "role": role, "content": content })
        })
        .collect();
    let mut body = json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "messages": messages,
    });
    if !persona.trim().is_empty() {
        body["system"] = Value::String(persona.trim().to_owned());
    }
    body.to_string()
}

/// Lit les données d'un événement du flux.
///
/// Seul le texte des blocs `text` compte (`text_delta`) : la réflexion du modèle (`thinking_delta`,
/// `signature_delta`), les `ping` et tout événement inconnu sont ignorés sans erreur. Un `error` en
/// plein flux arrête la réponse, avec sa raison. Une réponse déclinée (`stop_reason: "refusal"`)
/// s'arrête là. Les autres raisons d'arrêt (`end_turn`, `max_tokens`…) n'arrêtent rien d'elles-mêmes :
/// la fin, c'est `message_stop`. Arrivée au plafond (`max_tokens`), la réponse est donc complète,
/// comme celle d'un autre fournisseur coupée par sa limite.
pub fn interpret(data: &str) -> Piece {
    let Ok(event) = serde_json::from_str::<Value>(data.trim()) else { return Piece::default() };
    match event["type"].as_str() {
        Some("content_block_delta") if event["delta"]["type"] == "text_delta" => {
            Piece { text: event["delta"]["text"].as_str().unwrap_or("").to_owned(), end: None }
        }
        Some("message_delta") if event["delta"]["stop_reason"] == "refusal" => {
            Piece { text: String::new(), end: Some(End::Declined) }
        }
        Some("message_stop") => Piece { text: String::new(), end: Some(End::Complete) },
        Some("error") => Piece { text: String::new(), end: Some(End::Failed(reason_of(&event))) },
        _ => Piece::default(),
    }
}

/// Le message d'erreur que porte un corps de réponse, s'il en porte un : celui d'un refus (4xx ou
/// 5xx).
pub fn error_message(body: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(body.trim()).ok()?;
    value.get("error").filter(|error| error.is_object())?;
    Some(reason_of(&value))
}

/// Ce que dit une erreur (`{"error": {"type": …, "message": …}}`) : son message, ou à défaut son
/// type.
fn reason_of(value: &Value) -> String {
    let error = &value["error"];
    [&error["message"], &error["type"]]
        .into_iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .find(|text| !text.is_empty())
        .unwrap_or("erreur sans explication")
        .to_owned()
}

/// Où demander une page de la liste des modèles. `after` : l'identifiant du dernier modèle de la
/// page précédente ; `None` si ce n'est pas un identifiant qu'on peut glisser tel quel dans une
/// adresse (il ne viendrait pas d'Anthropic).
pub fn models_path(after: Option<&str>) -> Option<String> {
    let first = format!("/models?limit={MODELS_PER_PAGE}");
    match after {
        None => Some(first),
        Some(id)
            if !id.is_empty()
                && id.len() <= 200
                && id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.')) =>
        {
            Some(format!("{first}&after_id={id}"))
        }
        Some(_) => None,
    }
}

/// Les modèles d'une page de `GET /v1/models`, et l'identifiant qui ouvre la suivante s'il y en a
/// une (`has_more`, `last_id`).
pub fn model_page(body: &str) -> Option<ModelPage> {
    let listing: Value = serde_json::from_str(body.trim()).ok()?;
    let models = listing["data"].as_array()?;
    let ids = models.iter().filter_map(|model| model["id"].as_str()).map(str::to_owned).collect();
    let next =
        (listing["has_more"] == true).then(|| listing["last_id"].as_str().map(str::to_owned)).flatten();
    Some(ModelPage { ids, next })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(role: Role, text: &str) -> Turn {
        Turn { role, text: text.to_owned() }
    }

    fn body(persona: &str, turns: &[Turn]) -> Value {
        serde_json::from_str(&request_body("un-modele", persona, turns)).expect("du JSON")
    }

    fn text(piece: &str) -> Piece {
        Piece { text: piece.to_owned(), end: None }
    }

    #[test]
    fn the_request_carries_the_persona_at_the_root_then_the_conversation() {
        let turns = [
            turn(Role::User, "Salut"),
            turn(Role::Capri, "Salut !"),
            turn(Role::User, "Ça va, l'« accent » ?"),
        ];
        assert_eq!(
            body("  Tu es Capri.\n", &turns),
            json!({
                "model": "un-modele",
                "max_tokens": MAX_TOKENS,
                "stream": true,
                "system": "Tu es Capri.",
                "messages": [
                    { "role": "user", "content": "Salut" },
                    { "role": "assistant", "content": "Salut !" },
                    { "role": "user", "content": "Ça va, l'« accent » ?" },
                ],
            })
        );
        // sans personnalité, pas de `system`
        assert!(body("  ", &turns).get("system").is_none());
    }

    #[test]
    fn nothing_the_api_refuses_is_ever_sent() {
        let sent = body("Tu es Capri.", &[turn(Role::User, "Salut")]);
        for absent in ["temperature", "top_p", "top_k", "thinking", "fallbacks", "metadata", "tools"] {
            assert!(sent.get(absent).is_none(), "{absent} : {sent}");
        }
        assert!(
            sent["messages"].as_array().expect("des messages").iter().all(|m| m["role"] != "system"),
            "la personnalité n'est pas un message"
        );
    }

    #[test]
    fn the_conversation_sent_always_alternates_and_ends_with_the_user() {
        let roles = |turns: &[Turn]| -> Vec<(String, String)> {
            body("", turns)["messages"]
                .as_array()
                .expect("des messages")
                .iter()
                .map(|m| {
                    (
                        m["role"].as_str().unwrap_or("").to_owned(),
                        m["content"].as_str().unwrap_or("").to_owned(),
                    )
                })
                .collect()
        };
        let pair = |role: &str, text: &str| (role.to_owned(), text.to_owned());
        // coupée deux fois : trois questions de suite, puis une réponse
        assert_eq!(
            roles(&[
                turn(Role::User, "un"),
                turn(Role::User, "deux"),
                turn(Role::User, "trois"),
                turn(Role::Capri, "réponse"),
                turn(Role::User, "quatre"),
            ]),
            [pair("user", "un\n\ndeux\n\ntrois"), pair("assistant", "réponse"), pair("user", "quatre")]
        );
        // une réponse restée en tête, sans sa question
        assert_eq!(
            roles(&[
                turn(Role::Capri, "orpheline"),
                turn(Role::Capri, "encore"),
                turn(Role::User, "question")
            ]),
            [pair("user", "question")]
        );
        // une réponse en dernier : ce n'est pas un début à continuer
        assert_eq!(
            roles(&[turn(Role::User, "question"), turn(Role::Capri, "réponse"), turn(Role::Capri, "suite")]),
            [pair("user", "question")]
        );
        // deux réponses de suite au milieu : fusionnées
        assert_eq!(
            roles(&[
                turn(Role::User, "a"),
                turn(Role::Capri, "b"),
                turn(Role::Capri, "c"),
                turn(Role::User, "d"),
            ]),
            [pair("user", "a"), pair("assistant", "b\n\nc"), pair("user", "d")]
        );
        assert!(roles(&[]).is_empty());
        assert!(roles(&[turn(Role::Capri, "seule")]).is_empty());
    }

    #[test]
    fn only_the_text_of_text_blocks_is_read() {
        let delta = |kind: &str, field: &str, value: &str| {
            json!({ "type": "content_block_delta", "index": 1, "delta": { "type": kind, field: value } })
                .to_string()
        };
        assert_eq!(interpret(&delta("text_delta", "text", "Salut ")), text("Salut "));
        assert_eq!(interpret(&delta("text_delta", "text", " déjà\n")), text(" déjà\n"));
        // la réflexion, même quand elle porte du texte, n'est jamais dite
        assert_eq!(interpret(&delta("thinking_delta", "thinking", "Il me demande…")), Piece::default());
        assert_eq!(interpret(&delta("thinking_delta", "thinking", "")), Piece::default());
        assert_eq!(interpret(&delta("signature_delta", "signature", "EqQBCgIYAh…")), Piece::default());
        assert_eq!(interpret(&delta("input_json_delta", "partial_json", "{\"a\"")), Piece::default());
        assert_eq!(interpret(&delta("citations_delta", "citation", "x")), Piece::default());
        assert_eq!(interpret(&delta("un_type_futur", "text", "x")), Piece::default());
    }

    #[test]
    fn pings_block_boundaries_and_unknown_events_are_skipped() {
        for data in [
            r#"{"type": "ping"}"#,
            r#"{"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","content":[],"model":"m","stop_reason":null,"usage":{"input_tokens":25,"output_tokens":1}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}"#,
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":15}}"#,
            r#"{"type":"un_evenement_futur","quoi":"rien"}"#,
            "",
            "pas du json",
            "{}",
            "[]",
            "null",
        ] {
            assert_eq!(interpret(data), Piece::default(), "{data:?}");
        }
    }

    #[test]
    fn knows_when_the_answer_is_over_declined_or_broken() {
        assert_eq!(
            interpret(r#"{"type":"message_stop"}"#),
            Piece { text: String::new(), end: Some(End::Complete) }
        );
        // au plafond de jetons : rien ne s'arrête avant message_stop, et ce qui est écrit compte
        assert_eq!(
            interpret(
                r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens","stop_sequence":null}}"#
            ),
            Piece::default()
        );
        let refusal = r#"{"type":"message_delta","delta":{"stop_reason":"refusal","stop_sequence":null,"stop_details":{"type":"refusal","category":"cyber","explanation":"…"}},"usage":{"output_tokens":3}}"#;
        assert_eq!(interpret(refusal), Piece { text: String::new(), end: Some(End::Declined) });
        let overloaded =
            r#"{"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}}"#;
        assert_eq!(
            interpret(overloaded),
            Piece { text: String::new(), end: Some(End::Failed("Overloaded".to_owned())) }
        );
        assert_eq!(
            interpret(r#"{"type":"error","error":{"type":"api_error","message":" "}}"#),
            Piece { text: String::new(), end: Some(End::Failed("api_error".to_owned())) }
        );
        assert_eq!(
            interpret(r#"{"type":"error"}"#),
            Piece { text: String::new(), end: Some(End::Failed("erreur sans explication".to_owned())) }
        );
    }

    #[test]
    fn finds_the_message_of_an_error_body() {
        let refused = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"},"request_id":"req_011CSHoEeqs5C35K2UUqR7Fy"}"#;
        assert_eq!(error_message(refused).as_deref(), Some("invalid x-api-key"));
        assert_eq!(
            error_message(r#"{"type":"error","error":{"type":"overloaded_error","message":""}}"#).as_deref(),
            Some("overloaded_error")
        );
        for body in ["", "<html>502</html>", "{}", r#"{"error":null}"#, r#"{"error":"texte"}"#, "pas du json"]
        {
            assert_eq!(error_message(body), None, "{body:?}");
        }
    }

    #[test]
    fn lists_the_models_page_by_page() {
        let first = r#"{"data":[{"type":"model","id":"modele-a","display_name":"A","max_input_tokens":1000000,"max_tokens":128000},{"type":"model","id":"modele-b"},{"type":"model"}],"first_id":"modele-a","has_more":true,"last_id":"modele-b"}"#;
        assert_eq!(
            model_page(first),
            Some(ModelPage {
                ids: vec!["modele-a".to_owned(), "modele-b".to_owned()],
                next: Some("modele-b".to_owned())
            })
        );
        let last =
            r#"{"data":[{"id":"modele-c"}],"first_id":"modele-c","has_more":false,"last_id":"modele-c"}"#;
        assert_eq!(model_page(last), Some(ModelPage { ids: vec!["modele-c".to_owned()], next: None }));
        assert_eq!(
            model_page(r#"{"data":[],"has_more":false,"first_id":null,"last_id":null}"#).map(|p| p.ids.len()),
            Some(0)
        );
        assert_eq!(model_page(r#"{"data":null}"#), None);
        assert_eq!(model_page("pas du json"), None);

        assert_eq!(models_path(None).as_deref(), Some("/models?limit=1000"));
        assert_eq!(models_path(Some("modele-b")).as_deref(), Some("/models?limit=1000&after_id=modele-b"));
        // un identifiant qui pourrait changer l'adresse n'y est jamais glissé
        for odd in ["", "a&limit=1", "a b", "a/../b", "é", "a#b", "a?b"] {
            assert_eq!(models_path(Some(odd)), None, "{odd:?}");
        }
    }
}
