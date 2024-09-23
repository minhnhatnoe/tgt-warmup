use std::error::Error;
use std::time::Duration;

use std::sync::{Arc, Mutex};
use tungstenite::{handshake::client::Response, stream::MaybeTlsStream, WebSocket};
use std::net::TcpStream;

const DEFAULT_API_DOMAIN: &str = "https://api.kucoin.com";
const DEFAULT_TOKEN_ENDPOINT: &str = "/api/v1/bullet-public";

#[derive(Debug)]
pub struct WebSocketClient {
    wss_domain: String,
    token: String,

    ping_interval: Duration,
    ping_timeout: Duration,
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

        let ping_timeout = match resp["data"]["instanceServers"][0]["pingTimeout"].to_owned() {
            serde_json::Value::Number(n) => Duration::from_millis(n.as_u64().unwrap()),
            unexpected => return Err(format!("Unexpected pingTimeout value: {}", unexpected).into())
        };

        Ok(Self::new(wss_domain, token, ping_interval, ping_timeout))
    }

    /// Constructs a WebSocketClient for connecting with KuCoin's WebSocket API
    /// 
    /// # Usage
    /// Token can be fetched manually by sending empty POST request to
    /// https://api.kucoin.com/api/v1/bullet-public
    /// 
    /// Consider using `new_with_token` to populate the fields with new token.
    pub fn new(wss_domain: String, token: String, ping_interval: Duration, ping_timeout: Duration) -> Self {
        WebSocketClient {
            wss_domain,
            token,
            ping_interval,
            ping_timeout,
        }
    }
}

struct WebSocketSessionInner {
    wsc: WebSocketClient,
    net_client: Mutex<WebSocket<MaybeTlsStream<TcpStream>>>,
}

pub struct WebSocketSession {
    wss: Arc<WebSocketSessionInner>,
}

impl WebSocketSession {
    pub fn start(wsc: WebSocketClient) -> Result<(WebSocketSession, Response), tungstenite::Error> {
        let (net_client, response) = tungstenite::connect(
                format!("{}?token={}", wsc.wss_domain, wsc.token))?;
    
        let session = WebSocketSession {
            wss: Arc::new(WebSocketSessionInner {
                wsc,
                net_client: Mutex::new(net_client),
            })
        };

        let _handle = std::thread::spawn(session.ping_loop());

        Ok((session, response))
    }

    // Ping. Discard response.
    fn ping_loop(&self) -> impl Fn() -> () {
        let session = self.clone();
        let data = "\"id\": \"0\", \"type\": \"ping\"}";

        move || {
            loop {
                let msg = tungstenite::Message::Text(data.to_string());
                session.send(msg).expect("Ping msg send failed");
                std::thread::sleep(session.wss.wsc.ping_interval);
            }
        }
    }

    fn send(&self, msg: tungstenite::Message) -> Result<(), tungstenite::Error> {
        self.wss.net_client.lock().unwrap().send(msg)
    }

    fn clone(&self) -> Self {
        WebSocketSession { wss: self.wss.clone() }
    }
}
