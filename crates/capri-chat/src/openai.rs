//! Le format « chat completions » d'OpenAI, que parlent Ollama, LM Studio, OpenRouter, DeepSeek
//! et beaucoup d'autres : ce qu'on envoie, et comment lire ce qui revient.
//!
//! Tout est du texte : ce module ne se connecte à rien. Il lit avec indulgence, parce que chaque
//! fournisseur a ses variantes (champs en plus, réflexion à part, erreurs sous deux formes), et ne
//! garde que le texte de la réponse.

use serde_json::{Value, json};

use crate::conversation::{Role, Turn};

/// La fin du flux, telle que l'annonce le format.
pub const DONE: &str = "[DONE]";

/// Le corps de la requête : la personnalité de Capri, puis la conversation.
///
/// Deux prises de parole de suite de la même personne (Capri coupée avant d'avoir répondu) sont
/// fusionnées : certains modèles refusent une conversation qui n'alterne pas.
pub fn request_body(model: &str, persona: &str, turns: &[Turn]) -> String {
    let mut messages: Vec<(&str, String)> = Vec::with_capacity(turns.len() + 1);
    if !persona.trim().is_empty() {
        messages.push(("system", persona.trim().to_owned()));
    }
    for turn in turns {
        let role = match turn.role {
            Role::User => "user",
            Role::Capri => "assistant",
        };
        match messages.last_mut() {
            Some((last, text)) if *last == role && role != "system" => {
                text.push_str("\n\n");
                text.push_str(&turn.text);
            }
            _ => messages.push((role, turn.text.clone())),
        }
    }
    let messages: Vec<Value> =
        messages.into_iter().map(|(role, content)| json!({ "role": role, "content": content })).collect();
    json!({ "model": model, "messages": messages, "stream": true }).to_string()
}

/// Ce qu'apporte un événement du flux.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Piece {
    /// Le texte à ajouter à la réponse ; vide si l'événement n'en porte pas.
    pub text: String,
    pub end: Option<End>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    /// La réponse est complète.
    Complete,
    /// Le fournisseur a abandonné en cours de route, et dit pourquoi.
    Failed(String),
}

/// Lit les données d'un événement du flux.
///
/// La réflexion d'un modèle (`reasoning`, `reasoning_content`) est ignorée : Capri ne dit que sa
/// réponse. Un événement illisible ou sans texte ne vaut rien, et n'arrête rien.
pub fn interpret(data: &str) -> Piece {
    let data = data.trim();
    if data == DONE {
        return Piece { text: String::new(), end: Some(End::Complete) };
    }
    let Ok(event) = serde_json::from_str::<Value>(data) else { return Piece::default() };
    if let Some(reason) = error_in(&event) {
        return Piece { text: String::new(), end: Some(End::Failed(reason)) };
    }
    let choice = &event["choices"][0];
    let text = text_of(&choice["delta"]["content"]);
    // `finish_reason` clôt la réponse même si le serveur n'envoie jamais `[DONE]`
    let end = (!choice["finish_reason"].is_null()).then_some(End::Complete);
    Piece { text, end }
}

/// Le message d'erreur que porte un corps de réponse, s'il en porte un. Sert pour un refus
/// (réponse 4xx ou 5xx) comme pour un abandon en plein flux.
pub fn error_message(body: &str) -> Option<String> {
    error_in(&serde_json::from_str::<Value>(body.trim()).ok()?)
}

/// Les noms des modèles que liste la réponse à `GET /models`.
pub fn model_ids(body: &str) -> Option<Vec<String>> {
    let listing: Value = serde_json::from_str(body.trim()).ok()?;
    let models = listing["data"].as_array()?;
    Some(models.iter().filter_map(|model| model["id"].as_str()).map(str::to_owned).collect())
}

/// `{"error": {"message": "…"}}` (OpenAI) ou `{"error": "…"}` (Ollama).
fn error_in(value: &Value) -> Option<String> {
    let error = value.get("error").filter(|error| !error.is_null())?;
    let message = match error {
        Value::String(message) => message.as_str(),
        _ => error["message"].as_str().unwrap_or(""),
    };
    let message = message.trim();
    Some(if message.is_empty() { "erreur sans explication".to_owned() } else { message.to_owned() })
}

