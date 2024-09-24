use serde_json::json;
use std::{error::Error, time::Duration};

const DEFAULT_API_DOMAIN: &str = "https://api.kucoin.com";
const DEFAULT_TOKEN_ENDPOINT: &str = "/api/v1/bullet-public";

#[derive(Debug)]
pub struct Credentials {
    wss_domain: String,
    token: String,

    pub ping_interval: Duration,
    pub ping_timeout: Duration,
}

impl Credentials {
    /// Constructs a Credentials for connecting with KuCoin's WebSocket API.
    /// Automatically fetches token from KuCoin's API.
    pub fn new_with_token() -> Result<Self, Box<dyn Error>> {
        let url = format!("{DEFAULT_API_DOMAIN}{DEFAULT_TOKEN_ENDPOINT}");

        let client = reqwest::blocking::Client::new();
        let resp: serde_json::Value = client.post(url).send()?.error_for_status()?.json()?;

        let wss_domain = match resp["data"]["instanceServers"][0]["endpoint"].to_owned() {
            serde_json::Value::String(s) => s,
            unexpected => return Err(format!("Unexpected endpoint value: {}", unexpected).into()),
        };

        let token = match resp["data"]["token"].to_owned() {
            serde_json::Value::String(s) => s,
            unexpected => return Err(format!("Unexpected token value: {}", unexpected).into()),
        };

        let ping_interval = match resp["data"]["instanceServers"][0]["pingInterval"].to_owned() {
            serde_json::Value::Number(n) => Duration::from_millis(n.as_u64().unwrap()),
            unexpected => {
                return Err(format!("Unexpected pingInterval value: {}", unexpected).into())
            }
        };

        let ping_timeout = match resp["data"]["instanceServers"][0]["pingTimeout"].to_owned() {
            serde_json::Value::Number(n) => Duration::from_millis(n.as_u64().unwrap()),
            unexpected => {
                return Err(format!("Unexpected pingTimeout value: {}", unexpected).into())
            }
        };

        Ok(Self::new(wss_domain, token, ping_interval, ping_timeout))
    }

    /// Constructs a Credentials for connecting with KuCoin's WebSocket API
    ///
    /// # Usage
    /// Token can be fetched manually by sending empty POST request to
    /// https://api.kucoin.com/api/v1/bullet-public
    ///
    /// Consider using `new_with_token` to populate the fields with new token.
    pub fn new(
        wss_domain: String,
        token: String,
        ping_interval: Duration,
        ping_timeout: Duration,
    ) -> Self {
        Credentials {
            wss_domain,
            token,
            ping_interval,
            ping_timeout,
        }
    }

    // Constructs a connection string for use with WebSockets
    pub fn connection_string(&self) -> String {
        format!("{}?token={}", self.wss_domain, self.token)
    }
}

#[derive(Debug)]
pub enum Message {
    Welcome,
    Pong(String),
    Ack(String),
    Message(serde_json::Value),
}

impl Message {
    /// Build a message from a json-formatted String
    pub fn from_string(msg_str: String) -> Result<Self, serde_json::Error> {
        let msg: serde_json::Value = serde_json::from_str(msg_str.as_str())?;

        let msg_type = msg
            .get("type")
            .expect("Cannot get message type")
            .as_str()
            .expect("Message type is not string");

        let id = msg["id"].as_str().ok_or("Cannot get message id");

        let msg = match msg_type {
            "welcome" => Self::Welcome,
            "ack" => Self::Ack(id.unwrap().to_owned()),
            "pong" => Self::Pong(id.unwrap().to_owned()),
            "message" => Self::Message(msg),
            other_type => panic!("Message type {other_type} not expected {:?}", msg),
        };

        Ok(msg)
    }
}

pub fn level2_subscription_string(symbol: &str) -> (String, String) {
    let topic = format!("/contractMarket/level2Depth5:{symbol}");
    (
        json!({
            "id": 1,
            "type": "subscribe",
            "topic": topic.to_owned(),
            "privateChannel": false,
            "response": true
        })
        .to_string(),
        topic,
    )
}

pub fn ping_string(id: &str) -> String {
    json!({
        "id": id,
        "type": "ping"
    })
    .to_string()
}
