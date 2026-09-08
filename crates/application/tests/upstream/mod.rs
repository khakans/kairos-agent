use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use kairos_domain::{USDC, WSOL};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
#[derive(Default)]
pub struct Control {
    pub price: AtomicU64,
    pub stale: AtomicBool,
    pub offline: AtomicBool,
    pub decision: Mutex<String>,
    pub failed_role: Mutex<String>,
    pub blocked_role: Mutex<String>,
    pub invalid_mint: AtomicBool,
    pub contexts: Mutex<Vec<Value>>,
    pub submissions: AtomicU64,
    pub active_analysts: AtomicU64,
    pub max_active_analysts: AtomicU64,
    pub slow_role: Mutex<String>,
    pub delay_ms: AtomicU64,
}
pub struct Feed {
    pub base: String,
    pub control: Arc<Control>,
}
pub async fn start() -> String {
    controlled().await.base
}
pub async fn controlled() -> Feed {
    let control = Arc::new(Control::default());
    control.price.store(150, Ordering::SeqCst);
    let router = Router::new()
        .route("/price/v3", get(prices))
        .route("/rpc", post(rpc))
        .route("/swap/v2/order", get(quote))
        .route("/chat", post(chat))
        .with_state(control.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Feed { base, control }
}
async fn prices(State(control): State<Arc<Control>>) -> Result<Json<Value>, StatusCode> {
    if control.offline.load(Ordering::SeqCst) {
        return Err(StatusCode::BAD_GATEWAY);
    }
    let block = if control.stale.load(Ordering::SeqCst) {
        999_000
    } else {
        1_000_000
    };
    Ok(Json(
        json!({WSOL:{"usdPrice":control.price.load(Ordering::SeqCst),"blockId":block},USDC:{"usdPrice":1,"blockId":block}}),
    ))
}
async fn rpc(
    State(control): State<Arc<Control>>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let result = match body["method"].as_str() {
        Some("getGenesisHash") => json!("5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"),
        Some("getSlot") => json!(1_000_000),
        Some("getMultipleAccounts") => {
            json!({"context":{"slot":1_000_000},"value":([9,6].map(|decimals|json!({"owner":"TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA","data":{"parsed":{"type":"mint","info":{"isInitialized":!control.invalid_mint.load(Ordering::SeqCst),"decimals":decimals,"supply":"1000000","mintAuthority":null,"freezeAuthority":null}}}})))})
        }
        _ => {
            control.submissions.fetch_add(1, Ordering::SeqCst);
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    Ok(Json(json!({"jsonrpc":"2.0","id":1,"result":result})))
}
async fn quote(
    State(control): State<Arc<Control>>,
    Query(query): Query<HashMap<String, String>>,
) -> Json<Value> {
    assert!(!query.contains_key("taker"));
    let atoms = query["amount"].parse::<u64>().unwrap();
    let price = control.price.load(Ordering::SeqCst);
    let out = if query["inputMint"] == USDC {
        atoms * 1000 / price
    } else {
        atoms * price / 1000
    };
    Json(
        json!({"inputMint":query["inputMint"],"outputMint":query["outputMint"],"inAmount":atoms.to_string(),"outAmount":out.to_string(),"otherAmountThreshold":(out*(10_000-query["slippageBps"].parse::<u64>().unwrap())/10_000).to_string(),"swapMode":"ExactIn","transaction":null}),
    )
}
async fn chat(State(control): State<Arc<Control>>, Json(body): Json<Value>) -> Json<Value> {
    let context: Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    control.contexts.lock().unwrap().push(context.clone());
    let role = context["agent_role"].as_str().unwrap();
    let slow = *control.slow_role.lock().unwrap() == role;
    if slow {
        tokio::time::sleep(std::time::Duration::from_millis(
            control.delay_ms.load(Ordering::SeqCst),
        ))
        .await;
    }
    if matches!(role, "market_analyst" | "onchain_analyst") {
        let active = control.active_analysts.fetch_add(1, Ordering::SeqCst) + 1;
        control
            .max_active_analysts
            .fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        control.active_analysts.fetch_sub(1, Ordering::SeqCst);
    }
    if *control.failed_role.lock().unwrap() == role {
        return Json(json!({"choices":[{"message":{"content":"invalid-json"}}]}));
    }
    if role != "strategy_evaluation" {
        if role == "onchain_analyst" {
            assert_eq!(context["onchain"]["mints"].as_array().unwrap().len(), 2);
        }
        let report = json!({"summary":format!("{role} analyzed current evidence"),"evidence":[format!("Read {} trade outcomes",context["trade_history"].as_array().unwrap().len())],"risks":["Test evidence only"],"assessment":if *control.blocked_role.lock().unwrap()==role {"block"} else {"proceed"}});
        return Json(json!({"choices":[{"message":{"content":report.to_string()}}]}));
    }
    assert_eq!(context["agent_reports"].as_object().unwrap().len(), 3);
    let selection = control.decision.lock().unwrap().clone();
    let content=match selection.as_str() {
        "close"=>json!({"action":"close","notional_usdc":"0","position_id":context["open_positions"][0]["id"],"rationale":"Evaluate previous entry and close","lessons":"Fees reduce realized returns"}).to_string(),
        "hold"=>json!({"action":"hold","notional_usdc":"0","position_id":null,"rationale":"Insufficient evidence","lessons":"Wait for better evidence"}).to_string(),
        ""=>json!({"action":"open","notional_usdc":"100","position_id":null,"rationale":"Test provider evaluates actual HTTP context","lessons":format!("Observed {} prior outcomes",context["trade_history"].as_array().unwrap().len())}).to_string(),
        _=>selection
    };
    Json(json!({"choices":[{"message":{"content":content}}]}))
}
