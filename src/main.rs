mod kucoin;

fn main() {
    let client = kucoin::WebSocketClient::new_with_token().unwrap();
    println!("{:?}", client);
}
