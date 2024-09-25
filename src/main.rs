mod kucoin;

fn main() {
    let credentials = kucoin::Credentials::new_with_token().unwrap();
    let (_session, response, rx) = kucoin::Session::start(&credentials, "ETHUSDTM").unwrap();

    println!("Handshake response: {:?}", response);

    loop {
        println!("{}", rx.recv().unwrap());
    }
}
