//! Restricted Jupiter V2 live executor. No network submission exists in the dry executor.
pub mod vault;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use fs2::FileExt;
use kairos_domain::{ApprovedIntent, TradingMode, USDC, WSOL};
use reqwest::Client;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use solana_address_lookup_table_interface::state::AddressLookupTable;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_hash::Hash;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::{v0, AddressLookupTableAccount, VersionedMessage};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use std::{
    collections::HashMap,
    fs::File,
    path::Path,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const JUPITER: &str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const WHIRLPOOL: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
const SYSTEM: &str = "11111111111111111111111111111111";
const COMPUTE: &str = "ComputeBudget111111111111111111111111111111";
pub const MAINNET: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
pub const FEE_RESERVE_LAMPORTS: u64 = 20_000_000;
pub const MAX_FEE_LAMPORTS: u64 = 100_000;
pub fn new_keypair_bytes() -> Vec<u8> {
    Keypair::new().to_bytes().to_vec()
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn key(value: &str) -> Result<Pubkey, String> {
    Pubkey::from_str(value).map_err(|_| "Invalid public key".into())
}
fn ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), key(TOKEN).unwrap().as_ref(), mint.as_ref()],
        &key(ATA).unwrap(),
    )
    .0
}
pub fn validate_endpoint(value: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid RPC URL")?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("RPC endpoints require HTTPS without embedded credentials".into());
    }
    let host = url.host_str().ok_or("RPC hostname required")?;
    // Remote endpoints are explicit operator-owned configuration, never supplied by model output.
    if host == "localhost" || host.ends_with(".local") || host.parse::<std::net::IpAddr>().is_ok() {
        return Err("Use a public DNS hostname for RPC".into());
    }
    Ok(())
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct WalletSnapshot {
    pub public_key: String,
    pub sol_lamports: u64,
    pub usdc_atoms: u64,
    pub wsol_atoms: u64,
    pub slot: u64,
    pub checked_at: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct LiveQuote {
    pub input_mint: String,
    pub output_mint: String,
    pub input_atoms: u64,
    pub output_atoms: u64,
    pub min_output_atoms: u64,
    pub price: Decimal,
    pub received_at: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiAccount {
    pubkey: String,
    is_signer: bool,
    is_writable: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiInstruction {
    program_id: String,
    accounts: Vec<ApiAccount>,
    data: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Build {
    input_mint: String,
    output_mint: String,
    in_amount: String,
    out_amount: String,
    other_amount_threshold: String,
    swap_mode: String,
    slippage_bps: u16,
    swap_instruction: ApiInstruction,
    #[serde(default)]
    setup_instructions: Vec<ApiInstruction>,
    cleanup_instruction: Option<ApiInstruction>,
    #[serde(default)]
    other_instructions: Vec<ApiInstruction>,
    tip_instruction: Option<ApiInstruction>,
    addresses_by_lookup_table_address: Option<HashMap<String, Vec<String>>>,
}
pub struct PreparedTrade {
    pub quote: LiveQuote,
    transaction: VersionedTransaction,
    pub last_valid_height: u64,
    pub scope_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PendingTrade {
    pub signature: String,
    pub quote: LiveQuote,
    pub last_valid_height: u64,
    pub submitted_at: u64,
    pub scope_id: String,
}
pub struct SignedTrade {
    pub pending: PendingTrade,
    encoded: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ConfirmedFill {
    pub signature: String,
    pub input_atoms: u64,
    pub output_atoms: u64,
    pub fee_lamports: u64,
    pub slot: u64,
}

pub struct LiveExecutor {
    #[cfg(test)]
    build_endpoint: String,
    #[cfg(test)]
    price_endpoint: String,
    client: Client,
    credentials: vault::Credentials,
    signer: Keypair,
    _lease: File,
    kill: Arc<AtomicBool>,
}
impl LiveExecutor {
    pub fn unlock(directory: &Path, password: &str, kill: Arc<AtomicBool>) -> Result<Self, String> {
        let credentials = vault::unlock(&directory.join("vault.json"), password)?;
        validate_endpoint(&credentials.rpc_url)?;
        validate_endpoint(&credentials.secondary_rpc_url)?;
        if credentials.rpc_url == credentials.secondary_rpc_url {
            return Err("Configure two independent RPC endpoints".into());
        }
        let signer =
            Keypair::try_from(credentials.keypair.as_slice()).map_err(|_| "Invalid signer")?;
        let lease_directory = std::env::var_os("KAIROS_LEASE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("kairos-wallet-leases"));
        std::fs::create_dir_all(&lease_directory)
            .map_err(|_| "Cannot create wallet lease directory")?;
        let lease = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lease_directory.join(format!("{}.lock", signer.pubkey())))
            .map_err(|_| "Cannot open execution lease")?;
        lease
            .try_lock_exclusive()
            .map_err(|_| "Execution lease for this wallet is held by another runtime")?;
        let client = Client::builder()
            .timeout(Duration::from_secs(12))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "HTTP initialization failed")?;
        Ok(Self {
            #[cfg(test)]
            build_endpoint: "https://api.jup.ag/swap/v2/build".into(),
            #[cfg(test)]
            price_endpoint: "https://api.jup.ag/price/v3".into(),
            client,
            credentials,
            signer,
            _lease: lease,
            kill,
        })
    }
    pub fn public_key(&self) -> String {
        self.signer.pubkey().to_string()
    }
    async fn response(response: reqwest::Response) -> Result<Value, String> {
        if !response.status().is_success() {
            return Err(format!("Provider HTTP {}", response.status().as_u16()));
        }
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| "Provider read failed")? {
            if bytes.len() + chunk.len() > 2_000_000 {
                return Err("Provider response exceeds size limit".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| "Provider returned invalid JSON".into())
    }
    async fn rpc_at(&self, url: &str, method: &str, params: Value) -> Result<Value, String> {
        let response = self
            .client
            .post(url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .await
            .map_err(|_| "RPC connection failed or timed out")?;
        let data = Self::response(response).await?;
        if data.get("error").is_some() {
            return Err(format!("RPC {method} failed"));
        }
        data.get("result")
            .cloned()
            .ok_or_else(|| "Malformed RPC response".into())
    }
    async fn rpc(&self, method: &str, params: Value) -> Result<Value, String> {
        self.rpc_at(&self.credentials.rpc_url, method, params).await
    }
    pub async fn wallet(&self) -> Result<WalletSnapshot, String> {
        for endpoint in [
            &self.credentials.rpc_url,
            &self.credentials.secondary_rpc_url,
        ] {
            let genesis = self.rpc_at(endpoint, "getGenesisHash", json!([])).await?;
            if genesis.as_str() != Some(MAINNET) {
                return Err(
                    "Jupiter live execution requires Solana mainnet; RPC cluster mismatch".into(),
                );
            }
        }
        let primary = self
            .rpc("getSlot", json!([{"commitment":"confirmed"}]))
            .await?
            .as_u64()
            .ok_or("Invalid primary slot")?;
        let secondary = self
            .rpc_at(
                &self.credentials.secondary_rpc_url,
                "getSlot",
                json!([{"commitment":"confirmed"}]),
            )
            .await?
            .as_u64()
            .ok_or("Invalid secondary slot")?;
        if primary.abs_diff(secondary) > 32 {
            return Err("RPC slot divergence exceeds 32 slots".into());
        }
        let addresses = [
            self.public_key(),
            ata(&self.signer.pubkey(), &key(USDC)?).to_string(),
            ata(&self.signer.pubkey(), &key(WSOL)?).to_string(),
        ];
        let accounts = self.rpc("getMultipleAccounts", json!([addresses,{"encoding":"base64","commitment":"confirmed","minContextSlot":primary}])).await?;
        let values = accounts["value"]
            .as_array()
            .ok_or("Missing wallet accounts")?;
        if values.len() != 3 {
            return Err("Missing wallet accounts".into());
        }
        let sol_lamports = values[0]["lamports"]
            .as_u64()
            .ok_or("Wallet is not funded")?;
        let usdc_atoms = token_amount(&values[1], USDC, &self.public_key())?;
        let wsol_atoms = token_amount(&values[2], WSOL, &self.public_key())?;
        Ok(WalletSnapshot {
            public_key: self.public_key(),
            sol_lamports,
            usdc_atoms,
            wsol_atoms,
            slot: primary,
            checked_at: now(),
        })
    }
    async fn build(
        &self,
        buy: bool,
        amount: u64,
        slippage: u16,
    ) -> Result<(Build, LiveQuote), String> {
        let (input, output) = if buy { (USDC, WSOL) } else { (WSOL, USDC) };
        let response = self
            .client
            .get({
                #[cfg(test)]
                {
                    self.build_endpoint.as_str()
                }
                #[cfg(not(test))]
                {
                    "https://api.jup.ag/swap/v2/build"
                }
            })
            .header("x-api-key", &self.credentials.jupiter_api_key)
            .query(&[
                ("inputMint", input.to_string()),
                ("outputMint", output.to_string()),
                ("amount", amount.to_string()),
                ("taker", self.public_key()),
                ("slippageBps", slippage.to_string()),
                ("wrapAndUnwrapSol", "false".into()),
                ("onlyDirectRoutes", "true".into()),
                ("dexes", "Whirlpool".into()),
            ])
            .send()
            .await
            .map_err(|_| "Jupiter connection failed or timed out")?;
        let build: Build = serde_json::from_value(Self::response(response).await?)
            .map_err(|_| "Jupiter build schema is unsupported")?;
        let input_atoms = build
            .in_amount
            .parse::<u64>()
            .map_err(|_| "Invalid quote input")?;
        let output_atoms = build
            .out_amount
            .parse::<u64>()
            .map_err(|_| "Invalid quote output")?;
        let min_output_atoms = build
            .other_amount_threshold
            .parse::<u64>()
            .map_err(|_| "Invalid minimum output")?;
        if build.input_mint != input
            || build.output_mint != output
            || input_atoms != amount
            || build.swap_mode != "ExactIn"
            || build.slippage_bps > slippage
            || output_atoms == 0
            || min_output_atoms == 0
            || min_output_atoms > output_atoms
        {
            return Err("Jupiter quote does not match the authorized intent".into());
        }
        let minimum = u128::from(output_atoms) * u128::from(10000 - slippage) / 10000;
        if u128::from(min_output_atoms) < minimum {
            return Err("Quote exceeds slippage policy".into());
        }
        let price = if buy {
            Decimal::from(input_atoms) * Decimal::from(1000) / Decimal::from(output_atoms)
        } else {
            Decimal::from(output_atoms) * Decimal::from(1000) / Decimal::from(input_atoms)
        };
        Ok((
            build,
            LiveQuote {
                input_mint: input.into(),
                output_mint: output.into(),
                input_atoms,
                output_atoms,
                min_output_atoms,
                price,
                received_at: now(),
            },
        ))
    }
    pub async fn quote(&self, buy: bool, amount: u64, slippage: u16) -> Result<LiveQuote, String> {
        Ok(self.build(buy, amount, slippage).await?.1)
    }
    async fn validate_reference_price(&self, quote: &LiveQuote, slot: u64) -> Result<(), String> {
        let response = self
            .client
            .get({
                #[cfg(test)]
                {
                    self.price_endpoint.as_str()
                }
                #[cfg(not(test))]
                {
                    "https://api.jup.ag/price/v3"
                }
            })
            .header("x-api-key", &self.credentials.jupiter_api_key)
            .query(&[("ids", format!("{WSOL},{USDC}"))])
            .send()
            .await
            .map_err(|_| "Reference price request failed")?;
        let prices = Self::response(response).await?;
        let read_price = |mint: &str| -> Result<Decimal, String> {
            let price = &prices[mint];
            let source_slot = price["blockId"]
                .as_u64()
                .ok_or("Reference source slot missing")?;
            if slot.abs_diff(source_slot) > 64 {
                return Err("Reference price is stale or diverges from RPC slot".into());
            }
            let value = Decimal::from_str(&price["usdPrice"].to_string())
                .map_err(|_| "Reference price missing or invalid")?;
            if value <= Decimal::ZERO {
                return Err("Reference price must be positive".into());
            }
            Ok(value)
        };
        let reference = read_price(WSOL)? / read_price(USDC)?;
        if (quote.price / reference - Decimal::ONE).abs() > Decimal::new(150, 4) {
            return Err(
                "Executable quote deviates more than 150 bps from fresh reference price".into(),
            );
        }
        Ok(())
    }
    pub async fn prepare(
        &self,
        intent: ApprovedIntent,
        buy: bool,
        amount: u64,
        slippage: u16,
    ) -> Result<PreparedTrade, String> {
        if intent.mode() != TradingMode::Live {
            return Err(
                "ModeCapabilityViolation: dry-run intent cannot enter live execution".into(),
            );
        }
        if (buy && intent.exit_atoms().is_some()) || (!buy && intent.exit_atoms() != Some(amount)) {
            return Err("Intent side or exit amount mismatch".into());
        }
        if buy && (intent.notional() * Decimal::from(1_000_000)).to_u64() != Some(amount) {
            return Err("Intent amount mismatch".into());
        }
        let wallet = self.wallet().await?;
        if wallet.sol_lamports < FEE_RESERVE_LAMPORTS + MAX_FEE_LAMPORTS {
            return Err("Insufficient SOL fee reserve (minimum 0.0201 SOL)".into());
        }
        if (buy && wallet.usdc_atoms < amount) || (!buy && wallet.wsol_atoms < amount) {
            return Err("Insufficient confirmed token balance".into());
        }
        let (build, quote) = self.build(buy, amount, slippage).await?;
        self.validate_reference_price(&quote, wallet.slot).await?;
        let swap = validate_swap(&build, &quote, &self.signer.pubkey())?;
        let mut lookups = Vec::new();
        for address in build
            .addresses_by_lookup_table_address
            .unwrap_or_default()
            .keys()
        {
            let account = self
                .rpc(
                    "getAccountInfo",
                    json!([address,{"encoding":"base64","commitment":"confirmed"}]),
                )
                .await?;
            if account["value"]["owner"].as_str()
                != Some("AddressLookupTab1e1111111111111111111111111")
            {
                return Err("Invalid lookup table owner".into());
            }
            let bytes = BASE64
                .decode(
                    account["value"]["data"][0]
                        .as_str()
                        .ok_or("Missing lookup table")?,
                )
                .map_err(|_| "Invalid lookup table encoding")?;
            let table =
                AddressLookupTable::deserialize(&bytes).map_err(|_| "Invalid lookup table")?;
            if table.meta.deactivation_slot != u64::MAX {
                return Err("Deactivated lookup table".into());
            }
            lookups.push(AddressLookupTableAccount {
                key: key(address)?,
                addresses: table.addresses.to_vec(),
            });
        }
        let hash = self
            .rpc("getLatestBlockhash", json!([{"commitment":"confirmed"}]))
            .await?;
        let blockhash = Hash::from_str(
            hash["value"]["blockhash"]
                .as_str()
                .ok_or("Missing blockhash")?,
        )
        .map_err(|_| "Invalid blockhash")?;
        let last_valid_height = hash["value"]["lastValidBlockHeight"]
            .as_u64()
            .ok_or("Missing block height")?;
        // Provider setup/cleanup instructions are never trusted. Both ATAs must already exist.
        let instructions = [
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            ComputeBudgetInstruction::set_compute_unit_price(10_000),
            swap,
        ];
        let message =
            v0::Message::try_compile(&self.signer.pubkey(), &instructions, &lookups, blockhash)
                .map_err(|_| "Transaction compilation failed")?;
        if message.header.num_required_signatures != 1 {
            return Err("Unexpected transaction signer".into());
        }
        let transaction = VersionedTransaction {
            signatures: vec![Signature::default()],
            message: VersionedMessage::V0(message),
        };
        let bytes = bincode::serialize(&transaction).map_err(|_| "Transaction encoding failed")?;
        if bytes.len() > 1232 {
            return Err("Transaction exceeds Solana packet limit".into());
        }
        let source = ata(&self.signer.pubkey(), &key(&quote.input_mint)?).to_string();
        let destination = ata(&self.signer.pubkey(), &key(&quote.output_mint)?).to_string();
        let simulation = self.rpc("simulateTransaction", json!([BASE64.encode(bytes),{"encoding":"base64","commitment":"confirmed","sigVerify":false,"accounts":{"encoding":"base64","addresses":[source,destination,self.public_key()]}}])).await?;
        validate_simulation(&simulation, &quote, &wallet, &self.public_key())?;
        if now().saturating_sub(quote.received_at) > 15 {
            return Err("Quote expired during simulation".into());
        }
        Ok(PreparedTrade {
            quote,
            transaction,
            last_valid_height,
            scope_id: intent.scope_id().into(),
        })
    }
    pub fn sign(&self, prepared: PreparedTrade) -> Result<SignedTrade, String> {
        if self.kill.load(Ordering::SeqCst) {
            return Err("Kill switch is active".into());
        }
        if now().saturating_sub(prepared.quote.received_at) > 15 {
            return Err("Quote expired before signing".into());
        }
        let transaction =
            VersionedTransaction::try_new(prepared.transaction.message, &[&self.signer])
                .map_err(|_| "Signing failed")?;
        let signature = transaction.signatures[0].to_string();
        let encoded = BASE64.encode(
            bincode::serialize(&transaction).map_err(|_| "Signed transaction encoding failed")?,
        );
        Ok(SignedTrade {
            pending: PendingTrade {
                signature,
                quote: prepared.quote,
                last_valid_height: prepared.last_valid_height,
                submitted_at: now(),
                scope_id: prepared.scope_id,
            },
            encoded,
        })
    }
    /// Caller MUST durably record `pending` before invoking this method. No automatic re-sign/retry.
    pub async fn submit(&self, signed: SignedTrade) -> Result<(), String> {
        if self.kill.load(Ordering::SeqCst) {
            return Err("Kill switch active before submission; reconcile stored signature".into());
        }
        if now().saturating_sub(signed.pending.quote.received_at) > 15 {
            return Err("Quote expired before submission; reconcile stored signature".into());
        }
        let result = self.rpc("sendTransaction", json!([signed.encoded,{"encoding":"base64","skipPreflight":false,"preflightCommitment":"confirmed","maxRetries":0}])).await?;
        if result.as_str() != Some(&signed.pending.signature) {
            return Err("Submission response signature mismatch; reconciliation required".into());
        }
        Ok(())
    }
    pub async fn reconcile(&self, pending: &PendingTrade) -> Result<Option<ConfirmedFill>, String> {
        let status = self
            .rpc(
                "getSignatureStatuses",
                json!([[pending.signature],{"searchTransactionHistory":true}]),
            )
            .await?;
        let value = &status["value"][0];
        if value.is_null() {
            return Ok(None);
        }
        if !value["err"].is_null() {
            return Err(
                "Transaction failed on-chain; review signature before clearing pending state"
                    .into(),
            );
        }
        if !matches!(
            value["confirmationStatus"].as_str(),
            Some("confirmed" | "finalized")
        ) {
            return Ok(None);
        }
        let tx = self.rpc("getTransaction", json!([pending.signature,{"encoding":"jsonParsed","commitment":"confirmed","maxSupportedTransactionVersion":0}])).await?;
        if tx.is_null() {
            return Ok(None);
        }
        if !tx["meta"]["err"].is_null() {
            return Err("Confirmed transaction contains an error".into());
        }
        let owner = self.public_key();
        let balance = |field: &str, mint: &str| -> Result<u64, String> {
            let list = tx["meta"][field]
                .as_array()
                .ok_or("Missing reconciliation balances")?;
            list.iter()
                .filter(|v| v["owner"].as_str() == Some(&owner) && v["mint"].as_str() == Some(mint))
                .try_fold(0u64, |sum, v| {
                    sum.checked_add(
                        v["uiTokenAmount"]["amount"]
                            .as_str()
                            .ok_or("Missing token amount")?
                            .parse::<u64>()
                            .map_err(|_| "Invalid token amount")?,
                    )
                    .ok_or_else(|| "Balance overflow".into())
                })
        };
        let input_atoms = balance("preTokenBalances", &pending.quote.input_mint)?
            .checked_sub(balance("postTokenBalances", &pending.quote.input_mint)?)
            .ok_or("Input reconciliation mismatch")?;
        let output_atoms = balance("postTokenBalances", &pending.quote.output_mint)?
            .checked_sub(balance("preTokenBalances", &pending.quote.output_mint)?)
            .ok_or("Output reconciliation mismatch")?;
        let fee_lamports = tx["meta"]["fee"]
            .as_u64()
            .ok_or("Missing transaction fee")?;
        if input_atoms != pending.quote.input_atoms
            || output_atoms < pending.quote.min_output_atoms
            || fee_lamports > MAX_FEE_LAMPORTS
        {
            return Err("Actual balance effects violate intent; reconciliation required".into());
        }
        Ok(Some(ConfirmedFill {
            signature: pending.signature.clone(),
            input_atoms,
            output_atoms,
            fee_lamports,
            slot: tx["slot"].as_u64().ok_or("Missing confirmed slot")?,
        }))
    }
}

fn token_amount(account: &Value, mint: &str, owner: &str) -> Result<u64, String> {
    if account.is_null() {
        return Err("Required token account missing. Create and fund USDC and wrapped SOL ATAs before live activation".into());
    }
    if account["owner"].as_str() != Some(TOKEN) {
        return Err("Unsupported token account owner".into());
    }
    let data = BASE64
        .decode(
            account["data"][0]
                .as_str()
                .ok_or("Missing token account data")?,
        )
        .map_err(|_| "Invalid token account encoding")?;
    if data.len() != 165
        || &data[0..32] != key(mint)?.as_ref()
        || &data[32..64] != key(owner)?.as_ref()
        || data[108] != 1
    {
        return Err("Token account identity or state mismatch".into());
    }
    if data[72..76] != [0, 0, 0, 0] || data[129..133] != [0, 0, 0, 0] {
        return Err("Delegated token accounts are not supported".into());
    }
    Ok(u64::from_le_bytes(
        data[64..72].try_into().map_err(|_| "Invalid amount")?,
    ))
}

fn validate_swap(build: &Build, quote: &LiveQuote, owner: &Pubkey) -> Result<Instruction, String> {
    if !build.setup_instructions.is_empty()
        || build.cleanup_instruction.is_some()
        || !build.other_instructions.is_empty()
        || build.tip_instruction.is_some()
    {
        return Err("Unsupported setup/cleanup/tip instructions. Use existing funded USDC and wrapped SOL ATAs".into());
    }
    let ix = &build.swap_instruction;
    if ix.program_id != JUPITER || ix.accounts.len() > 64 {
        return Err("Swap program is not allowlisted".into());
    }
    let data = BASE64
        .decode(&ix.data)
        .map_err(|_| "Invalid instruction encoding")?;
    if data.len() < 28 {
        return Err("Unsupported Jupiter instruction".into());
    }
    // Anchor discriminators: route and shared_accounts_route only; exact-output and ledger variants fail closed.
    let route = [229, 23, 203, 151, 122, 227, 173, 42];
    let shared = [193, 32, 155, 51, 65, 214, 156, 129];
    let (authority, source, destination) = if data[..8] == route {
        (1, 2, 3)
    } else if data[..8] == shared {
        (2, 3, 6)
    } else {
        return Err("Unsupported Jupiter route discriminator".into());
    };
    let tail = &data[data.len() - 19..];
    if u64::from_le_bytes(tail[..8].try_into().unwrap()) != quote.input_atoms
        || u64::from_le_bytes(tail[8..16].try_into().unwrap()) != quote.output_atoms
        || u16::from_le_bytes(tail[16..18].try_into().unwrap()) != build.slippage_bps
        || tail[18] != 0
    {
        return Err("Swap instruction amounts or fee differ from quote".into());
    }
    let expected_source = ata(owner, &key(&quote.input_mint)?).to_string();
    let expected_destination = ata(owner, &key(&quote.output_mint)?).to_string();
    if ix.accounts.get(authority).map(|a| a.pubkey.as_str()) != Some(&owner.to_string())
        || ix.accounts.get(source).map(|a| a.pubkey.as_str()) != Some(&expected_source)
        || ix.accounts.get(destination).map(|a| a.pubkey.as_str()) != Some(&expected_destination)
    {
        return Err("Swap authority or token destination mismatch".into());
    }
    let accounts: Result<Vec<_>, String> = ix
        .accounts
        .iter()
        .map(|a| {
            let pubkey = key(&a.pubkey)?;
            if a.is_signer && pubkey != *owner {
                return Err("Unexpected instruction signer".into());
            }
            Ok(if a.is_writable {
                AccountMeta::new(pubkey, a.is_signer)
            } else {
                AccountMeta::new_readonly(pubkey, a.is_signer)
            })
        })
        .collect();
    Ok(Instruction {
        program_id: key(JUPITER)?,
        accounts: accounts?,
        data,
    })
}
fn validate_simulation(
    simulation: &Value,
    quote: &LiveQuote,
    wallet: &WalletSnapshot,
    owner: &str,
) -> Result<(), String> {
    let value = &simulation["value"];
    if !value["err"].is_null() {
        return Err("On-chain simulation failed".into());
    }
    let logs = value["logs"].as_array().ok_or("Missing simulation logs")?;
    for log in logs {
        let log = log.as_str().ok_or("Invalid simulation log")?;
        if log.starts_with("Program ") && log.contains(" invoke [") {
            let program = log
                .split_whitespace()
                .nth(1)
                .ok_or("Invalid invocation log")?;
            if ![JUPITER, TOKEN, ATA, WHIRLPOOL, SYSTEM, COMPUTE].contains(&program) {
                return Err("Simulation invoked a program outside the allowlist".into());
            }
        }
    }
    let accounts = value["accounts"]
        .as_array()
        .ok_or("Missing simulated balance effects")?;
    if accounts.len() != 3 {
        return Err("Missing simulated accounts".into());
    }
    let before_input = if quote.input_mint == USDC {
        wallet.usdc_atoms
    } else {
        wallet.wsol_atoms
    };
    let before_output = if quote.output_mint == USDC {
        wallet.usdc_atoms
    } else {
        wallet.wsol_atoms
    };
    let after_input = token_amount(&accounts[0], &quote.input_mint, owner)?;
    let after_output = token_amount(&accounts[1], &quote.output_mint, owner)?;
    let after_sol = accounts[2]["lamports"]
        .as_u64()
        .ok_or("Missing simulated SOL balance")?;
    if before_input.checked_sub(after_input) != Some(quote.input_atoms)
        || after_output.saturating_sub(before_output) < quote.min_output_atoms
        || wallet.sol_lamports.saturating_sub(after_sol) > MAX_FEE_LAMPORTS
        || after_sol < FEE_RESERVE_LAMPORTS
    {
        return Err("Simulation balance effects violate authorized spend or minimum output".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_and_simulation_fail_closed() {
        assert!(
            Hash::from_str(MAINNET).is_ok(),
            "Genesis hash must be a full 32-byte hash"
        );
        assert!(validate_endpoint("http://localhost:8899").is_err());
        assert!(validate_endpoint("https://user:secret@example.com").is_err());
        assert!(validate_endpoint("https://api.mainnet-beta.solana.com").is_ok());
        let quote = LiveQuote {
            input_mint: USDC.into(),
            output_mint: WSOL.into(),
            input_atoms: 1,
            output_atoms: 1,
            min_output_atoms: 1,
            price: Decimal::ONE,
            received_at: now(),
        };
        assert!(validate_simulation(
            &json!({"value":{"err":null,"logs":["Program malicious invoke [1]"]}}),
            &quote,
            &WalletSnapshot::default(),
            SYSTEM
        )
        .is_err());
    }
}

#[cfg(test)]
mod pipeline_tests;
