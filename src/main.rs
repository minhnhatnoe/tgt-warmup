mod kucoin;

fn main() {
    let client = kucoin::WebSocketClient::new_with_token().unwrap();
    let (session, _response) = kucoin::WebSocketSession::start(client).unwrap();

    session.subscribe_level2("ETHUSDTM");
    loop {
        println!("Waiting for message");
        let msg = session.recv_level2();
        println!("{:?}", msg);
    }
}
