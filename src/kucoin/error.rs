use serde_json;
use tungstenite;

#[derive(Debug)]
pub enum RecvError {
    KeyNotExists(String),
    ParseError(serde_json::Error),
    NetworkError(tungstenite::Error)
}

impl From<String> for RecvError {
    fn from(value: String) -> Self {
        RecvError::KeyNotExists(value)
    }
}

impl From<serde_json::Error> for RecvError {
    fn from(value: serde_json::Error) -> Self {
        RecvError::ParseError(value)
    }
}

impl From<tungstenite::Error> for RecvError {
    fn from(value: tungstenite::Error) -> Self {
        RecvError::NetworkError(value)
    }
}
