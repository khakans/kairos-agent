//! Local HTTP contract fixtures only. These tests cannot reach Solana or Jupiter.
use super::*;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use kairos_domain::{RiskInput, RiskPolicy};
use std::sync::Mutex;

#[derive(Clone)]
struct Fixture {
    owner: String,
    calls: Arc<Mutex<Vec<String>>>,
    signature: Arc<Mutex<String>>,
    tamper: Arc<AtomicBool>,
}
fn token_account(mint: &str, owner: &str, amount: u64) -> Value {
    let mut bytes = vec![0; 165];
    bytes[..32].copy_from_slice(key(mint).unwrap().as_ref());
    bytes[32..64].copy_from_slice(key(owner).unwrap().as_ref());
    bytes[64..72].copy_from_slice(&amount.to_le_bytes());
    bytes[108] = 1;
    json!({"owner":TOKEN,"lamports":2039280,"data":[BASE64.encode(bytes),"base64"]})
}
fn build_payload(owner: &str) -> Value {
    let owner_key = key(owner).unwrap();
    let input = ata(&owner_key, &key(USDC).unwrap()).to_string();
    let output = ata(&owner_key, &key(WSOL).unwrap()).to_string();
    let mut data = vec![229, 23, 203, 151, 122, 227, 173, 42, 0, 0, 0, 0];
    data.extend(100_000_000u64.to_le_bytes());
    data.extend(700_000_000u64.to_le_bytes());
    data.extend(50u16.to_le_bytes());
    data.push(0);
    json!({"inputMint":USDC,"outputMint":WSOL,"inAmount":"100000000","outAmount":"700000000","otherAmountThreshold":"696500000","swapMode":"ExactIn","slippageBps":50,"setupInstructions":[],"cleanupInstruction":null,"otherInstructions":[],"tipInstruction":null,"addressesByLookupTableAddress":{},"swapInstruction":{"programId":JUPITER,"data":BASE64.encode(data),"accounts":[{"pubkey":TOKEN,"isSigner":false,"isWritable":false},{"pubkey":owner,"isSigner":true,"isWritable":false},{"pubkey":input,"isSigner":false,"isWritable":true},{"pubkey":output,"isSigner":false,"isWritable":true}]}})
}
async fn build(State(f): State<Fixture>) -> Json<Value> {
    f.calls.lock().unwrap().push("build".into());
    let mut payload = build_payload(&f.owner);
    if f.tamper.load(Ordering::SeqCst) {
        payload["swapInstruction"]["accounts"][3]["pubkey"] = json!(SYSTEM);
    }
    Json(payload)
}
async fn prices() -> Json<Value> {
    Json(
        json!({WSOL:{"usdPrice":142.857,"blockId":1_000_000},USDC:{"usdPrice":1,"blockId":1_000_000}}),
    )
}
async fn rpc(State(f): State<Fixture>, Json(request): Json<Value>) -> Json<Value> {
    let method = request["method"].as_str().unwrap();
    f.calls.lock().unwrap().push(method.into());
    let balance = |amount: &str, mint: &str| json!({"owner":f.owner,"mint":mint,"uiTokenAmount":{"amount":amount}});
    let result = match method {
        "getGenesisHash" => json!(MAINNET),
        "getSlot" => json!(1_000_000),
        "getMultipleAccounts" => {
            json!({"value":[{"lamports":100_000_000},token_account(USDC,&f.owner,1_000_000_000),token_account(WSOL,&f.owner,0)]})
        }
        "getLatestBlockhash" => {
            json!({"value":{"blockhash":Hash::new_unique().to_string(),"lastValidBlockHeight":1_000_150}})
        }
        "simulateTransaction" => {
            json!({"value":{"err":null,"logs":[format!("Program {JUPITER} invoke [1]"),format!("Program {WHIRLPOOL} invoke [2]")],"accounts":[token_account(USDC,&f.owner,900_000_000),token_account(WSOL,&f.owner,700_000_000),{"lamports":99_981_000}]}})
        }
        "sendTransaction" => {
            let bytes = BASE64
                .decode(request["params"][0].as_str().unwrap())
                .unwrap();
            let tx: VersionedTransaction = bincode::deserialize(&bytes).unwrap();
            assert!(
                tx.signatures[0].verify(key(&f.owner).unwrap().as_ref(), &tx.message.serialize())
            );
            assert_eq!(request["params"][1]["skipPreflight"], false);
            let signature = tx.signatures[0].to_string();
            *f.signature.lock().unwrap() = signature.clone();
            json!(signature)
        }
        "getSignatureStatuses" => json!({"value":[{"err":null,"confirmationStatus":"confirmed"}]}),
        "getTransaction" => {
            json!({"slot":1_000_001,"meta":{"err":null,"fee":19000,"preTokenBalances":[balance("1000000000",USDC),balance("0",WSOL)],"postTokenBalances":[balance("900000000",USDC),balance("700000000",WSOL)]}})
        }
        _ => panic!("Unexpected RPC call: {method}"),
    };
    Json(json!({"jsonrpc":"2.0","id":1,"result":result}))
}
fn intent(mode: TradingMode) -> ApprovedIntent {
    RiskPolicy::default()
        .approve(&RiskInput {
            mode,
            scope_id: "fixture-wallet".into(),
            paused: false,
            data_fresh: true,
            token_safe: true,
            notional: Decimal::from(100),
            fee: Decimal::ZERO,
            open_positions: 0,
            exposure: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            available_balance: Decimal::from(1000),
        })
        .unwrap()
}

