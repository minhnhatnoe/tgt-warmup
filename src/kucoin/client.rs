use super::api;
use super::api::*;
use super::book;
use super::error;
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tungstenite::{handshake::client::Response, stream::MaybeTlsStream};

struct WebSocket {
    net_client: Mutex<tungstenite::WebSocket<MaybeTlsStream<TcpStream>>>,
}

impl WebSocket {
    fn new(connection_string: String) -> Result<(WebSocket, Response), tungstenite::Error> {
        let (net_client, response) = tungstenite::connect(connection_string)?;

        let session = WebSocket {
            net_client: Mutex::new(net_client),
        };

        Ok((session, response))
    }

    fn send(&self, msg: String) -> Result<(), tungstenite::Error> {
        let msg = tungstenite::Message::Text(msg);

        let mut net_client = self.net_client.lock().unwrap();
        net_client.send(msg)
    }

    fn recv(&self) -> Result<String, tungstenite::Error> {
        let msg = self.net_client.lock().unwrap().read()?.into_text()?;
        Ok(msg)
    }
}

pub struct Session {
    ws: Arc<WebSocket>,
    data: Arc<Mutex<HashMap<String, mpsc::Sender<book::MarketBook>>>>,
}

impl Session {
    /// Initiate a WebSocket connection to the server and returns a handle
    /// for future operations.
    ///
    /// Steps performed:
    /// - Perform TLS handshake
    /// - Poll for welcome message
    /// - Starts a thread that regularly pings the server
    /// - Subscribes to a level 2 depth 5 topic (can extend to multiple)
    /// - Starts a thread that receives messages from the server
    pub fn start(
        credentials: &Credentials,
        level2_symbol: &str,
    ) -> Result<(Session, Response, mpsc::Receiver<book::MarketBook>), tungstenite::Error> {
        let (ws, response) = WebSocket::new(credentials.connection_string())?;
        let (pong_send, pong_recv) = mpsc::channel::<String>();

        let session = Self {
            ws: Arc::new(ws),
            data: Arc::new(Mutex::new(HashMap::new())),
        };

        match session.recv().expect("Cannot receive welcome") {
            Message::Welcome => println!("Client received server welcome!"),
            other_type => panic!("Message {:?} not expected", other_type),
        }

        session.spawn_ping_loop(
            pong_recv,
            credentials.ping_timeout,
            credentials.ping_interval,
        );

        let rx = session.subscribe_level2(level2_symbol);

        session.spawn_recv_loop(pong_send);

        Ok((session, response, rx))
    }

    fn spawn_ping_loop(
        &self,
        pong_recv: mpsc::Receiver<String>,
        ping_timeout: Duration,
        ping_interval: Duration,
    ) {
        let mut id: u64 = 0;

        fn duration_substract(a: Duration, b: Duration) -> Duration {
            if a <= b {
                return Duration::new(0, 0);
            }
            return a - b;
        }

        let session = self.clone();

        thread::spawn(move || loop {
            let id_str = id.to_string();
            // Will be blocked by recv loop. Todo: Use async ws,
            let _ = session.send(ping_string(id_str.as_str()));

            let send_time = Instant::now();

            loop {
                match pong_recv.recv_timeout(duration_substract(ping_timeout, send_time.elapsed()))
                {
                    Err(mpsc::RecvTimeoutError::Disconnected) => return (),
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                    Ok(id_recv) => {
                        if id_recv == id_str {
                            thread::sleep(duration_substract(ping_interval, send_time.elapsed()));
                            break;
                        }
                    }
                }
            }
            id += 1;
        });
    }

    fn spawn_recv_loop(&self, pong_send: mpsc::Sender<String>) {
        let session = self.clone();
        thread::spawn(move || loop {
            match session.recv() {
                Err(msg) => println!("{:?}", msg),
                Ok(Message::Pong(id)) => pong_send.send(id).expect("Cannot reach ping thread"),
                Ok(Message::Ack(_)) => (),
                Ok(Message::Message(msg)) => {
                    let (msg, topic) =
                        book::MarketBook::new(msg).expect(format!("Cannot parse msg").as_str());

                    let data_table = session.data.lock().unwrap();
                    let chan = data_table
                        .get(topic.as_str())
                        .expect(format!("Topic has no channel {}", topic).as_str());
                    chan.send(msg)
                        .expect(format!("Cannot send message for topic {:?}", topic).as_str())
                }
                Ok(other) => panic!("Received unexpected {:?}", other),
            }
        });
    }

    // All send should be done before recv_loop since we don't have async yet
    fn send(&self, msg: String) -> Result<(), tungstenite::Error> {
        self.ws.send(msg)
    }

    fn recv(&self) -> Result<Message, error::RecvError> {
        Ok(Message::from_string(self.ws.recv()?)?)
    }

    fn clone(&self) -> Self {
        Self {
            ws: self.ws.clone(),
            data: self.data.clone(),
        }
    }

    /// Starts subscribing to a level 2 depth 5 topic
    ///
    /// ## Returns
    /// A Receiver, receiving MarketBook.
    fn subscribe_level2(&self, symbol: &str) -> mpsc::Receiver<book::MarketBook> {
        let (msg, topic) = api::level2_subscription_string(symbol);

        let (send, recv) = mpsc::channel::<book::MarketBook>();
        self.data.lock().unwrap().insert(topic, send);

        self.send(msg).expect("Subscribe failed");
        // todo: ack

        recv
    }
}
