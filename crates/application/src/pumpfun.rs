//! Read-only Pump.fun discovery via PumpPortal. Never subscribes to paid trade streams.
use crate::upstream;
use futures_util::{stream::FuturesUnordered, SinkExt, StreamExt};
use kairos_live::now;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use solana_pubkey::Pubkey;
use std::{collections::VecDeque, str::FromStr, time::Duration};
use tokio::{sync::watch, time};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
};

const CAPACITY: usize = 300;
const FRESH_SECONDS: u64 = 90;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub max_pair_age_hours: u64,
    pub min_liquidity_usd: Decimal,
    pub min_volume_h1_usd: Decimal,
    pub min_abs_change_h1_pct: Decimal,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            max_pair_age_hours: 24,
            min_liquidity_usd: Decimal::from(10_000),
            min_volume_h1_usd: Decimal::from(10_000),
            min_abs_change_h1_pct: Decimal::from(5),
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=168).contains(&self.max_pair_age_hours)
            || [&self.min_liquidity_usd, &self.min_volume_h1_usd]
                .into_iter()
                .any(|n| *n < Decimal::ZERO || *n > Decimal::from(1_000_000_000) || n.scale() > 2)
            || self.min_abs_change_h1_pct < Decimal::ZERO
            || self.min_abs_change_h1_pct > Decimal::from(10_000)
            || self.min_abs_change_h1_pct.scale() > 2
        {
            return Err("Pump.fun filters require age 1-168 hours, USD amounts 0-1 billion and absolute change 0-10000%, with at most 2 decimals".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pair {
    pub address: String,
    pub dex: String,
    pub created_at: u64,
    pub price_usd: Decimal,
    pub liquidity_usd: Option<Decimal>,
    pub volume_h1_usd: Decimal,
    pub change_h1_pct: Decimal,
    pub checked_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub mint: String,
    pub name: String,
    pub symbol: String,
    pub event: String,
    pub first_seen_at: u64,
    pub mayhem: bool,
    pub pair: Option<Pair>,
    pub matches_filters: bool,
    pub reasons: Vec<String>,
}
impl Candidate {
    fn screen(&mut self, config: &Config, timestamp: u64, connected: bool) {
        self.reasons.clear();
        if !connected {
            self.reasons.push("PumpPortal disconnected".into());
        }
        if self.mayhem {
            self.reasons.push("Mayhem-mode token excluded".into());
        }
        if let Some(pair) = &self.pair {
            if timestamp.saturating_sub(pair.checked_at) > FRESH_SECONDS {
                self.reasons.push("Pair metrics are stale".into());
            }
            if pair.created_at > timestamp
                || timestamp.saturating_sub(pair.created_at) > config.max_pair_age_hours * 3600
            {
                self.reasons
                    .push("Pair outside configured age window".into());
            }
            match pair.liquidity_usd {
                Some(liquidity) if liquidity < config.min_liquidity_usd => {
                    self.reasons.push("Liquidity below minimum".into())
                }
                None => self.reasons.push(
                    "Liquidity unavailable; bonding-curve reserves are not pool liquidity".into(),
                ),
                _ => {}
            }
            if pair.volume_h1_usd < config.min_volume_h1_usd {
                self.reasons.push("1h volume below minimum".into());
            }
            if pair.change_h1_pct.abs() < config.min_abs_change_h1_pct {
                self.reasons
                    .push("Absolute 1h price change below minimum".into());
            }
        } else {
            self.reasons
                .push("Awaiting indexed pair with complete metrics".into());
        }
        self.matches_filters = self.reasons.is_empty();
    }
}

#[derive(Clone, Default, Serialize)]
pub struct Discovery {
    pub connected: bool,
    pub last_event_at: Option<u64>,
    pub last_refresh_at: Option<u64>,
    pub error: Option<String>,
    pub tracked: usize,
    pub matching: usize,
    pub candidates: Vec<Candidate>,
}
pub struct Adapter {
    config: watch::Sender<Config>,
    updates: watch::Receiver<Discovery>,
    worker: Option<Worker>,
}
impl Adapter {
    pub fn new(config: Config) -> Self {
        let (config_tx, config_rx) = watch::channel(config);
        let (updates_tx, updates_rx) = watch::channel(Discovery::default());
        Self {
            config: config_tx,
            updates: updates_rx,
            worker: Some(Worker {
                config: config_rx,
                updates: updates_tx,
                websocket: "wss://pumpportal.fun/api/data".into(),
                dex: "https://api.dexscreener.com".into(),
            }),
        }
    }
    pub fn configure(&self, config: Config) {
        self.config.send_if_modified(|current| {
            if *current == config {
                false
            } else {
                *current = config;
                true
            }
        });
    }
    pub fn take_worker(&mut self) -> Option<Worker> {
        self.worker.take()
    }
    #[cfg(feature = "test-support")]
    pub fn use_test_endpoint(&mut self, base: &str) {
        if let Some(worker) = &mut self.worker {
            worker.websocket = format!("{}/pump", base.replacen("http://", "ws://", 1));
            worker.dex = base.into();
        }
    }
    pub fn snapshot(&self, config: &Config) -> Discovery {
        if !config.enabled {
            return Discovery::default();
        }
        let mut result = self.updates.borrow().clone();
        if self.updates.has_changed().is_err() {
            result.connected = false;
            result.error = Some("Discovery worker stopped".into());
        }
        for candidate in &mut result.candidates {
            candidate.screen(config, now(), result.connected);
        }
        result.matching = result
            .candidates
            .iter()
            .filter(|c| c.matches_filters)
            .count();
        result
    }
}

pub struct Worker {
    config: watch::Receiver<Config>,
    updates: watch::Sender<Discovery>,
    websocket: String,
    dex: String,
}
impl Worker {
    pub async fn run(mut self) {
        let Ok(client) = Client::builder()
            .timeout(Duration::from_secs(8))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
        else {
            self.updates.send_replace(Discovery {
                error: Some("Cannot create discovery client".into()),
                ..Discovery::default()
            });
            return;
        };
        let mut tracked = VecDeque::new();
        let mut backoff = 2;
        loop {
            let config = self.config.borrow_and_update().clone();
            if !config.enabled {
                tracked.clear();
                self.updates.send_replace(Discovery::default());
                if self.config.changed().await.is_err() {
                    return;
                }
                continue;
            }
            let started = time::Instant::now();
            let result = tokio::select! {
                result = stream(&client, &self.websocket, &self.dex, &config, &mut tracked, &self.updates) => result,
                changed = self.config.changed() => { if changed.is_err() { return; } continue; }
            };
            let mut snapshot = self.updates.borrow().clone();
            snapshot.connected = false;
            snapshot.error = result.err();
            for candidate in &mut snapshot.candidates {
                candidate.screen(&config, now(), false);
            }
            snapshot.matching = 0;
            self.updates.send_replace(snapshot);
            if started.elapsed() >= Duration::from_secs(60) {
                backoff = 2;
            }
            tokio::select! {
                _ = time::sleep(Duration::from_secs(backoff)) => {},
                changed = self.config.changed() => { if changed.is_err() { return; } }
            }
            backoff = (backoff * 2).min(60);
        }
    }
}

fn label(value: &Value, key: &str, limit: usize) -> String {
    value[key]
        .as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}
fn ingest(tracked: &mut VecDeque<Candidate>, value: &Value, timestamp: u64) -> bool {
    // PumpPortal also emits other launchpads; neither a "pump" suffix nor a symbol proves provenance.
    let event = value["txType"].as_str().unwrap_or("");
    if !matches!(
        (event, value["pool"].as_str()),
        ("create", Some("pump")) | ("migrate" | "migration", Some("pump" | "pump-amm"))
    ) {
        return false;
    }
    let event = if event == "migrate" {
        "migration"
    } else {
        event
    };
    let Some(mint) = value["mint"]
        .as_str()
        .filter(|m| Pubkey::from_str(m).is_ok())
    else {
        return false;
    };
    if let Some(existing) = tracked.iter_mut().find(|c| c.mint == mint) {
        let mayhem = value["is_mayhem_mode"].as_bool().unwrap_or(false);
        let changed =
            (event == "migration" && existing.event != event) || (mayhem && !existing.mayhem);
        if event == "migration" {
            existing.event = event.into();
        }
        existing.mayhem |= mayhem;
        return changed;
    }
    if tracked.len() == CAPACITY {
        // Keep screened high-activity pairs while rotating the oldest unmatched discoveries.
        let index = tracked.iter().position(|c| !c.matches_filters).unwrap_or(0);
        tracked.remove(index);
    }
    tracked.push_back(Candidate {
        mint: mint.into(),
        name: label(value, "name", 80),
        symbol: label(value, "symbol", 24),
        event: event.into(),
        first_seen_at: timestamp,
        mayhem: value["is_mayhem_mode"].as_bool().unwrap_or(false),
        pair: None,
        matches_filters: false,
        reasons: vec![],
    });
    true
}
fn decimal(value: &Value) -> Option<Decimal> {
    match value {
        Value::String(s) => Decimal::from_str(s).ok(),
        Value::Number(n) => Decimal::from_str(&n.to_string()).ok(),
        _ => None,
    }
}
fn parse_pair(value: &Value, mint: &str, timestamp: u64) -> Option<Pair> {
    if value["chainId"] != "solana"
        || value["baseToken"]["address"] != mint
        || !matches!(value["dexId"].as_str(), Some("pumpfun" | "pumpswap"))
    {
        return None;
    }
    let address = value["pairAddress"]
        .as_str()
        .filter(|p| Pubkey::from_str(p).is_ok())?;
    let created_at = value["pairCreatedAt"].as_u64()? / 1000;
    let price_usd = decimal(&value["priceUsd"])?;
    let liquidity_usd = decimal(&value["liquidity"]["usd"]);
    let volume_h1_usd = decimal(&value["volume"]["h1"])?;
    let change_h1_pct = decimal(&value["priceChange"]["h1"])?;
    if created_at == 0
        || created_at > timestamp
        || price_usd <= Decimal::ZERO
        || liquidity_usd.is_some_and(|v| v < Decimal::ZERO)
        || volume_h1_usd < Decimal::ZERO
    {
        return None;
    }
    Some(Pair {
        address: address.into(),
        dex: value["dexId"].as_str()?.into(),
        created_at,
        price_usd,
        liquidity_usd,
        volume_h1_usd,
        change_h1_pct,
        checked_at: timestamp,
    })
}
fn enrich(tracked: &mut VecDeque<Candidate>, mints: &[String], values: &[Value], timestamp: u64) {
    for candidate in tracked.iter_mut().filter(|c| mints.contains(&c.mint)) {
        let best = values
            .iter()
            .filter_map(|v| parse_pair(v, &candidate.mint, timestamp))
            .max_by_key(|p| p.liquidity_usd);
        if let Some(pair) = &best {
            if let Some(value) = values.iter().find(|v| {
                v["pairAddress"] == pair.address && v["baseToken"]["address"] == candidate.mint
            }) {
                candidate.name = label(&value["baseToken"], "name", 80);
                candidate.symbol = label(&value["baseToken"], "symbol", 24);
            }
        }
        // Missing/delisted metrics invalidate prior evidence; no zero-filled or fabricated market data.
        candidate.pair = best;
    }
}
fn publish(
    tracked: &mut VecDeque<Candidate>,
    state: &mut Discovery,
    config: &Config,
    updates: &watch::Sender<Discovery>,
) {
    let timestamp = now();
    tracked
        .retain(|c| timestamp.saturating_sub(c.first_seen_at) <= config.max_pair_age_hours * 3600);
    for candidate in tracked.iter_mut() {
        candidate.screen(config, timestamp, state.connected);
    }
    let mut candidates: Vec<_> = tracked.iter().cloned().collect();
    candidates.sort_by_key(|c| {
        std::cmp::Reverse((
            c.matches_filters,
            c.pair.as_ref().map(|p| p.volume_h1_usd).unwrap_or_default(),
            c.first_seen_at,
        ))
    });
    state.tracked = tracked.len();
    // ponytail: rolling 300-token window; use an indexed history provider when full-universe coverage is needed.
    candidates.truncate(30);
    state.matching = candidates.iter().filter(|c| c.matches_filters).count();
    state.candidates = candidates;
    updates.send_replace(state.clone());
}
async fn stream(
    client: &Client,
    websocket: &str,
    dex: &str,
    config: &Config,
    tracked: &mut VecDeque<Candidate>,
    updates: &watch::Sender<Discovery>,
) -> Result<(), String> {
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(64 * 1024))
        .max_frame_size(Some(64 * 1024));
    let (mut socket, _) = time::timeout(
        Duration::from_secs(10),
        connect_async_with_config(websocket, Some(ws_config), false),
    )
    .await
    .map_err(|_| "PumpPortal connection timed out")?
    .map_err(|_| "PumpPortal connection unavailable")?;
    for method in ["subscribeNewToken", "subscribeMigration"] {
        time::timeout(
            Duration::from_secs(5),
            socket.send(Message::Text(json!({"method":method}).to_string().into())),
        )
        .await
        .map_err(|_| "PumpPortal subscription timed out")?
        .map_err(|_| "PumpPortal subscription unavailable")?;
    }
    let mut state = Discovery {
        connected: true,
        ..Discovery::default()
    };
    publish(tracked, &mut state, config, updates);
    let mut refresh = time::interval(Duration::from_secs(15));
    refresh.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    let mut heartbeat = time::interval(Duration::from_secs(20));
    heartbeat.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    let mut last_frame = time::Instant::now();
    let mut pending = FuturesUnordered::new();
    let mut batch_failed = false;
    loop {
        tokio::select! {
            message = socket.next() => {
                last_frame = time::Instant::now();
                match message {
                    Some(Ok(Message::Text(text))) => {
                        let value: Value = serde_json::from_str(&text).map_err(|_| "PumpPortal returned invalid JSON")?;
                        if value.get("error").is_some() { return Err("PumpPortal rejected the data subscription".into()); }
                        if ingest(tracked, &value, now()) { state.last_event_at = Some(now()); publish(tracked, &mut state, config, updates); }
                    },
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {},
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return Err("PumpPortal disconnected; reconnecting".into()),
                    _ => {}
                }
            },
            _ = heartbeat.tick() => {
                if last_frame.elapsed() > Duration::from_secs(60) { return Err("PumpPortal heartbeat expired; reconnecting".into()); }
                time::timeout(Duration::from_secs(5), socket.send(Message::Ping(vec![].into()))).await.map_err(|_| "PumpPortal heartbeat timed out")?.map_err(|_| "PumpPortal heartbeat unavailable")?;
                publish(tracked, &mut state, config, updates);
            },
            _ = refresh.tick(), if pending.is_empty() => {
                batch_failed = false;
                let mints: Vec<_> = tracked.iter().map(|c| c.mint.clone()).collect();
                for batch in mints.chunks(30) {
                    let mints = batch.to_vec();
                    let request = client.get(format!("{dex}/tokens/v1/solana/{}", mints.join(",")));
                    pending.push(async move { (mints, upstream::json(request, "DEX Screener", true, Duration::from_secs(8)).await) });
                }
            },
            Some((mints, result)) = pending.next(), if !pending.is_empty() => {
                match result {
                    Ok(value) if value.is_array() => {
                        enrich(tracked, &mints, value.as_array().expect("array checked"), now());
                        state.last_refresh_at = Some(now());
                        if !batch_failed && pending.is_empty() { state.error = None; }
                    },
                    _ => {
                        batch_failed = true;
                        for candidate in tracked.iter_mut().filter(|c| mints.contains(&c.mint)) { candidate.pair = None; }
                        state.error = Some("DEX Screener metrics unavailable; affected candidates excluded".into());
                    }
                }
                publish(tracked, &mut state, config, updates);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_domain::{USDC, WSOL};

    fn pair(timestamp: u64) -> Value {
        json!({"chainId":"solana","dexId":"pumpswap","pairAddress":USDC,"baseToken":{"address":WSOL,"name":"Test only","symbol":"TEST"},"pairCreatedAt":(timestamp-600)*1000,"priceUsd":"0.001","liquidity":{"usd":15000},"volume":{"h1":20000},"priceChange":{"h1":-12}})
    }
    #[test]
    fn filters_require_real_complete_fresh_metrics_and_exact_provenance() {
        let timestamp = now();
        let config = Config {
            enabled: true,
            ..Config::default()
        };
        let mut tracked = VecDeque::new();
        let mut event = json!({"mint":WSOL,"pool":"bonk","txType":"create","name":"Untrusted\nmetadata","symbol":"TEST"});
        assert!(!ingest(&mut tracked, &event, timestamp));
        event["pool"] = json!("pump");
        assert!(ingest(&mut tracked, &event, timestamp));
        assert!(!ingest(&mut tracked, &event, timestamp));
        assert_eq!(tracked.len(), 1);
        assert!(!tracked[0].name.contains('\n'));
        let mints = vec![WSOL.into()];
        let value = pair(timestamp);
        enrich(
            &mut tracked,
            &mints,
            std::slice::from_ref(&value),
            timestamp,
        );
        tracked[0].screen(&config, timestamp, true);
        assert!(
            tracked[0].matches_filters,
            "negative price movement counts by magnitude"
        );
        tracked[0].screen(&config, timestamp + FRESH_SECONDS + 1, true);
        assert!(!tracked[0].matches_filters);
        tracked[0].screen(&config, timestamp, false);
        assert!(!tracked[0].matches_filters);

        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove("liquidity");
        enrich(&mut tracked, &mints, &[missing], timestamp);
        tracked[0].screen(&config, timestamp, true);
        assert!(
            tracked[0].pair.is_some(),
            "display observed prices even when liquidity is absent"
        );
        assert!(tracked[0].pair.as_ref().unwrap().liquidity_usd.is_none());
        assert!(!tracked[0].matches_filters);
        for (path, replacement) in [
            ("chainId", json!("ethereum")),
            ("dexId", json!("raydium")),
            ("pairAddress", json!("../invalid")),
            ("priceUsd", json!("NaN")),
            ("volume", json!({"h1":-1})),
            ("priceChange", json!({})),
            ("pairCreatedAt", json!((timestamp + 30) * 1000)),
            ("baseToken", json!({"address":USDC})),
        ] {
            let mut invalid = value.clone();
            invalid[path] = replacement;
            assert!(parse_pair(&invalid, WSOL, timestamp).is_none());
        }
        enrich(&mut tracked, &mints, &[value], timestamp);
        tracked[0].screen(
            &Config {
                max_pair_age_hours: 1,
                ..config.clone()
            },
            timestamp + 3601,
            true,
        );
        assert!(tracked[0].reasons.iter().any(|r| r.contains("age window")));
        event["txType"] = json!("migrate");
        event["pool"] = json!("pump-amm");
        event["is_mayhem_mode"] = json!(true);
        ingest(&mut tracked, &event, timestamp);
        tracked[0].screen(&config, timestamp, true);
        assert_eq!(tracked[0].event, "migration");
        assert!(!tracked[0].matches_filters);
        enrich(&mut tracked, &mints, &[], timestamp);
        assert!(
            tracked[0].pair.is_none(),
            "a delisted pair cannot keep old metrics"
        );
        let invalid = Config {
            max_pair_age_hours: u64::MAX,
            ..config
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn window_is_bounded_and_disconnected_or_disabled_feeds_exclude_candidates() {
        let timestamp = now();
        let config = Config {
            enabled: true,
            ..Config::default()
        };
        let mut tracked = VecDeque::new();
        for index in 0..CAPACITY + 10 {
            let mut bytes = [0u8; 32];
            bytes[..8].copy_from_slice(&(index as u64).to_le_bytes());
            ingest(
                &mut tracked,
                &json!({"mint":Pubkey::new_from_array(bytes).to_string(),"pool":"pump","txType":"create"}),
                timestamp,
            );
        }
        assert_eq!(tracked.len(), CAPACITY);
        let mut adapter = Adapter::new(config.clone());
        let worker = adapter.take_worker().unwrap();
        assert!(adapter.take_worker().is_none(), "one worker per runtime");
        let mut state = Discovery {
            connected: true,
            ..Discovery::default()
        };
        publish(&mut tracked, &mut state, &config, &worker.updates);
        assert_eq!(adapter.snapshot(&config).candidates.len(), 30);
        assert_eq!(adapter.snapshot(&Config::default()).tracked, 0);
        drop(worker);
        assert!(!adapter.snapshot(&config).connected);
    }
}
