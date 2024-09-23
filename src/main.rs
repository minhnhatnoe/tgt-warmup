mod kucoin;

fn main() {
    let client = kucoin::WebSocketClient::new_with_token().unwrap();
    let (session, _response) = kucoin::WebSocketSession::start(client).unwrap();

    
}
