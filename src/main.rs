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
        let mut all_market_data = Vec::new();
        loop {
            let mut cursor: Option<String> = None;
            loop {
                let now = Utc::now();
                match client.get_markets(cursor.as_deref()).await {
                    Ok(markets) => {
                        if let Some(arr) = markets["data"].as_array() {
                            all_market_data.extend(arr.iter().cloned());
                        }
                        let next_cursor = markets["next_cursor"].as_str().map(|s| s.to_string());
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
            // Only use active markets with order books enabled and collect token_ids
            let mut token_ids = Vec::new();
            for m in all_market_data.iter() {
                if m.get("active").and_then(|a| a.as_bool()).unwrap_or(false)
                    && m.get("enable_order_book").and_then(|e| e.as_bool()).unwrap_or(false)
                {
                    if let Some(tokens) = m.get("tokens").and_then(|t| t.as_array()) {
                        for token in tokens {
                            if let Some(token_id) = token.get("token_id").and_then(|s| s.as_str()) {
                                if !token_id.is_empty() {
                                    token_ids.push(token_id.to_string());
                                }
                            }
                        }
                    }
                }
            }
            use std::collections::HashSet;
            let unique_token_ids: Vec<String> = token_ids.into_iter().collect::<HashSet<_>>().into_iter().collect();
            if unique_token_ids.is_empty() {
                eprintln!("No valid token_ids found!");
            } else {
                // Call get_order_books in batches of 10
                for batch in unique_token_ids.chunks(10) {
                    //println!("Requesting order books for token_ids: {:?}", batch);
                    match client.get_order_books(&batch.iter().cloned().collect::<Vec<_>>()).await {
                        Ok(order_books) => {
                            let now = Utc::now();
                            //println!("Fetched order books successfully.");
                            //println!("Order books: {:?}", order_books);
                            let serializable_books: Vec<_> = order_books.iter().map(|ob| {
                                let token_id = ob.asset_id.clone();
                                // Find the market_slug for this token_id
                                let market_slug = all_market_data.iter()
                                    .filter(|m| m.get("active").and_then(|a| a.as_bool()).unwrap_or(false))
                                    .filter(|m| m.get("enable_order_book").and_then(|e| e.as_bool()).unwrap_or(false))
                                    .find_map(|m| {
                                        m.get("tokens")
                                            .and_then(|tokens| tokens.as_array())
                                            .and_then(|arr| arr.iter().find(|token| token.get("token_id").and_then(|s| s.as_str()) == Some(&token_id)))
                                            .and_then(|_| m.get("market_slug").and_then(|s| s.as_str()).map(|s| s.to_string()))
                                    });
                                serde_json::json!({
                                    "token_id": token_id,
                                    "market_slug": market_slug,
                                    "order_book": format!("{:?}", ob)
                                })
                            }).collect();
                            let value = serde_json::json!({
                                "order_books": serializable_books,
                                "_fetched_at": now.to_rfc3339()
                            });
                            let json = serde_json::to_string(&value).unwrap();
                            //println!("Payload size: {:.2} KB", json.len() as f64 / 1024.0);
                            if let Err(e) = writeln!(file, "{}", json) {
                                eprintln!("Failed to write to log file: {}", e);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error fetching order books: {} for token_ids: {:?}", e, batch);
                        }
                    }
                }
            }
            sleep(Duration::from_secs(60 * 5)).await;
        }
    }
}
