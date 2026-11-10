//! Qui a dit quoi. L'historique est borné : une compagne qui tourne toute la journée ne doit pas
//! grossir sans fin, et ce qui sera envoyé à un modèle doit rester court.

/// Qui parle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Capri,
}

/// Une prise de parole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub role: Role,
    pub text: String,
}

/// Les derniers échanges, du plus ancien au plus récent.
#[derive(Clone, Debug)]
pub struct Conversation {
    turns: Vec<Turn>,
    max_turns: usize,
}

impl Conversation {
    /// `max_turns` : nombre de prises de parole gardées (au moins deux : une question et sa
    /// réponse).
    pub fn new(max_turns: usize) -> Self {
        Self { turns: Vec::new(), max_turns: max_turns.max(2) }
    }

    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Ajoute une prise de parole. Un texte vide (ou fait d'espaces) est ignoré.
    pub fn push(&mut self, role: Role, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.turns.push(Turn { role, text: text.to_owned() });
        // On oublie les plus anciennes, en faisant toujours commencer l'historique par
        // l'utilisateur : une réponse sans sa question n'aide aucun modèle.
        while self.turns.len() > self.max_turns {
            self.turns.remove(0);
        }
        while self.turns.len() > 1 && self.turns[0].role == Role::Capri {
            self.turns.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.turns.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles(conversation: &Conversation) -> String {
        conversation.turns().iter().map(|t| if t.role == Role::User { 'U' } else { 'C' }).collect()
    }

    #[test]
    fn keeps_turns_in_order_and_ignores_blank_ones() {
        let mut c = Conversation::new(10);
        assert!(c.is_empty());
        c.push(Role::User, "  Salut  ");
        c.push(Role::Capri, "");
        c.push(Role::Capri, " \n ");
        c.push(Role::Capri, "Salut !");
        assert_eq!(roles(&c), "UC");
        assert_eq!(c.turns()[0].text, "Salut", "les espaces autour sont retirés");
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn forgets_the_oldest_turns_and_always_starts_with_the_user() {
        let mut c = Conversation::new(4);
        for i in 0..5 {
            c.push(Role::User, &format!("question {i}"));
            c.push(Role::Capri, &format!("réponse {i}"));
        }
        assert_eq!(roles(&c), "UCUC");
        assert_eq!(c.turns()[0].text, "question 3");

        // une question de plus fait tomber la plus ancienne, et sa réponse orpheline avec elle
        c.push(Role::User, "question 5");
        assert_eq!(roles(&c), "UCU");
        assert_eq!(c.turns()[0].text, "question 4");
    }

    #[test]
    fn a_tiny_limit_still_holds_a_question_and_its_answer() {
        let mut c = Conversation::new(0);
        c.push(Role::User, "a");
        c.push(Role::Capri, "b");
        assert_eq!(roles(&c), "UC");
    }
}