/// Le texte d'un contenu : une chaîne, ou une liste de blocs dont on ne garde que le texte.
fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block["type"].as_str().is_none_or(|kind| kind == "text"))
            .filter_map(|block| block["text"].as_str())
            .collect(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(role: Role, text: &str) -> Turn {
        Turn { role, text: text.to_owned() }
    }

    fn text(piece: &str) -> Piece {
        Piece { text: piece.to_owned(), end: None }
    }

    #[test]
    fn the_request_carries_the_persona_then_the_conversation() {
        let turns = [
            turn(Role::User, "Salut"),
            turn(Role::Capri, "Salut !"),
            turn(Role::User, "Ça va, l'« accent » ?"),
        ];
        let body: Value =
            serde_json::from_str(&request_body("un-modèle:8b", "  Tu es Capri.\n", &turns)).expect("du JSON");
        assert_eq!(
            body,
            json!({
                "model": "un-modèle:8b",
                "stream": true,
                "messages": [
                    { "role": "system", "content": "Tu es Capri." },
                    { "role": "user", "content": "Salut" },
                    { "role": "assistant", "content": "Salut !" },
                    { "role": "user", "content": "Ça va, l'« accent » ?" },
                ],
            })
        );
    }

    #[test]
    fn consecutive_turns_of_the_same_speaker_are_merged() {
        // elle a été coupée deux fois : trois questions de suite, puis une réponse
        let turns = [
            turn(Role::User, "un"),
            turn(Role::User, "deux"),
            turn(Role::User, "trois"),
            turn(Role::Capri, "réponse"),
            turn(Role::User, "quatre"),
        ];
        let body: Value = serde_json::from_str(&request_body("m", "", &turns)).expect("du JSON");
        assert_eq!(
            body["messages"],
            json!([
                { "role": "user", "content": "un\n\ndeux\n\ntrois" },
                { "role": "assistant", "content": "réponse" },
                { "role": "user", "content": "quatre" },
            ]),
            "et sans personnalité, pas de message système"
        );
    }

    #[test]
    fn reads_the_text_of_each_chunk() {
        // un morceau d'Ollama
        let ollama = r#"{"id":"chatcmpl-1","object":"chat.completion.chunk","created":1,"model":"m","system_fingerprint":"fp_ollama","choices":[{"index":0,"delta":{"role":"assistant","content":"Salut "},"finish_reason":null}]}"#;
        assert_eq!(interpret(ollama), text("Salut "));
        // le premier morceau n'annonce souvent que le rôle
        assert_eq!(interpret(r#"{"choices":[{"index":0,"delta":{"role":"assistant"}}]}"#), Piece::default());
        assert_eq!(interpret(r#"{"choices":[{"delta":{"content":null}}]}"#), Piece::default());
        assert_eq!(
            interpret(r#"{"choices":[{"delta":{"content":""},"finish_reason":null}]}"#),
            Piece::default()
        );
        // les espaces et les retours à la ligne d'un morceau comptent
        assert_eq!(interpret(r#"{"choices":[{"delta":{"content":" déjà\n"}}]}"#), text(" déjà\n"));
        // un contenu en blocs : on ne garde que le texte
        let blocks = r#"{"choices":[{"delta":{"content":[{"type":"thinking","thinking":"hum"},{"type":"text","text":"Oui"},{"text":"."}]}}]}"#;
        assert_eq!(interpret(blocks), text("Oui."));
    }

    #[test]
    fn a_model_s_reasoning_is_never_part_of_the_answer() {
        for chunk in [
            r#"{"choices":[{"delta":{"role":"assistant","content":"","reasoning":"Il me demande…"}}]}"#,
            r#"{"choices":[{"delta":{"content":null,"reasoning_content":"Réfléchissons."}}]}"#,
            r#"{"choices":[{"delta":{"reasoning_details":[{"type":"reasoning.text","text":"…"}]}}]}"#,
        ] {
            assert_eq!(interpret(chunk), Piece::default(), "{chunk}");
        }
    }

    #[test]
    fn knows_when_the_answer_is_over() {
        let done = Piece { text: String::new(), end: Some(End::Complete) };
        assert_eq!(interpret("[DONE]"), done);
        assert_eq!(interpret(" [DONE]\n"), done);
        assert_eq!(interpret(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#), done);
        // le dernier mot peut arriver avec l'annonce de la fin
        assert_eq!(
            interpret(r#"{"choices":[{"delta":{"content":"fin."},"finish_reason":"length"}]}"#),
            Piece { text: "fin.".to_owned(), end: Some(End::Complete) }
        );
        // le décompte des jetons, envoyé après coup : rien à dire, rien à arrêter
        assert_eq!(
            interpret(r#"{"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":5}}"#),
            Piece::default()
        );
    }

    #[test]
    fn what_cannot_be_read_is_skipped() {
        for data in
            ["", "pas du json", "{\"choices\":[{\"delta\":{\"content\":\"coup", "42", "[]", "null", "{}"]
        {
            assert_eq!(interpret(data), Piece::default(), "{data:?}");
        }
    }

    #[test]
    fn reports_an_error_sent_in_the_middle_of_the_stream() {
        // la forme d'OpenRouter quand son fournisseur lâche en cours de réponse
        let openrouter = r#"{"id":"gen-1","error":{"code":"server_error","message":"Provider disconnected"},"choices":[{"index":0,"delta":{"content":""},"finish_reason":"error"}]}"#;
        assert_eq!(
            interpret(openrouter),
            Piece { text: String::new(), end: Some(End::Failed("Provider disconnected".to_owned())) }
        );
        assert_eq!(interpret(r#"{"error":null,"choices":[{"delta":{"content":"ok"}}]}"#), text("ok"));
    }

    #[test]
    fn finds_the_message_of_an_error_body() {
        let openai = r#"{"error":{"message":"model \"absent\" not found, try pulling it first","type":"not_found_error","param":null,"code":null}}"#;
        assert_eq!(
            error_message(openai).as_deref(),
            Some("model \"absent\" not found, try pulling it first")
        );
        assert_eq!(
            error_message(r#"{"error":"  model is required "}"#).as_deref(),
            Some("model is required")
        );
        assert_eq!(error_message(r#"{"error":{"code":429}}"#).as_deref(), Some("erreur sans explication"));
        for body in
            ["", "<html>502 Bad Gateway</html>", "{}", r#"{"error":null}"#, r#"{"message":"ailleurs"}"#]
        {
            assert_eq!(error_message(body), None, "{body:?}");
        }
    }

    #[test]
    fn lists_the_models_a_server_offers() {
        let listing = r#"{"object":"list","data":[{"id":"ministral-3:8b","object":"model","created":1,"owned_by":"library"},{"id":"qwen3.5:4b","object":"model"},{"object":"model"}]}"#;
        assert_eq!(model_ids(listing), Some(vec!["ministral-3:8b".to_owned(), "qwen3.5:4b".to_owned()]));
        assert_eq!(model_ids(r#"{"object":"list","data":[]}"#), Some(Vec::new()));
        assert_eq!(model_ids(r#"{"object":"list","data":null}"#), None);
        assert_eq!(model_ids("pas du json"), None);
    }
}
