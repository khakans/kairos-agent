//! Actual public, read-only feed diagnostic. No keys, wallet, AI charges or transactions.
use kairos_application::pumpfun::{Adapter, Config};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), String> {
    let config = Config {
        enabled: true,
        ..Config::default()
    };
    let mut adapter = Adapter::new(config.clone());
    let worker = tokio::spawn(adapter.take_worker().unwrap().run());
    let started = tokio::time::Instant::now();
    let mut passed = false;
    while started.elapsed() < Duration::from_secs(120) {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let snapshot = adapter.snapshot(&config);
        let indexed = snapshot
            .candidates
            .iter()
            .filter(|c| c.pair.is_some())
            .count();
        println!(
            "connected={} tracked={} indexed={} matching={} error={}",
            snapshot.connected,
            snapshot.tracked,
            indexed,
            snapshot.matching,
            snapshot.error.as_deref().unwrap_or("none")
        );
        if snapshot.connected && indexed > 0 {
            let candidate = snapshot
                .candidates
                .iter()
                .find(|c| c.pair.is_some())
                .unwrap();
            println!(
                "PASS actual Pump.fun mint {} / metrics source {} / screened={} / reasons={:?}",
                candidate.mint,
                candidate.pair.as_ref().unwrap().dex,
                candidate.matches_filters,
                candidate.reasons
            );
            passed = true;
            break;
        }
    }
    adapter.configure(Config::default());
    drop(adapter);
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .map_err(|_| "Discovery worker did not stop")?
        .map_err(|_| "Discovery worker failed")?;
    if passed {
        Ok(())
    } else {
        Err("No indexed token observed within 120 seconds; inspect provider availability".into())
    }
}