#[tokio::test]
async fn live_http_pipeline_signs_only_after_validation_and_reconciles_real_deltas() {
    let signer = Keypair::new();
    let fixture = Fixture {
        owner: signer.pubkey().to_string(),
        calls: Arc::new(Mutex::new(Vec::new())),
        signature: Arc::new(Mutex::new(String::new())),
        tamper: Arc::new(AtomicBool::new(false)),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new()
        .route("/build", get(build))
        .route("/price", get(prices))
        .route("/rpc", post(rpc))
        .with_state(fixture.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let lease_path =
        std::env::temp_dir().join(format!("kairos-contract-{}", rand::random::<u64>()));
    let lease = File::create(&lease_path).unwrap();
    let kill = Arc::new(AtomicBool::new(false));
    let executor = LiveExecutor {
        client: Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap(),
        credentials: vault::Credentials {
            rpc_url: format!("http://{address}/rpc"),
            secondary_rpc_url: format!("http://{address}/rpc"),
            jupiter_api_key: "fixture-only".into(),
            keypair: signer.to_bytes().to_vec(),
        },
        signer,
        _lease: lease,
        kill: kill.clone(),
        build_endpoint: format!("http://{address}/build"),
        price_endpoint: format!("http://{address}/price"),
    };
    assert!(executor
        .prepare(intent(TradingMode::DryRun), true, 100_000_000, 50)
        .await
        .is_err());
    assert!(
        fixture.calls.lock().unwrap().is_empty(),
        "Dry Run must stop before any network call"
    );
    fixture.tamper.store(true, Ordering::SeqCst);
    assert!(executor
        .prepare(intent(TradingMode::Live), true, 100_000_000, 50)
        .await
        .is_err());
    assert!(!fixture
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|m| m == "sendTransaction"));
    fixture.tamper.store(false, Ordering::SeqCst);
    let prepared = executor
        .prepare(intent(TradingMode::Live), true, 100_000_000, 50)
        .await
        .unwrap();
    kill.store(true, Ordering::SeqCst);
    assert!(executor.sign(prepared).is_err());
    kill.store(false, Ordering::SeqCst);
    let prepared = executor
        .prepare(intent(TradingMode::Live), true, 100_000_000, 50)
        .await
        .unwrap();
    let signed = executor.sign(prepared).unwrap();
    let pending = signed.pending.clone();
    executor.submit(signed).await.unwrap();
    let fill = executor.reconcile(&pending).await.unwrap().unwrap();
    assert_eq!(fill.input_atoms, 100_000_000);
    assert_eq!(fill.output_atoms, 700_000_000);
    assert_eq!(fill.fee_lamports, 19000);
    let calls = fixture.calls.lock().unwrap();
    let submit = calls.iter().position(|m| m == "sendTransaction").unwrap();
    let simulate = calls
        .iter()
        .position(|m| m == "simulateTransaction")
        .unwrap();
    assert!(simulate < submit);
    assert_eq!(calls.iter().filter(|m| *m == "sendTransaction").count(), 1);
    drop(calls);
    drop(executor);
    server.abort();
    std::fs::remove_file(lease_path).unwrap();
}

#[test]
fn malformed_amounts_and_destinations_never_reach_signer() {
    let owner = Keypair::new().pubkey();
    let mut value = build_payload(&owner.to_string());
    let quote = LiveQuote {
        input_mint: USDC.into(),
        output_mint: WSOL.into(),
        input_atoms: 100_000_000,
        output_atoms: 700_000_000,
        min_output_atoms: 696_500_000,
        price: Decimal::from(143),
        received_at: now(),
    };
    assert!(validate_swap(
        &serde_json::from_value(value.clone()).unwrap(),
        &quote,
        &owner
    )
    .is_ok());
    value["swapInstruction"]["programId"] = json!(SYSTEM);
    assert!(validate_swap(&serde_json::from_value(value).unwrap(), &quote, &owner).is_err());
    let mut bad = quote;
    bad.input_atoms += 1;
    assert!(validate_swap(
        &serde_json::from_value(build_payload(&owner.to_string())).unwrap(),
        &bad,
        &owner
    )
    .is_err());
}
