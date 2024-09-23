use std::error::Error;
use std::time::Duration;

const DEFAULT_API_DOMAIN: &str = "https://api.kucoin.com";
const DEFAULT_TOKEN_ENDPOINT: &str = "/api/v1/bullet-public";

#[derive(Debug)]
pub struct WebSocketClient {
    wss_domain: String,

    token: String,
    ping_interval: Duration,
}

impl WebSocketClient {
    /// Constructs a WebSocketClient for connecting with KuCoin's WebSocket API.
    /// Automatically fetches token from KuCoin's API.
    pub fn new_with_token() -> Result<Self, Box<dyn Error>> {
        let url = format!("{DEFAULT_API_DOMAIN}{DEFAULT_TOKEN_ENDPOINT}");

        let client = reqwest::blocking::Client::new();
        let resp: serde_json::Value = client
            .post(url)
            .send()?
            .error_for_status()?
            .json()?;

        let wss_domain = match resp["data"]["instanceServers"][0]["endpoint"].to_owned() {
            serde_json::Value::String(s) => s,
            unexpected => return Err(format!("Unexpected endpoint value: {}", unexpected).into())
        };

        let token = match resp["data"]["token"].to_owned() {
            serde_json::Value::String(s) => s,
            unexpected => return Err(format!("Unexpected token value: {}", unexpected).into())
        };

        let ping_interval = match resp["data"]["instanceServers"][0]["pingInterval"].to_owned() {
            serde_json::Value::Number(n) => Duration::from_millis(n.as_u64().unwrap()),
            unexpected => return Err(format!("Unexpected pingInterval value: {}", unexpected).into())
        };

        Ok(Self::new(wss_domain, token, ping_interval))
    }

    /// Constructs a WebSocketClient for connecting with KuCoin's WebSocket API
    /// 
    /// # Usage
    /// Token can be fetched manually by sending empty POST request to
    /// https://api.kucoin.com/api/v1/bullet-public
    /// 
    /// Consider using `new_with_token` to populate the fields with new token.
    pub fn new(wss_domain: String, token: String, ping_interval: Duration) -> Self {
        WebSocketClient {
            wss_domain,
            token,
            ping_interval
        }
    }

    fn send(&self) {
        
    }

}
