use super::*;
use kairos_domain::WSOL;
use kairos_live::{ConfirmedFill, LiveQuote, PendingTrade};

#[test]
fn live_settlements_export_linked_entry_and_exit_outcomes_for_memory() {
    let directory = std::env::temp_dir().join(format!("kairos-settlement-{}", id()));
    let mut app = Application::open(&directory).unwrap();
    app.state.mode = TradingMode::Live;
    app.state.live.public_key = Some("test-wallet".into());
    for buy in [true, false] {
        let decision_id = id();
        app.state.live.pending_decision = Some(
            json!({"id":decision_id,"model":"test-model","result":{"action":if buy {"open"} else {"close"}}}),
        );
        let quote = LiveQuote {
            input_mint: if buy { USDC.into() } else { WSOL.into() },
            output_mint: if buy { WSOL.into() } else { USDC.into() },
            input_atoms: if buy { 100_000_000 } else { 666_666_666 },
            output_atoms: if buy { 666_666_666 } else { 110_000_000 },
            min_output_atoms: if buy { 660_000_000 } else { 109_000_000 },
            price: Decimal::from(150),
            received_at: now(),
        };
        if !buy {
            app.state.live.pending_position_id = Some(app.state.positions[0].id.clone());
            app.state.positions[0].status = "ExitPending".into();
        }
        let pending = PendingTrade {
            signature: format!("test-signature-{buy}"),
            quote: quote.clone(),
            last_valid_height: 1,
            submitted_at: now(),
            scope_id: "test-wallet".into(),
        };
        app.state.live.pending = Some(pending.clone());
        app.event(
            "execution.pending",
            "Test settlement boundary: signature already durably recorded",
            "test",
        );
        app.persist(None).unwrap();
        app.record_confirmed_fill(
            pending.clone(),
            ConfirmedFill {
                signature: pending.signature,
                input_atoms: quote.input_atoms,
                output_atoms: quote.output_atoms,
                fee_lamports: 5_000,
                slot: 1,
            },
            "test",
        )
        .unwrap();
        assert_eq!(
            app.state.orders.last().unwrap().decision_id.as_deref(),
            Some(decision_id.as_str())
        );
    }
    assert_eq!(app.state.positions[0].status, "Closed");
    assert_eq!(
        app.state.positions[0].realized_pnl,
        Decimal::new(9_998425, 6)
    );
    let memory = journal::memory(&app.db, &app.journal_root).unwrap();
    assert_eq!(memory.len(), 2);
    assert!(memory
        .iter()
        .all(|r| r["event"]["mode"] == "LIVE" && r["order"]["signature"].is_string()));
    assert_eq!(memory[1]["position"]["status"], "Closed");
    assert_eq!(memory[1]["position"]["realized_pnl"], "9.998425");
    assert!(
        memory[1]["portfolio"].is_null(),
        "do not report pre-refresh wallet cash as post-fill cash"
    );
    drop(app);
    std::fs::remove_dir_all(directory).unwrap();
}
