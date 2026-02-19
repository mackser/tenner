use std::fmt;

#[derive(Clone, Debug)]
pub enum Message {
    Broadcast { from: String, body: String },
    Private { from: String, body: String },
    System(String),
}

impl Message {
    pub fn to_display_for(&self, _viewer: &str) -> String {
        match self {
            Message::Broadcast { from, body } => format!("{}: {}", from, body),
            Message::Private { from, body } => format!("(private) {}: {}", from, body),
            Message::System(text) => format!("server: {}", text),
        }
    }

    pub fn broadcast(from: impl Into<String>, body: impl Into<String>) -> Self {
        Message::Broadcast {
            from: from.into(),
            body: body.into(),
        }
    }

    pub fn private(from: impl Into<String>, body: impl Into<String>) -> Self {
        Message::Private {
            from: from.into(),
            body: body.into(),
        }
    }

    pub fn system(text: impl Into<String>) -> Self {
        Message::System(text.into())
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::Broadcast { from, body } => write!(f, "{}: {}", from, body),
            Message::Private { from, body } => write!(f, "(private) {}: {}", from, body),
            Message::System(text) => write!(f, "server: {}", text),
        }
    }
}
