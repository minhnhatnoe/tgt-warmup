use super::error::RecvError;

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
    pub fn new(data: serde_json::Value) -> Result<(Self, String), RecvError> {
        let topic = data
            .get("topic").ok_or("key topic not exists".to_string())?
            .as_str().expect("value of key topic is not a string")
            .to_string();
        let data = data.get("data").ok_or("key data not exists".to_string())?;

        let asks = data.get("asks").ok_or("key asks doesn't exists".to_string())?;
        let bids = data.get("bids").ok_or("key bids doesn't exists".to_string())?;
        Ok((MarketBook {
            asks: MarketBook::get_asks_bids(asks),
            bids: MarketBook::get_asks_bids(bids),
        }, topic))
    }
}
