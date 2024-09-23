use std::{
    error::Error,
    net::TcpStream,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::json;
use tungstenite::{handshake::client::Response, stream::MaybeTlsStream, WebSocket};

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

    /// Constructs a WebSocketClient for connecting with KuCoin's WebSocket API
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
        WebSocketClient {
            wss_domain,
            token,
            ping_interval,
            ping_timeout,
        }
    }
}

#[derive(Debug)]
enum WebSocketMessage {
    Welcome,
    Pong(String),
    Ack(String),
    Message(serde_json::Value),
}

impl WebSocketMessage {
    fn from_string(msg_str: String) -> Self {
        let msg: serde_json::Value = serde_json::from_str(msg_str.as_str()).expect("msg");

        let msg_type = msg
            .get("type")
            .expect("Cannot get message type")
            .as_str()
            .expect("Message type is not string");

        match msg_type {
            "welcome" => WebSocketMessage::Welcome,
            "ack" => {
                WebSocketMessage::Ack(msg["id"].as_str().expect("Cannot get ack id").to_owned())
            }
            "pong" => {
                WebSocketMessage::Pong(msg["id"].as_str().expect("Cannot get pong id").to_owned())
            }
            "message" => WebSocketMessage::Message(msg),
            other_type => panic!("Message type {other_type} not expected {:?}", msg),
        }
    }
}

struct WebSocketSessionInner {
    client: WebSocketClient,
    net_client: Mutex<WebSocket<MaybeTlsStream<TcpStream>>>,
}

impl WebSocketSessionInner {
    fn new(
        client: WebSocketClient,
    ) -> Result<(WebSocketSessionInner, Response), tungstenite::Error> {
        let (net_client, response) =
            tungstenite::connect(format!("{}?token={}", client.wss_domain, client.token))?;

        let session = WebSocketSessionInner {
            client,
            net_client: Mutex::new(net_client),
        };

        Ok((session, response))
    }

    fn send(&self, msg: tungstenite::Message) {
        self.net_client
            .lock()
            .unwrap()
            .send(msg)
            .expect("Cannot send msg");
    }

    fn recv(&self) -> WebSocketMessage {
        let msg = self
            .net_client
            .lock()
            .unwrap()
            .read()
            .expect("msg receive failed")
            .into_text()
            .expect("Cannot decode received msg");
        WebSocketMessage::from_string(msg)
    }
}

#[derive(Debug)]
pub struct MarketBook {
    asks: [(f64, i64); 5],
    bids: [(f64, i64); 5],
}

impl MarketBook {
    fn get_asks_bids(data: &serde_json::Value) -> [(f64, i64); 5] {
        let data = data
            .as_array()
            .expect("Data is not an array")
            .iter()
            .map(|x| {
                let price = x
                    .get(0)
                    .expect("Cannot get price from ask")
                    .as_str()
                    .expect("Price is not a string");
                let price = price.parse::<f64>().expect("Price is not a float");
                let size = x
                    .get(1)
                    .expect("Cannot get size from ask")
                    .as_i64()
                    .expect("Size is not an integer");
                (price, size)
            });

        let mut res = [(0.0, 0); 5];
        for (i, x) in data.enumerate() {
            res[i] = x;
        }

        res
    }
    pub fn new(asks: &serde_json::Value, bids: &serde_json::Value) -> Self {
        MarketBook {
            asks: MarketBook::get_asks_bids(asks),
            bids: MarketBook::get_asks_bids(bids),
        }
    }
}

pub struct WebSocketSession {
    wss: Arc<WebSocketSessionInner>,
}

impl WebSocketSession {
    /// Initiate a WebSocket connection to the server and returns a handle
    /// for future operations.
    ///
    /// Steps performed:
    /// - Perform TLS handshake
    /// - Poll for welcome message
    /// - Starts a thread that regularly pings the server
    pub fn start(
        client: WebSocketClient,
    ) -> Result<(WebSocketSession, Response), tungstenite::Error> {
        let (session, response) = WebSocketSessionInner::new(client)?;
        let session = WebSocketSession {
            wss: Arc::new(session),
        };

        match session.recv() {
            WebSocketMessage::Welcome => println!("Received welcome"),
            other_type => panic!("Message {:?} not expected", other_type),
        }

        let _handle = std::thread::spawn(session.ping_loop());

        Ok((session, response))
    }

    /// Subscribe to Level 2 order book updates for a given symbol.
    /// The acknowledgement from the server is discarded.
    pub fn subscribe_level2(&self, symbol: &str) {
        let data = json!({
            "id": 1,
            "type": "subscribe",
            "topic": format!("/contractMarket/level2Depth5:{symbol}"),
            "response": true
        })
        .to_string();

        self.send(tungstenite::Message::Text(data));
    }

    // Receive Level 2 order book updates.
    pub fn recv_level2(&self) -> MarketBook {
        let msg = match self.recv() {
            WebSocketMessage::Message(v) => v,
            other_type => panic!("Message {:?} not expected", other_type),
        };

        let msg = msg.get("data").expect("Cannot get data from msg");

        let asks = msg.get("asks").expect("Cannot get asks from msg");
        let bids = msg.get("bids").expect("Cannot get bids from msg");

        MarketBook::new(asks, bids)
    }

    // Ping. Discard response.
    fn ping_loop(&self) -> impl Fn() -> () {
        let session = self.clone();
        let data = json!({
            "id": 0,
            "type": "ping"
        })
        .to_string();

        move || loop {
            let msg = tungstenite::Message::Text((&data).to_owned());
            session.send(msg);
            std::thread::sleep(session.wss.client.ping_interval);
        }
    }

    fn send(&self, msg: tungstenite::Message) {
        self.wss.send(msg)
    }

    fn recv(&self) -> WebSocketMessage {
        loop {
            let msg = self.wss.recv();
            match msg {
                WebSocketMessage::Message(_) | WebSocketMessage::Welcome => return msg,
                WebSocketMessage::Ack(_) | WebSocketMessage::Pong(_) => {
                    println!("Received {:?}", msg)
                }
            }
        }
    }

    fn clone(&self) -> Self {
        WebSocketSession {
            wss: self.wss.clone(),
        }
    }
}
