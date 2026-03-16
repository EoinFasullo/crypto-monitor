use chrono::{Local, Utc};
use eframe::egui;
use egui::Color32;
use egui_plot::{Line, Plot, PlotPoints};
use reqwest::Client;
use serde_json::Value;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default()
        .with_inner_size([1400.0, 1000.0])
        .with_min_inner_size([1000.0, 700.0]),
    ..Default::default()
};

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

    prev_btc: f64,
    prev_eth: f64,
    prev_sol: f64,

    btc_history: Vec<f64>,
    eth_history: Vec<f64>,
    sol_history: Vec<f64>,

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
            prev_btc: 0.0,
            prev_eth: 0.0,
            prev_sol: 0.0,
            btc_history: Vec::new(),
            eth_history: Vec::new(),
            sol_history: Vec::new(),
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

                let btc = json["bitcoin"]["usd"].as_f64().ok_or("Missing BTC price")?;
                let eth = json["ethereum"]["usd"].as_f64().ok_or("Missing ETH price")?;
                let sol = json["solana"]["usd"].as_f64().ok_or("Missing SOL price")?;

                Ok((btc, eth, sol))
            });

            let _ = tx.send(result);
        });
    }

    fn movement_text(current: f64, previous: f64) -> &'static str {
        if previous == 0.0 {
            "-"
        } else if current > previous {
            "↑"
        } else if current < previous {
            "↓"
        } else {
            "→"
        }
    }

    fn movement_color(current: f64, previous: f64) -> Color32 {
        if previous == 0.0 {
            Color32::LIGHT_GRAY
        } else if current > previous {
            Color32::LIGHT_GREEN
        } else if current < previous {
            Color32::LIGHT_RED
        } else {
            Color32::LIGHT_GRAY
        }
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
                        self.prev_btc = self.btc;
                        self.prev_eth = self.eth;
                        self.prev_sol = self.sol;

                        self.btc = btc;
                        self.eth = eth;
                        self.sol = sol;

                        self.btc_history.push(btc);
                        if self.btc_history.len() > 30 {
                            self.btc_history.remove(0);
                        }

                        self.eth_history.push(eth);
                        if self.eth_history.len() > 30 {
                            self.eth_history.remove(0);
                        }

                        self.sol_history.push(sol);
                        if self.sol_history.len() > 30 {
                            self.sol_history.remove(0);
                        }

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

            let btc_color = Self::movement_color(self.btc, self.prev_btc);
            let eth_color = Self::movement_color(self.eth, self.prev_eth);
            let sol_color = Self::movement_color(self.sol, self.prev_sol);

            let btc_arrow = Self::movement_text(self.btc, self.prev_btc);
            let eth_arrow = Self::movement_text(self.eth, self.prev_eth);
            let sol_arrow = Self::movement_text(self.sol, self.prev_sol);

            ui.colored_label(btc_color, format!("BTC: ${:.2} {}", self.btc, btc_arrow));
            ui.colored_label(eth_color, format!("ETH: ${:.2} {}", self.eth, eth_arrow));
            ui.colored_label(sol_color, format!("SOL: ${:.2} {}", self.sol, sol_arrow));

            ui.separator();
            ui.label(format!("Status: {}", self.status));
            ui.label(format!("Last Updated\n{}", self.last_updated));

            ui.add_space(10.0);

            if ui.button("Refresh Now").clicked() && self.receiver.is_none() {
                self.fetch_prices_async();
                self.last_fetch = Instant::now();
            }

            ui.label("Auto-refresh: every 30 seconds");
            ui.add_space(15.0);

            ui.heading("BTC Price Chart");
            ui.label("Last 30 updates");

            let btc_points: PlotPoints = self
                .btc_history
                .iter()
                .enumerate()
                .map(|(i, price)| [i as f64, *price])
                .collect();

            let btc_line = Line::new(btc_points);

            Plot::new("btc_chart")
                .height(180.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(btc_line);
                });

            ui.add_space(10.0);

            ui.heading("ETH Price Chart");
            ui.label("Last 30 updates");

            let eth_points: PlotPoints = self
                .eth_history
                .iter()
                .enumerate()
                .map(|(i, price)| [i as f64, *price])
                .collect();

            let eth_line = Line::new(eth_points);

            Plot::new("eth_chart")
                .height(180.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(eth_line);
                });

            ui.add_space(10.0);

            ui.heading("SOL Price Chart");
            ui.label("Last 30 updates");

            let sol_points: PlotPoints = self
                .sol_history
                .iter()
                .enumerate()
                .map(|(i, price)| [i as f64, *price])
                .collect();

            let sol_line = Line::new(sol_points);

            Plot::new("sol_chart")
                .height(180.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(sol_line);
                });
        });

        ctx.request_repaint_after(Duration::from_millis(250));
    }
}