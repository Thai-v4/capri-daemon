//! Un faux serveur, sur cette machine, pour les tests de l'app : il sert une connexion par réponse
//! donnée, dans l'ordre, et rend les en-têtes et le corps de chaque requête reçue. Rien ne sort de
//! la machine.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc::{Receiver, channel};

/// Une requête reçue : ses en-têtes (ligne de requête comprise), puis son corps.
pub type Request = (String, String);

pub fn serve(responses: Vec<String>) -> (SocketAddr, Receiver<Request>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("un port libre");
    let address = listener.local_addr().expect("adresse");
    let (sender, requests) = channel();
    std::thread::spawn(move || {
        for (stream, response) in listener.incoming().zip(responses) {
            let Ok(mut stream) = stream else { return };
            let (mut bytes, mut buffer) = (Vec::new(), [0u8; 4096]);
            // la requête entière : ses en-têtes, puis le corps qu'ils annoncent
            let request = loop {
                if let Some(head_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&bytes[..head_end]).into_owned();
                    let length =
                        header(&head, "content-length").and_then(|value| value.parse().ok()).unwrap_or(0);
                    if bytes.len() >= head_end + 4 + length {
                        break (head, String::from_utf8_lossy(&bytes[head_end + 4..]).into_owned());
                    }
                }
                match stream.read(&mut buffer) {
                    Ok(received) if received > 0 => bytes.extend_from_slice(&buffer[..received]),
                    _ => return,
                }
            };
            let _ = sender.send(request);
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (address, requests)
}

/// La valeur d'un en-tête, quelle que soit sa casse.
pub fn header(head: &str, name: &str) -> Option<String> {
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_owned())
    })
}

/// Une réponse complète, de longueur annoncée.
pub fn response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Une réponse en flux qui dit `text`, puis s'arrête.
pub fn reply(text: &str) -> String {
    let event = serde_json::json!({ "choices": [{ "delta": { "content": text } }] });
    response("200 OK", "text/event-stream", &format!("data: {event}\n\ndata: [DONE]\n\n"))
}

/// La liste de modèles `GET /models`.
pub fn listing(models: &[&str]) -> String {
    let data: Vec<_> = models.iter().map(|id| serde_json::json!({ "id": id, "object": "model" })).collect();
    response("200 OK", "application/json", &serde_json::json!({ "object": "list", "data": data }).to_string())
}
