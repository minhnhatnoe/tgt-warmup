mod kucoin;

#[tokio::main]
async fn main() {
    let client = kucoin::WebSocketClient::new_with_token().await.unwrap();
    println!("{:?}", client);
}
