use polymarket_rs_client::ClobClient;
use serde_json::to_string;
use std::{fs::OpenOptions, io::Write, time::Duration};
use tokio::time::sleep;
use chrono::Utc;

const HOST: &str = "https://clob.polymarket.com";
const POLYGON: u64 = 137;
const PRIVATE_KEY: &str = "0xde8b4b485b372656221bba68ec1eaee9a7c86cd5c8566c808cca212781b580cb";

#[tokio::main]
async fn main() {
    // Initialize the Polymarket client
    let client = ClobClient::with_l1_headers(HOST, PRIVATE_KEY, POLYGON);
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("data.jsonl")
        .expect("Unable to open log file");
    loop {
        let mut cursor: Option<String> = None;
        loop {
            let now = Utc::now();
            match client.get_markets(cursor.as_deref()).await {
                Ok(markets) => {
                    let mut value = serde_json::to_value(&markets).unwrap();
                    if let serde_json::Value::Object(ref mut map) = value {
                        map.insert("_fetched_at".to_string(), serde_json::Value::String(now.to_rfc3339()));
                    }
                    let json = serde_json::to_string(&value).unwrap();
                    if let Err(e) = writeln!(file, "{}", json) {
                        eprintln!("Failed to write to log file: {}", e);
                    }
                    let next_cursor = value
                        .get("next_cursor")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string());
                    if let Some(c) = next_cursor {
                        cursor = Some(c);
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error fetching markets: {}", e);
                    break;
                }
            }
        }
        sleep(Duration::from_secs(60)).await; // Adjust polling interval as needed
    }
}
