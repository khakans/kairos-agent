//! Explicit, real-provider diagnostic. Never creates a wallet or enables live execution.
use kairos_application::{
    models::{Action, Command},
    services::Services,
    Application,
};
use kairos_domain::TradingMode;
use rust_decimal::Decimal;
use uuid::Uuid;

async fn command(app: &mut Application, action: Action) -> Result<(), String> {
    app.command(Command {
        request_id: Uuid::new_v4().to_string(),
        action,
    })
    .await
    .map(|_| ())
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let services = Services::from_env()?;
    if std::env::args().any(|arg| arg == "--repeat-feed") {
        let mut failures = 0;
        for sample in 1..=12 {
            let started = std::time::Instant::now();
            let result = async {
                let (price, slot) = services.market().await?;
                services.onchain(slot).await?;
                Ok::<_, String>((price, slot))
            }
            .await;
            match result {
                Ok((price, slot)) => println!(
                    "PASS sample {sample}/12: price={price}, slot={slot}, elapsed={}ms",
                    started.elapsed().as_millis()
                ),
                Err(error) => {
                    failures += 1;
                    println!("FAIL sample {sample}/12: {error}");
                }
            }
            if sample < 12 {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
        return if failures == 0 {
            Ok(())
        } else {
            Err(format!("{failures}/12 actual feed samples failed"))
        };
    }
    let (price, slot) = services.market().await?;
    println!("PASS actual mainnet market: SOL/USDC={price}, slot={slot}");
    let chain = services.onchain(slot).await?;
    println!(
        "PASS actual mint accounts: {}",
        chain["mints"].as_array().unwrap().len()
    );
    let buy = services.quote(true, 10_000_000, 50, price).await?;
    let sell = services
        .quote(false, buy.min_output_atoms, 50, price)
        .await?;
    println!(
        "PASS actual quote-only buy/sell: {} WSOL atoms / {} USDC atoms",
        buy.output_atoms, sell.output_atoms
    );

    let directory = std::env::current_dir()
        .map_err(|_| "Cannot locate workspace")?
        .join(format!(".smoke-data-actual-{}", Uuid::new_v4()));
    let mut app = Application::open(&directory)?;
    println!("Diagnostic data: {}", directory.display());
    for mode in [TradingMode::Live, TradingMode::DryRun] {
        command(&mut app, Action::SetMode { mode }).await?;
        app.tick().await?;
        if let Some(error) = &app.state.market_error {
            return Err(error.clone());
        }
        assert!(!app.state.markets.is_empty());
        assert!(app.state.live.public_key.is_none());
        println!("PASS runtime {mode:?} actual feed, no wallet");
    }
    command(
        &mut app,
        Action::CreateSession {
            name: "Actual provider smoke".into(),
            starting_balance: Decimal::from(1000),
            fee_bps: 10,
            slippage_bps: 50,
        },
    )
    .await?;
    command(&mut app, Action::StartSession).await?;
    if services.ai_ready() {
        println!(
            "AI model: {}; cycle budget: {}s",
            services.model,
            services.cycle_timeout().as_secs()
        );
        let started = std::time::Instant::now();
        let result = command(&mut app, Action::RunCycle).await;
        for agent in &app.state.agents {
            println!("AGENT {}: {}", agent.name, agent.status);
        }
        println!("AI cycle elapsed: {}s", started.elapsed().as_secs());
        result?;
        println!(
            "PASS actual AI cycle: {}",
            app.state.last_decision.as_ref().unwrap()["result"]["action"]
        );
    } else {
        println!("SKIP AI cycle: configure KAIROS_AI_URL and KAIROS_AI_MODEL");
    }
    if std::env::args().any(|arg| arg == "--analysis-only") {
        assert!(app.state.orders.is_empty());
        println!("PASS analysis-only diagnostic: no proposals approved, no trades executed");
    } else if let Some(proposal) = app.state.proposals.last().cloned() {
        command(&mut app, Action::ApproveProposal { id: proposal.id }).await?;
        let position = app
            .state
            .positions
            .last()
            .ok_or("Expected virtual position")?
            .id
            .clone();
        command(&mut app, Action::ClosePosition { id: position }).await?;
        assert_eq!(app.state.orders.len(), 2);
        assert!(app
            .state
            .orders
            .iter()
            .all(|order| order.signature.is_none()));
        println!(
            "PASS actual-market dry fill and close; realized PnL={}",
            app.snapshot().portfolio.realized_pnl
        );
    } else {
        // A real model may hold. Quote checks above still exercise both market directions.
        println!("SKIP virtual execution: AI chose hold; no trade forced");
    }
    command(&mut app, Action::StopSession).await?;
    println!(
        "Journal retained at {}",
        directory.join("trade-log").display()
    );
    Ok(())
}
