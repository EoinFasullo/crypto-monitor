use chrono::{Utc, Local};
use eframe::egui;
use reqwest::Client;
use serde_json::Value;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Crypto Monitor",
        options,
        Box::new(|_cc| Box::new(CryptoApp::default())),
    )
}

struct CryptoApp {
    btc: f64,
    eth: f64,
    sol: f64,
    status: String,
    last_updated: String,
    receiver: Option<Receiver<Result<(f64, f64, f64), String>>>,
    last_fetch: Instant,
}

impl Default for CryptoApp {
    fn default() -> Self {
        Self {
            btc: 0.0,
            eth: 0.0,
            sol: 0.0,
            status: "Starting...".to_string(),
            last_updated: "Never".to_string(),
            receiver: None,
            last_fetch: Instant::now() - Duration::from_secs(60),
        }
    }
}

impl CryptoApp {
    fn fetch_prices_async(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.status = "Fetching prices...".to_string();

        thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(format!("Failed to start async runtime: {}", e)));
                    return;
                }
            };

            let result = rt.block_on(async {
                let url = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum,solana&vs_currencies=usd";

                let client = Client::new();

                let response = client
                    .get(url)
                    .header("User-Agent", "crypto_monitor_gui/0.1 by Eoin")
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;

                let text = response
                    .text()
                    .await
                    .map_err(|e| format!("Failed reading response: {}", e))?;

                let json: Value =
                    serde_json::from_str(&text).map_err(|e| format!("JSON parse error: {}", e))?;

                if json.get("status").is_some() {
                    return Err(format!("API error: {}", text));
                }

                let btc = json["bitcoin"]["usd"]
                    .as_f64()
                    .ok_or("Missing BTC price")?;
                let eth = json["ethereum"]["usd"]
                    .as_f64()
                    .ok_or("Missing ETH price")?;
                let sol = json["solana"]["usd"]
                    .as_f64()
                    .ok_or("Missing SOL price")?;

                Ok((btc, eth, sol))
            });

            let _ = tx.send(result);
        });
    }
}

impl eframe::App for CryptoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_fetch.elapsed() >= Duration::from_secs(30) && self.receiver.is_none() {
            self.fetch_prices_async();
            self.last_fetch = Instant::now();
        }

        if let Some(rx) = &self.receiver {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok((btc, eth, sol)) => {
                        self.btc = btc;
                        self.eth = eth;
                        self.sol = sol;
                        self.status = "Prices loaded successfully".to_string();
                        let utc = Utc::now();
let local = Local::now();

self.last_updated = format!(
    "EST: {}\nUTC: {}",
    local.format("%Y-%m-%d %H:%M:%S"),
    utc.format("%Y-%m-%d %H:%M:%S")
);
                    }
                    Err(err) => {
                        self.status = err;
                    }
                }
                self.receiver = None;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Crypto Monitor");
            ui.separator();

            ui.label(format!("BTC: ${:.2}", self.btc));
            ui.label(format!("ETH: ${:.2}", self.eth));
            ui.label(format!("SOL: ${:.2}", self.sol));

            ui.separator();
            ui.label(format!("Status: {}", self.status));
            ui.label(format!("Last updated: {}", self.last_updated));

            ui.add_space(10.0);

            if ui.button("Refresh Now").clicked() && self.receiver.is_none() {
                self.fetch_prices_async();
                self.last_fetch = Instant::now();
            }

            ui.label("Auto-refresh: every 30 seconds");
        });

        ctx.request_repaint_after(Duration::from_millis(250));
    }
}