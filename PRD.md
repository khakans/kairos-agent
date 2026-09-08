# Product Requirements Document (PRD)

## Kairos Agent — Multi-Agent AI Crypto Trading Automation on Solana

> **Pembaruan sumber data dan pembelajaran (instruksi owner):** Dry Run dan Live sama-sama memakai live market data. Dry Run hanya mengestimasi eksekusi tanpa transaksi on-chain. Seluruh eksekusi kedua mode dicatat di `trade-log/YYYY/MM/DD/` dan outcome log dibaca agent AI sebagai konteks keputusan open/close berikutnya. Detail format, timezone, dan konfigurasi provider ada di `README.md`.

> **Keputusan implementasi 8 September 2026 (instruksi owner):** produk menyediakan tepat dua mode, `Dry Run` dan `Live`. Referensi Observe, Paper, dan Shadow di baseline berikut menjadi konteks roadmap/validasi, bukan mode yang diekspos aplikasi. Implementasi serta batas validasi saat ini dicatat di `README.md` dan `IMPLEMENTATION.md`. Gate live pada bagian 44 tetap berlaku; permintaan mengimplementasikan Live bukan otorisasi menggunakan dana selama pengembangan.

| Atribut | Nilai |
|---|---|
| Status dokumen | Draft teknis awal / living document |
| Versi | 0.2.0 |
| Tanggal | 8 September 2026 |
| Pemilik produk | Khairul Kanters |
| Nama kerja produk | Kairos Agent |
| Target jaringan awal | Solana Mainnet dan Devnet |
| Target aset awal | Spot token pada jaringan Solana |
| Core application | Rust |
| Desktop host | Tauri 2 |
| Web frontend | React + TypeScript + shadcn/ui |
| Server host | Rust + Axum, dikemas sebagai Docker image |
| Mode deployment | Desktop standalone, desktop remote client, dan Docker headless server |

### Riwayat Revisi

| Versi | Tanggal | Perubahan |
|---|---|---|
| 0.1.0 | 8 September 2026 | Baseline arsitektur dan kebutuhan produk. |
| 0.2.0 | 8 September 2026 | Menambahkan mode Dry Run yang dapat digunakan tanpa koneksi wallet, beserta runtime contract, virtual account, API, UI, persistence, security guard, testing, dan acceptance criteria. Seluruh scope versi 0.1.0 tetap dipertahankan. |

---

## 1. Ringkasan Eksekutif

Kairos Agent adalah aplikasi otomatisasi trading kripto berbasis multi-agent AI yang berfokus pada eksekusi spot on-chain di jaringan Solana. Sistem menerima data pasar real-time, menggabungkan market structure, aktivitas on-chain, smart-money/whale intelligence, referensi harga, dan keluaran model bahasa untuk menghasilkan proposal transaksi. Semua tindakan yang dapat memindahkan dana harus melewati risk engine dan execution pipeline deterministik yang ditulis dalam Rust.

Produk menyediakan tiga cara penggunaan:

1. **Desktop standalone**: UI Tauri, Rust core, database, adapter, dan agent runtime berjalan pada komputer pengguna; wallet signer hanya diaktifkan untuk mode yang membutuhkannya.
2. **Desktop remote client**: Tauri hanya menjadi client yang terhubung ke runtime Docker pada server.
3. **Web dashboard**: React/shadcn UI disajikan oleh Rust server pada satu port bersama REST API dan WebSocket.

Sistem tidak memberikan akses signing langsung kepada LLM. AI hanya menghasilkan analisis dan `TradeProposal` terstruktur. Risk engine deterministik memiliki hak veto. Transaction builder, simulator, signer, submitter, confirmation tracker, position supervisor, dan portfolio ledger juga deterministik.

Telegram Bot tersedia sebagai adapter komunikasi untuk notifikasi, monitoring, penjelasan keputusan, approval opsional, dan tindakan pengurangan risiko. Telegram tidak pernah memegang private key dan tidak boleh memiliki endpoint withdrawal.

Target utama versi pertama adalah membangun runtime yang dapat diobservasi, dapat direplay, aman untuk paper trading, serta dapat ditingkatkan secara bertahap menuju live trading dengan modal terbatas.

Produk juga menyediakan **Dry Run** sebagai jalur validasi end-to-end tanpa kewajiban menghubungkan wallet. Dalam mode ini, market data dan agent runtime berjalan nyata, tetapi modal, balance, order, fill, dan posisi direpresentasikan oleh akun virtual. Runtime secara keras melarang signing dan submission on-chain sehingga pengguna dapat menguji konfigurasi adapter, strategi, orchestration, risk policy, serta lifecycle posisi sebelum menyiapkan wallet.

> **Batas produk:** produk ini merupakan perangkat lunak otomasi dan analitik. Sistem tidak menjamin keuntungan, tidak menghilangkan risiko pasar, dan tidak boleh mengaktifkan live trading sebelum kontrol risiko serta pengujian yang ditetapkan dalam dokumen ini terpenuhi.

---

## 2. Visi Produk

Membangun sebuah command center trading Solana yang:

- bekerja otomatis dan dapat berjalan 24/7;
- menggunakan beberapa AI agent dengan tanggung jawab yang jelas;
- mempunyai sumber data real-time dan redundant;
- dapat membuka, mengelola, dan menutup banyak posisi;
- transparan mengenai alasan setiap keputusan;
- dapat diaudit dan direplay dari data historis;
- memiliki kontrol risiko yang tidak bergantung pada LLM;
- dapat dipasang sebagai desktop application maupun server Docker;
- dapat dipantau dari browser, Tauri, dan Telegram;
- memungkinkan adapter data, LLM, Telegram, dan wallet dikonfigurasi dari UI.

---

## 3. Objective Utama

Objective produk bukan “memaksimalkan profit tanpa batas”, melainkan:

> Menjalankan strategi trading kripto secara otomatis, terukur, dapat diaudit, dan risk-adjusted, dengan perlindungan modal ketika data, model, provider, jaringan, atau sistem mengalami kegagalan.

Ukuran keberhasilan teknis utama:

- tidak ada transaksi tanpa `RiskApproved` decision;
- tidak ada private key yang keluar dari signer boundary;
- tidak ada proposal yang dieksekusi menggunakan data atau quote kedaluwarsa;
- posisi dan saldo internal selalu dapat direkonsiliasi dengan state on-chain;
- adapter failure tidak menyebabkan aplikasi crash secara keseluruhan;
- seluruh state transition kritis tercatat di audit log;
- mode dry run, paper, dan live menggunakan pipeline domain yang sama, dengan execution capability yang berbeda;
- dry run dapat mencapai status ready tanpa wallet, signer, fee reserve, atau execution lease;
- satu wallet hanya dikontrol oleh satu execution runtime aktif;
- UI dapat menampilkan aktivitas agent dan execution pipeline secara real-time.

---

## 4. Goals dan Non-Goals

### 4.1 Goals

1. Mendukung trading spot token Solana melalui on-chain liquidity.
2. Mendukung multi-position dan beberapa strategi secara simultan.
3. Mengambil market data melalui Solana WebSocket dan/atau Yellowstone gRPC.
4. Mendukung protocol decoder untuk DEX yang dipilih.
5. Menggunakan Jupiter sebagai quote/routing layer awal.
6. Menggunakan Pyth dan Jupiter Price sebagai reference-price layer.
7. Menggunakan Jito dan Solana RPC sebagai transaction submission route.
8. Menyediakan adapter GMGN dan wallet intelligence sebagai enrichment opsional.
9. Menyediakan adapter LLM remote maupun lokal.
10. Menyediakan Telegram adapter yang dapat dikonfigurasi dari UI.
11. Menyediakan dedicated automation wallet untuk unattended execution.
12. Mendukung Tauri desktop dan Docker server dengan shared Rust core.
13. Expose satu HTTP port untuk SPA, REST API, WebSocket, health endpoint, dan optional Telegram webhook.
14. Menyediakan dry run tanpa wallet, paper trading, observe-only, shadow, dan live modes.
15. Menyediakan backtesting, replay, dan post-trade evaluation dalam roadmap produk.

### 4.2 Non-Goals Versi Awal

- Custodial service publik yang menyimpan dana banyak pengguna.
- Withdrawal otomatis melalui Telegram.
- Leverage/perpetual futures pada MVP.
- Cross-chain execution pada MVP.
- High-frequency market making dengan colocated validator pada MVP.
- Menjamin profit atau win rate tertentu.
- Memberikan LLM hak untuk menandatangani atau mengirim arbitrary transaction.
- Menjalankan beberapa server aktif yang mengeksekusi wallet sama tanpa distributed execution lease.
- Social trading marketplace publik pada MVP.
- Self-modifying strategy yang langsung mengubah parameter produksi tanpa backtest dan approval.

---

## 5. Prinsip Arsitektur

### 5.1 AI proposes, deterministic core disposes

AI agent hanya boleh:

- menganalisis konteks;
- menilai evidence;
- membuat recommendation;
- menghasilkan trade thesis;
- menghasilkan `TradeProposal` yang tervalidasi schema;
- mengevaluasi hasil transaksi.

AI agent tidak boleh:

- membaca seed phrase atau private key;
- meminta signer menandatangani arbitrary bytes;
- mengubah risk policy produksi secara langsung;
- melakukan withdrawal;
- menganggap transaksi berhasil hanya karena request submit diterima;
- melewati freshness, liquidity, simulation, atau program allowlist checks.

### 5.2 Adapter bukan agent

Adapter bertugas menyediakan konektivitas dan normalisasi capability. Agent menggunakan domain data yang telah dinormalisasi dan tidak bergantung langsung pada format provider.

### 5.3 Satu domain core, beberapa host

Seluruh business logic berada di shared Rust crates. Tauri dan Axum adalah transport/host, bukan tempat implementasi logic trading.

### 5.4 Fail safe

Jika sistem tidak dapat memastikan kebenaran data, saldo, quote, atau status transaksi, tindakan default adalah:

- tidak membuka posisi baru;
- menjaga exit rule deterministik tetap aktif;
- masuk `SafeMode` jika state tidak dapat direkonsiliasi;
- menampilkan penyebab block di UI dan Telegram.

### 5.5 Observable by design

Semua keputusan harus memiliki:

- correlation ID;
- cycle ID;
- strategy ID;
- agent-run IDs;
- source adapter IDs;
- source slots/timestamps;
- evidence;
- risk decision;
- transaction signature jika ada;
- final reconciliation result.

### 5.6 Wallet-Optional Validation

Wallet merupakan dependency untuk eksekusi on-chain, bukan dependency untuk menjalankan intelligence pipeline. Observe, Dry Run, dan Paper dapat melakukan discovery, market hydration, agent analysis, proposal generation, serta deterministic risk evaluation tanpa wallet yang terhubung. Runtime harus melakukan capability gating per mode; ketiadaan wallet tidak boleh membuat market/agent runtime berstatus gagal jika mode aktif tidak memerlukan signing.

Pada Dry Run, pemisahan harus bersifat structural: execution dispatcher hanya memiliki capability `Simulate`, sedangkan capability `Sign` dan `Submit` tidak diregistrasikan. Pembatasan ini tidak boleh hanya bergantung pada tombol UI.

---

## 6. Persona dan Role

### 6.1 Owner

Memiliki seluruh hak konfigurasi:

- mengelola wallet;
- mengelola adapter dan credential;
- mengatur risk policy;
- mengaktifkan live mode;
- mengelola user;
- menutup posisi;
- menjalankan emergency control;
- melihat audit log dan credential metadata.

### 6.2 Operator

- melihat dashboard;
- melihat agent dan adapter;
- pause trading;
- reject proposal;
- menutup posisi jika diizinkan;
- tidak dapat melihat secret;
- tidak dapat mengganti wallet atau live risk limits tanpa izin.

### 6.3 Viewer

- read-only dashboard;
- portfolio, posisi, agent, market, dan audit summary;
- tidak dapat melakukan command yang mengubah state.

### 6.4 Telegram Principal

Telegram user dipetakan ke salah satu role terbatas di atas. Identitas utama adalah numeric Telegram user ID, bukan username.

---

## 7. Mode Deployment

### 7.1 Desktop Standalone

```text
React/shadcn UI
    ↕ Tauri IPC/events
Tauri Host
    ↓
Shared Rust Application Core
    ├── Adapter runtime
    ├── Agent runtime
    ├── Risk engine
    ├── Position supervisor
    ├── Local storage
    └── Local signer (optional; Live/wallet-backed mode)
```

Karakteristik:

- tidak membutuhkan server;
- cocok untuk development, observe, dry run tanpa wallet, paper, dan penggunaan lokal;
- runtime berhenti jika aplikasi benar-benar ditutup;
- dapat menyediakan tray mode agar runtime tetap aktif ketika window ditutup;
- adapter Telegram menggunakan long polling selama runtime aktif;
- database awal menggunakan SQLite WAL.

### 7.2 Docker Headless Server

```text
Browser/Tauri Remote Client
    ↕ HTTPS + WebSocket
Axum Server Host :8080
    ↓
Shared Rust Application Core
    ├── Adapter runtime
    ├── Agent runtime
    ├── Position supervisor 24/7
    ├── Telegram runtime 24/7
    ├── Persistent storage
    └── Server signer (optional; Live/wallet-backed mode)
```

Karakteristik:

- headless dan tidak membawa dependency GUI Tauri;
- berjalan 24/7 dengan restart policy;
- menyajikan React SPA, REST API, dan WebSocket pada satu port;
- menggunakan persistent volume;
- ideal sebagai execution authority untuk live mode.

### 7.3 Desktop Remote Client

Tauri dapat dikonfigurasi sebagai client dari Docker runtime:

```text
Tauri UI → HTTPS/WebSocket → Docker Rust runtime
```

Tauri tidak menjalankan local execution engine pada mode ini.

### 7.4 Single Execution Authority

Tauri local runtime dan Docker runtime tidak boleh mengeksekusi wallet yang sama secara bersamaan. Diperlukan `ExecutionLease` dengan heartbeat dan expiry.

Dry Run tidak membutuhkan `ExecutionLease` karena tidak mengendalikan wallet dan tidak dapat mengirim transaksi. Apabila wallet kebetulan sudah terhubung, Dry Run tetap dilarang memperoleh lease untuk signing/submission; wallet hanya boleh dipakai sebagai read-only portfolio context jika pengguna secara eksplisit mengaktifkannya.

```rust
pub struct ExecutionLease {
    pub wallet_id: WalletId,
    pub runtime_id: RuntimeId,
    pub acquired_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
```

Jika lease hilang:

- entry baru dihentikan;
- runtime mencoba memastikan tidak ada transaksi pending;
- operator menerima alert;
- takeover hanya setelah expiry dan reconciliation.

---

## 8. Technology Stack

### 8.1 Backend dan Runtime

- Rust stable toolchain.
- Tokio asynchronous runtime.
- Axum untuk HTTP API, static SPA serving, dan WebSocket pada server mode.
- Tauri 2 untuk desktop host.
- Serde untuk serialization.
- Rust Decimal/fixed-point representation untuk nilai finansial; hindari floating point untuk saldo dan amount.
- Tracing untuk structured telemetry.
- SQLx atau storage abstraction setara untuk persistence.

### 8.2 Frontend

- React.
- TypeScript strict mode.
- shadcn/ui sebagai component foundation.
- TanStack Query untuk server state.
- WebSocket event client untuk live state.
- Zod atau schema validator setara pada boundary frontend.
- Visual style: nuansa Jatevo.ai — off-white, near-black, yellow status strip, restrained purple/cyan/pink accent, mono operational labels, flat panels, small radius, subtle borders, tanpa card decoration berlebihan.

### 8.3 Persistence

MVP:

- SQLite dengan WAL untuk desktop dan single Docker instance.
- Persistent Docker volume untuk database dan encrypted vault.

Tahap lanjut:

- PostgreSQL untuk multi-user/multi-worker transactional state.
- TimescaleDB atau ClickHouse opsional untuk market event/candle dalam volume tinggi.
- Redis/NATS opsional untuk distributed events dan locks.

### 8.4 Blockchain dan Market Connectivity

- Solana RPC HTTP.
- Solana RPC WebSocket.
- Yellowstone gRPC untuk production market stream.
- Jupiter Swap V2.
- Pyth Price Feeds.
- Jito transaction delivery.
- Protocol-specific DEX decoders.

---

## 9. Struktur Rust Workspace

```text
kairos-agent/
├── Cargo.toml
├── apps/
│   ├── desktop/
│   │   ├── src-tauri/
│   │   └── tauri.conf.json
│   ├── server/
│   │   └── src/main.rs
│   └── web/
│       ├── src/
│       └── package.json
├── crates/
│   ├── domain/
│   ├── application/
│   ├── runtime-supervisor/
│   ├── adapter-core/
│   ├── adapter-solana-rpc/
│   ├── adapter-yellowstone/
│   ├── adapter-jupiter/
│   ├── adapter-pyth/
│   ├── adapter-jito/
│   ├── adapter-gmgn/
│   ├── adapter-telegram/
│   ├── adapter-llm/
│   ├── dex-raydium/
│   ├── dex-orca/
│   ├── dex-meteora/
│   ├── dex-pump/
│   ├── market-state/
│   ├── opportunity-scanner/
│   ├── agent-orchestrator/
│   ├── strategy-engine/
│   ├── risk-engine/
│   ├── quote-engine/
│   ├── execution-engine/
│   ├── position-manager/
│   ├── portfolio-ledger/
│   ├── wallet-signer/
│   ├── reconciliation/
│   ├── persistence/
│   ├── notification/
│   ├── audit/
│   └── telemetry/
├── migrations/
├── Dockerfile
└── compose.yaml
```

Dependency direction:

```text
domain
  ↑
application/use-cases
  ↑
infrastructure adapters
  ↑
Tauri/Axum hosts
```

`domain` tidak boleh bergantung pada Tauri, Axum, Telegram, provider API, atau database driver.

---

## 10. High-Level Runtime Architecture

```text
Runtime Supervisor
├── Credential Manager
├── Adapter Registry
├── Market Stream Supervisor
├── Market State Engine
├── Opportunity Scanner
├── Agent Orchestrator
├── Strategy Coordinator
├── Deterministic Risk Engine
├── Quote Engine
├── Execution Engine
├── Position Supervisor
├── Reconciliation Engine
├── Portfolio Ledger
├── Notification Service
├── Telegram Command Router
├── Audit Service
└── Health Supervisor
```

Semua long-running task berjalan sebagai supervised Tokio tasks dengan:

- cancellation token;
- bounded channel;
- restart policy;
- exponential backoff;
- health heartbeat;
- structured error classification;
- shutdown deadline;
- no unbounded spawning.

---

## 11. Application Boot Lifecycle

```text
Created
→ LoadingConfiguration
→ UnlockingVault
→ InitializingStorage
→ InitializingAdapters
→ ConnectingAdapters
→ SyncingChainState
→ WarmingMarketState
→ ValidatingAgentReadiness
→ ObserveReady / DryRunReady / PaperReady / ShadowReady / LiveReady
```

### 11.1 Urutan Boot

1. Resolve runtime identity dan deployment mode.
2. Load non-secret configuration.
3. Unlock encrypted credential vault.
4. Open database dan jalankan migration.
5. Recover last clean/unclean shutdown marker.
6. Load wallet jika tersedia, risk policy, position ledger, virtual ledger, dan pending transactions.
7. Acquire execution lease hanya jika mode memerlukan real wallet execution; Dry Run tidak meminta lease.
8. Initialize adapter registry.
9. Connect required adapters berdasarkan active strategy dependency graph.
10. Bandingkan slot dari minimal dua sumber jika tersedia.
11. Ambil snapshot token accounts, balances, positions, pools, dan recent signatures jika wallet terhubung; pada Dry Run buat atau load snapshot virtual account.
12. Mulai market streams dan buffer event baru selama snapshot berlangsung.
13. Merge snapshot dengan buffered events berdasarkan slot/order.
14. Rebuild candle, volatility, liquidity, dan derived state.
15. Reconcile posisi internal dengan wallet state untuk mode wallet-backed, atau dengan virtual ledger untuk Dry Run/Paper.
16. Load agent definitions, prompts, bindings, dan provider capabilities.
17. Jalankan readiness checks.
18. Masuk ke mode yang diperbolehkan.

### 11.2 Mode-Aware Readiness

| Dependency | Observe | Dry Run | Paper | Shadow | Live |
|---|---:|---:|---:|---:|---:|
| Market data minimum | Wajib | Wajib | Wajib | Wajib | Wajib |
| Agent/strategy runtime | Opsional sesuai fitur | Wajib | Wajib | Wajib | Wajib |
| Risk policy | Tidak untuk monitoring | Wajib, memakai virtual capital | Wajib | Wajib | Wajib |
| Wallet connection | Tidak | Tidak | Tidak; boleh read-only | Sesuai simulation fidelity | Wajib |
| Signer | Tidak | Dilarang digunakan | Tidak | Tidak men-submit | Wajib |
| Execution lease | Tidak | Tidak | Tidak | Tidak untuk submission | Wajib |
| On-chain submission adapters | Tidak | Tidak | Tidak | Tidak men-submit | Wajib |
| Virtual account | Tidak | Wajib | Wajib | Opsional | Tidak |

Readiness endpoint harus mengembalikan `required`, `optional`, `prohibited`, dan `status` untuk setiap capability. Dengan demikian indikator wallet dapat menampilkan `Not required` pada Dry Run, bukan `Failed` atau `Degraded`.

### 11.3 Unclean Shutdown Recovery

Jika shutdown sebelumnya tidak bersih:

- semua `Submitted` transaction diperiksa ulang;
- saldo on-chain dibaca ulang;
- open position direkonsiliasi;
- reservation kedaluwarsa dibersihkan hanya setelah bukti tidak ada pending execution;
- live entry dinonaktifkan sampai recovery selesai.

---

## 12. Multi-Agent Architecture

### 12.1 Jumlah Agent

MVP menggunakan empat agent logis:

1. Orchestrator.
2. Market Analysis.
3. On-chain Intelligence.
4. Strategy & Evaluation.

Target production menggunakan enam agent:

1. Orchestrator Agent.
2. Market Structure Agent.
3. On-chain Intelligence Agent.
4. Strategy Agent.
5. Risk Analyst Agent.
6. Post-Trade Evaluator Agent.

### 12.2 Orchestrator Agent

Tanggung jawab:

- menerima trigger cycle;
- menentukan dependency agent;
- memastikan input tersedia dan fresh;
- menjalankan specialist agent secara paralel;
- menangani timeout;
- mengumpulkan output;
- menghindari duplikasi cycle;
- menutup cycle dengan outcome terstruktur.

Orchestrator tidak memiliki signer dan tidak boleh menghasilkan executable transaction.

### 12.3 Market Structure Agent

Input:

- OHLCV multi-timeframe;
- recent swaps;
- liquidity dan depth approximation;
- price impact per notional;
- cross-DEX spread;
- volatility;
- Pyth/Jupiter reference deviation;
- market freshness;
- existing portfolio exposure.

Output:

```rust
pub struct MarketAnalysis {
    pub regime: MarketRegime,
    pub direction: Direction,
    pub strength: Decimal,
    pub volatility: Decimal,
    pub liquidity_quality: Decimal,
    pub confidence: Decimal,
    pub evidence: Vec<Evidence>,
    pub invalidation_conditions: Vec<Condition>,
    pub expires_at: DateTime<Utc>,
}
```

### 12.4 On-chain Intelligence Agent

Input:

- whale/smart-money wallet activity;
- token holder concentration;
- transfer graph summary;
- deployer/creator activity;
- token authority dan Token-2022 extensions;
- GMGN enrichment;
- pool creation/migration event;
- correlated wallet clusters;
- suspected wash/bundled activity.

Output harus membedakan:

- transfer vs swap;
- exchange/custody wallet vs trader;
- LP action vs directional trade;
- correlated wallets vs independent confirmation;
- raw whale size vs historically profitable smart money.

### 12.5 Strategy Agent

Menggabungkan specialist reports dan menghasilkan:

```rust
pub struct TradeProposal {
    pub id: ProposalId,
    pub strategy_id: StrategyId,
    pub pair: TradingPair,
    pub side: TradeSide,
    pub maximum_notional: Decimal,
    pub entry_policy: EntryPolicy,
    pub stop_loss: ExitRule,
    pub take_profit: Vec<TakeProfitRule>,
    pub trailing_stop: Option<TrailingStopRule>,
    pub time_stop: Option<Duration>,
    pub maximum_slippage_bps: u16,
    pub thesis: String,
    pub confidence: Decimal,
    pub evidence_refs: Vec<EvidenceRef>,
    pub input_slots: Vec<u64>,
    pub expires_at: DateTime<Utc>,
}
```

Allowed actions:

- `Buy`;
- `Sell`;
- `Hold`;
- `Close`;
- `ReducePosition`.

### 12.6 Risk Analyst Agent

Memberikan advisory analysis untuk kondisi kompleks:

- manipulasi;
- conflicting sources;
- abnormal liquidity;
- unusual wallet correlation;
- market regime yang berbeda dari backtest;
- data-quality anomaly.

Risk Analyst tidak menggantikan deterministic risk engine.

### 12.7 Post-Trade Evaluator

Mengevaluasi:

- expected vs actual output;
- realized slippage;
- fee dan priority fee;
- thesis validity;
- signal contribution;
- holding duration;
- exit quality;
- adapter latency;
- agent calibration.

Evaluator hanya menghasilkan recommendation. Perubahan strategi harus melalui backtest, validation, dan approval.

### 12.8 Agent Run Contract

Setiap run mempunyai:

```rust
pub struct AgentRun {
    pub id: AgentRunId,
    pub cycle_id: CycleId,
    pub agent_id: AgentId,
    pub model_binding_id: ModelBindingId,
    pub input_hash: Hash,
    pub input_freshness: Vec<DataFreshness>,
    pub status: AgentRunStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub output_schema_version: String,
    pub output: Option<AgentOutput>,
    pub error: Option<AgentError>,
}
```

Status:

```text
Queued → Running → Completed
                 ↘ TimedOut
                 ↘ InvalidOutput
                 ↘ Failed
                 ↘ Cancelled
```

---

## 13. Deterministic Core Services

Komponen berikut bukan AI agent:

| Service | Tanggung jawab |
|---|---|
| Market State Engine | Normalisasi event, snapshot, candle, liquidity, freshness |
| Opportunity Scanner | Candidate discovery dan cheap deterministic filter |
| Risk Engine | Position sizing, exposure, loss limit, veto |
| Quote Engine | Fresh executable quote dan route comparison |
| Execution Engine | Build, validate, simulate, sign, submit |
| Position Manager | Multi-position ledger dan exit lifecycle |
| Position Supervisor | Monitor TP/SL/trailing/time-stop terus-menerus |
| Portfolio Ledger | Balance, reservation, cost basis, realized/unrealized PnL |
| Reconciliation Engine | Samakan internal state dengan confirmed on-chain state |
| Kill Switch | Hentikan risk-increasing action |
| Adapter Supervisor | Health, reconnect, resync, failover |
| Audit Service | Append-only security dan decision log |

---

## 14. Adapter Architecture

### 14.1 Adapter Categories

1. Market stream adapters.
2. RPC/state adapters.
3. DEX decoder/pool adapters.
4. Reference price adapters.
5. Token intelligence adapters.
6. Historical market data adapters.
7. Quote/routing adapters.
8. Transaction submission adapters.
9. Confirmation adapters.
10. LLM adapters.
11. Telegram adapter.
12. Wallet/signer adapters.

### 14.2 Common Adapter Interface

```rust
#[async_trait]
pub trait Adapter: Send + Sync {
    fn id(&self) -> AdapterId;
    fn kind(&self) -> AdapterKind;
    async fn connect(&self) -> Result<()>;
    async fn disconnect(&self) -> Result<()>;
    async fn health(&self) -> AdapterHealth;
    async fn capabilities(&self) -> AdapterCapabilities;
}
```

### 14.3 Adapter State Machine

```text
Unconfigured
→ Configured
→ Connecting
→ Authenticating
→ Syncing
→ Warming
→ Ready
```

Failure branches:

```text
Ready → Degraded → Reconnecting → Syncing → Ready
                    ↘ Failed → Disabled
```

### 14.4 Capability-Level Readiness

Status `Connected` tidak cukup. Readiness harus per capability:

```rust
pub struct AdapterReadiness {
    pub connectivity: Health,
    pub market_data: CapabilityHealth,
    pub historical_data: CapabilityHealth,
    pub quote: CapabilityHealth,
    pub execution: CapabilityHealth,
    pub confirmation: CapabilityHealth,
    pub last_slot: Option<u64>,
    pub lag_slots: Option<u64>,
    pub last_success_at: Option<DateTime<Utc>>,
}
```

Required capability dihitung dari `TradingMode` dan dependency graph strategy. Pada Dry Run, market data, reference price, token intelligence, quote, dan LLM yang dipakai strategy dapat berstatus required. Wallet/signer, confirmation, Jito, serta RPC transaction submission berstatus `Prohibited` atau `NotRequired`. Adapter Supervisor tidak boleh mencoba auto-connect adapter submission hanya karena credential pernah dikonfigurasi.

### 14.5 Reconnect dan Resync

Ketika stream putus:

1. Tandai adapter `Degraded`.
2. Blokir entry yang bergantung pada adapter tersebut.
3. Gunakan fallback untuk posisi terbuka.
4. Reconnect dengan capped exponential backoff dan jitter.
5. Ambil latest snapshot.
6. Deteksi gap slot/event.
7. Backfill bila tersedia.
8. Replay buffered event.
9. Bandingkan state dengan provider lain.
10. Kembali `Ready` hanya setelah consistency checks lulus.

### 14.6 UI Configuration Flow

```text
Settings → Connections → Add connection
→ Select adapter type
→ Select provider
→ Input endpoint/credential
→ Test connection
→ Display capability/latency
→ Enable
→ Sync/Warm
→ Ready
```

---

## 15. Market Data Adapters

### 15.1 ChainStreamAdapter

```rust
pub trait ChainStreamAdapter: Adapter {
    async fn subscribe_transactions(
        &self,
        filters: TransactionFilters,
    ) -> Result<MarketEventStream>;

    async fn subscribe_accounts(
        &self,
        filters: AccountFilters,
    ) -> Result<AccountUpdateStream>;

    async fn current_slot(&self) -> Result<u64>;
}
```

Implementasi:

- `SolanaWebSocketAdapter` untuk MVP/fallback.
- `YellowstoneGrpcAdapter` untuk production primary.
- Jito ShredStream/low-level feed sebagai tahap advanced latency.

Solana WebSocket menyediakan `accountSubscribe`, `programSubscribe`, `logsSubscribe`, `signatureSubscribe`, slot, root, dan block-related subscriptions. Yellowstone gRPC digunakan untuk stream account/transaction/block dengan throughput dan latency yang lebih cocok bagi trading/indexing production.

Referensi:

- [Solana RPC WebSocket](https://solana.com/docs/rpc/websocket)
- [Yellowstone/Geyser overview](https://solana.com/docs/payments/accept-payments/indexing)

### 15.2 RpcStateAdapter

```rust
pub trait RpcStateAdapter: Adapter {
    async fn get_accounts(&self, keys: &[Pubkey]) -> Result<Vec<Account>>;
    async fn get_token_balances(&self, owner: Pubkey) -> Result<Vec<TokenBalance>>;
    async fn latest_blockhash(&self) -> Result<BlockhashContext>;
    async fn recent_priority_fees(&self, accounts: &[Pubkey]) -> Result<Vec<PriorityFeeSample>>;
    async fn simulate(&self, transaction: &VersionedTransaction) -> Result<SimulationResult>;
    async fn signature_status(&self, signature: Signature) -> Result<TransactionStatus>;
}
```

Minimal production configuration:

- Primary HTTP RPC.
- Secondary HTTP RPC dari provider berbeda.
- Primary WebSocket/Yellowstone.
- Fallback WebSocket.

### 15.3 Commitment Policy

- `processed`: digunakan untuk rapid market observation, bukan final accounting.
- `confirmed`: digunakan untuk portfolio operational state dan sebagian besar fill acknowledgment.
- `finalized`: digunakan untuk permanent settlement/audit checkpoint.

`processed` dapat hilang akibat fork. Sistem wajib menangani rollback/fork dan tidak menganggap `sendTransaction` response sebagai bukti fill.

Referensi:

- [Solana confirmation levels](https://solana.com/docs/payments/production-readiness)
- [Solana sendTransaction](https://solana.com/docs/rpc/http/sendtransaction)
- [Solana getSignatureStatuses](https://solana.com/docs/rpc/http/getsignaturestatuses)

---

## 16. DEX Protocol Adapters

### 16.1 DexDecoder Contract

```rust
pub trait DexDecoder: Send + Sync {
    fn venue(&self) -> Venue;
    fn program_ids(&self) -> &[Pubkey];
    fn decode_transaction(&self, tx: &ChainTransaction) -> Result<Vec<DexEvent>>;
    fn decode_account(&self, update: &AccountUpdate) -> Result<Vec<PoolStateUpdate>>;
}
```

### 16.2 Prioritas Venue

#### MVP Liquid Markets

- Raydium AMM v4/CPMM/CLMM.
- Orca Whirlpools.
- Meteora DLMM.

#### New Token/Memecoin Phase

- Pump.fun bonding curve.
- PumpSwap.
- Raydium LaunchLab.
- Pool creation/migration events.

### 16.3 Normalized Events

```rust
pub struct SwapEvent {
    pub slot: u64,
    pub signature: Signature,
    pub venue: Venue,
    pub pool: Pubkey,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub input_amount: u64,
    pub output_amount: u64,
    pub effective_price: Decimal,
    pub fee_amount: Option<u64>,
    pub commitment: CommitmentLevel,
    pub observed_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
}
```

```rust
pub struct PoolSnapshot {
    pub venue: Venue,
    pub pool: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub executable_bid: Decimal,
    pub executable_ask: Decimal,
    pub liquidity_usd: Decimal,
    pub price_impact_samples: Vec<PriceImpactSample>,
    pub slot: u64,
}
```

Referensi:

- [Raydium developer documentation](https://docs.raydium.io/)
- [Raydium program addresses](https://docs.raydium.io/reference/program-addresses)
- [Orca developer overview](https://docs.orca.so/developers/overview)

---

## 17. Reference Price dan Token Intelligence

### 17.1 PythAdapter

Fungsi:

- fair/reference price untuk aset yang didukung;
- deviation detection;
- circuit breaker;
- USD portfolio valuation;
- cross-check terhadap DEX price.

Pyth bukan executable quote dan tidak menggantikan pool state atau Jupiter quote.

Referensi: [Pyth Price Feeds](https://docs.pyth.network/price-feeds)

### 17.2 JupiterPriceAdapter

Fungsi:

- broad Solana token indicative price;
- price freshness menggunakan block identifier;
- candidate ranking;
- valuation dan sanity check.

Harga dari Price API tidak digunakan langsung sebagai `min_out`.

Referensi: [Jupiter Price API](https://developers.jup.ag/docs/price)

### 17.3 TokenIntelligenceAdapter

Data wajib:

- canonical mint;
- token program;
- decimals;
- supply;
- mint authority;
- freeze authority;
- metadata;
- Token-2022 extensions;
- transfer fees;
- permanent delegate;
- non-transferable/default frozen/pausable behavior;
- holder concentration;
- known verification status;
- pool liquidity dan age.

Jupiter Tokens/GMGN dapat digunakan sebagai enrichment, tetapi on-chain mint validation tetap authoritative.

Referensi:

- [Solana Token Extensions](https://solana.com/docs/tokens/extensions)
- [Token-2022 Transfer Fees](https://solana.com/docs/tokens/extensions/transfer-fees)
- [Permanent Delegate](https://solana.com/docs/tokens/extensions/permanent-delegate)
- [Jupiter Tokens API](https://developers.jup.ag/docs/tokens)

---

## 18. Intelligence Adapters: GMGN, Whale, dan Social

### 18.1 GMGNAdapter

Status: optional enrichment, bukan source of truth.

Fungsi:

- token discovery;
- trending token;
- smart-money labels;
- wallet PnL enrichment;
- holder analysis;
- KOL signal;
- token security enrichment.

GMGN tidak digunakan sebagai signer atau primary transaction executor. Semua token dan quote divalidasi kembali melalui on-chain/RPC/Jupiter.

### 18.2 Internal Wallet Intelligence

Whale tracking dibangun sebagai capability internal menggunakan Yellowstone stream:

```rust
pub struct WalletSignal {
    pub wallet: Pubkey,
    pub mint: Pubkey,
    pub action: WalletAction,
    pub token_amount: u64,
    pub value_usd: Decimal,
    pub wallet_score: Decimal,
    pub conviction_score: Decimal,
    pub observed_slot: u64,
}
```

Wallet score mempertimbangkan:

- realized PnL;
- consistency;
- win rate;
- entry timing;
- liquidity-adjusted return;
- holding duration;
- transfer contamination;
- suspected insider/bundling;
- wash activity;
- correlated wallets.

### 18.3 Fomo/Social Adapter

Tidak menjadi dependency MVP. Social feed dapat ditambahkan setelah on-chain foundation stabil. Social signal tidak boleh menjadi satu-satunya alasan entry.

---

## 19. LLM Adapter

### 19.1 UI Configuration

```text
Settings → Connections → AI Providers
```

Fields:

- connection name;
- provider type;
- base URL;
- API key;
- model;
- timeout;
- maximum output tokens;
- temperature jika relevan;
- maximum concurrency;
- structured output capability;
- tool-calling capability;
- streaming capability;
- daily request/cost ceiling;
- fallback connection/model.

### 19.2 LLM Connection Model

```rust
pub struct LlmConnectionConfig {
    pub id: LlmConnectionId,
    pub name: String,
    pub provider: LlmProviderType,
    pub base_url: Url,
    pub credential_ref: Option<CredentialRef>,
    pub model: String,
    pub timeout: Duration,
    pub maximum_concurrency: usize,
    pub enabled: bool,
}
```

API key tidak berada dalam config row. Hanya `CredentialRef` yang disimpan.

### 19.3 Provider Implementations

- OpenAI-compatible HTTP provider.
- Provider-specific remote adapters jika capability berbeda.
- Local HTTP model adapter.
- Embedded local model adapter sebagai tahap lanjutan.

### 19.4 LLM Contract

```rust
pub trait LlmAdapter: Adapter {
    async fn model_capabilities(&self, model: &str) -> Result<LlmCapabilities>;
    async fn generate_structured(
        &self,
        request: LlmRequest,
        schema: OutputSchema,
    ) -> Result<StructuredLlmResponse>;
}
```

### 19.5 Agent Model Binding

```rust
pub struct AgentModelBinding {
    pub agent_id: AgentId,
    pub primary_connection: LlmConnectionId,
    pub primary_model: String,
    pub fallback_connection: Option<LlmConnectionId>,
    pub fallback_model: Option<String>,
}
```

Agent berbeda dapat memakai model berbeda. Market/on-chain agent dapat menggunakan fast model, sedangkan Strategy menggunakan reasoning model. Evaluator dapat menggunakan local model untuk mengurangi biaya.

### 19.6 Failure Policy

```text
Primary timeout
→ one bounded retry jika error retryable
→ fallback provider/model
→ validate structured output
→ jika gagal: no-trade / HOLD
```

Open position tetap diawasi deterministic Position Supervisor ketika seluruh LLM unavailable.

### 19.7 Custom URL Security

- HTTPS wajib untuk remote URL.
- HTTP hanya untuk localhost/loopback yang diizinkan.
- Tolak URL dengan embedded credential.
- Batasi redirect dan cegah redirect remote ke private network.
- Batasi response size.
- Terapkan connect/read/total timeout.
- Jangan menaruh API key di query string.
- Redact authorization header dan secret dari log.
- Allowlist outbound destination dapat diaktifkan pada server production.

---

## 20. Telegram Bot Adapter

### 20.1 Konfigurasi UI

```text
Settings → Connections → Telegram Bot
```

Fields:

- connection name;
- bot token;
- transport: long polling atau webhook;
- authorized users;
- role/permissions;
- notification categories;
- minimum severity;
- quiet hours;
- daily summary schedule;
- optional webhook URL dan secret token.

Bot token disimpan di encrypted credential vault.

### 20.2 Transport

MVP desktop dan Docker menggunakan long polling. Long polling tidak memerlukan public inbound endpoint. Bot hanya aktif selama runtime hidup.

Webhook tersedia untuk deployment server dan menggunakan satu exposed application port. Webhook wajib HTTPS dari sisi publik dan menggunakan Telegram webhook secret token.

Telegram menyatakan webhook dan `getUpdates` tidak dapat digunakan pada saat yang sama.

Referensi:

- [Telegram Bot API](https://core.telegram.org/bots/api)
- [Telegram webhook guide](https://core.telegram.org/bots/webhooks)

### 20.3 Telegram Account Pairing

1. Bot token divalidasi melalui `getMe`.
2. Runtime membuat high-entropy one-time pairing code.
3. UI menampilkan `Open Telegram` deep link.
4. Pengguna mengirim `/start <pairing-code>`.
5. Runtime memverifikasi expiry dan single use.
6. Numeric Telegram user ID dan chat ID ditampilkan di UI.
7. Owner menetapkan role dan menyetujui pairing.

Referensi: [Telegram Bot deep linking](https://core.telegram.org/bots/features)

### 20.4 Bot Commands

Read-only:

```text
/status
/health
/adapters
/agents
/cycle
/portfolio
/positions
/position <id>
/pnl
/history
/market <symbol>
/analyze <symbol>
/signals
/why <position-id>
/watch <symbol>
/unwatch <symbol>
/watchlist
/whales <symbol>
/dry_run
/dry_run_status
```

Risk-reducing:

```text
/pause
/kill
/reject <proposal-id>
/close <position-id>
/close_all
```

Risk-increasing, default disabled:

```text
/resume
/approve <proposal-id>
/open
/change_limit
/change_strategy
```

Withdrawal tidak pernah tersedia melalui Telegram.

### 20.5 Sensitive Command Rules

- Authorization berdasarkan numeric Telegram user ID.
- Inline button callback memakai one-time nonce.
- Confirmation memiliki short expiry.
- Fresh quote dan simulation selalu diulang setelah confirmation.
- Telegram command tidak dapat memanggil signer secara langsung.
- `/kill` dapat langsung menghentikan entry baru.
- `/resume` setelah kill switch membutuhkan UI/local owner approval secara default.
- `/close` dan `/close_all` hanya menghasilkan risk-reducing sell flow dan tetap melewati validation.

### 20.6 Dry-Run Telegram Behavior

Notifikasi yang berasal dari Dry Run harus diawali label `[DRY RUN]`. `/dry_run_status` bersifat read-only dan menampilkan session, virtual equity, simulated positions, PnL, serta adapter readiness. Memulai, mereset, atau menghentikan Dry Run melalui Telegram default-nya disabled dan, jika kemudian diizinkan, tidak boleh mengubah mode Live/Paper atau memicu signer. Command `/close` pada simulated position hanya menghasilkan virtual fill.

### 20.7 Notification Events

- proposal dibuat;
- proposal ditolak;
- order dibangun/disubmit/confirmed/failed;
- TP/SL/trailing exit;
- position closed;
- daily loss limit;
- adapter degraded/failed/recovered;
- market data stale;
- LLM provider failed;
- wallet fee reserve rendah;
- execution lease hilang;
- safe mode;
- daily performance summary.

Default tidak mengirim seluruh agent cycle agar bot tidak noisy.

---

## 21. Wallet dan Signer Architecture

### 21.1 Wallet Modes

| Mode | Signing | Automation | Kegunaan |
|---|---:|---:|---|
| Dry-run virtual account | Tidak; tidak memiliki private key | Ya, hanya simulasi | Menguji seluruh lifecycle tanpa koneksi wallet |
| Watch-only | Tidak | Tidak | Portfolio monitoring |
| External wallet | User approval | Terbatas | Ownership, funding, manual trade |
| Dedicated automation wallet | Ya | Ya | Unattended automated trading |
| Hardware/KMS/delegated signer | Ya | Ya sesuai policy | Advanced production |

### 21.2 Recommended Fund Separation

```text
Main wallet
├── menyimpan mayoritas aset
├── funding automation wallet
└── menerima profit/withdrawal manual

Automation wallet
├── menyimpan modal trading terbatas
├── digunakan Rust signer
├── tidak memiliki unrestricted withdrawal command
└── tunduk pada spend/risk policy
```

Seed phrase main wallet tidak perlu dan tidak disarankan diimpor ke aplikasi.

### 21.3 Wallet Connection UI

```text
Settings → Wallets → Add wallet
```

Options:

1. Add watch-only address.
2. Connect external wallet.
3. Create automation wallet.
4. Import dedicated automation key dengan warning dan secure flow.
5. Connect advanced signer pada fase berikutnya.

Wallet Standard/Reown dapat digunakan untuk external wallet flow. Karena Tauri WebView bukan browser extension host yang sama dengan Chrome, WalletConnect/Reown QR atau system-browser pairing perlu disediakan dan diuji melalui proof-of-concept.

Referensi:

- [Solana Wallet Standard integration](https://solana.com/docs/frontend/web3-compat)
- [Reown Solana support](https://docs.reown.com/appkit/networks/solana)

### 21.4 Signer Boundary

```rust
pub trait SignerAdapter: Send + Sync {
    fn public_key(&self) -> Pubkey;
    async fn sign_message(&self, message: &[u8]) -> Result<Signature>;
    async fn sign_transaction(
        &self,
        intent: AuthorizedTransactionIntent,
        transaction: VersionedTransaction,
    ) -> Result<VersionedTransaction>;
}
```

Frontend tidak mendapat `sign_any_transaction`. Frontend hanya dapat meminta application command seperti `approve_proposal(proposal_id)`. Rust memuat proposal dari storage, memvalidasi ulang, membangun ulang transaction, lalu meminta signer.

### 21.5 Wallet Readiness

- cluster benar;
- signer public key cocok;
- balance dapat dibaca;
- SOL fee reserve cukup;
- spend policy aktif;
- program allowlist aktif;
- RPC/Jito ready;
- simulation tersedia;
- execution lease aktif;
- mode paper/live sesuai policy.

Checklist di atas berlaku untuk mode wallet-backed. Pada Dry Run, wallet readiness dinyatakan `NotRequired`; status tersebut tidak boleh menurunkan overall runtime readiness. Pengguna dapat memulai Dry Run sebelum membuat, mengimpor, atau menghubungkan wallet apa pun.

### 21.6 Secret Storage

Desktop menggunakan OS-backed secret storage/Stronghold. Docker menggunakan encrypted vault pada persistent volume dengan master key dari Docker secret/file, bukan plaintext environment yang dicatat sembarangan.

Referensi: [Tauri Stronghold](https://v2.tauri.app/plugin/stronghold/)

### 21.7 Dry-Run Virtual Account

Virtual account merupakan ledger entity internal, bukan Solana keypair, tidak memiliki seed phrase/private key, tidak dapat menerima dana, dan tidak dapat digunakan sebagai signer. Pada pembuatan session, pengguna menentukan:

- starting quote balance, misalnya virtual USDC;
- optional starting token balances;
- denomination/currency pelaporan;
- fee model;
- latency model;
- slippage/fill model;
- reset policy dan persistence policy.

Jika quote/build provider membutuhkan public key pada request, runtime boleh menghasilkan **ephemeral non-funded public key** khusus untuk pembentukan payload. Tidak ada private key yang disimpan atau dipakai untuk signing. Hasil RPC simulation yang mensyaratkan funded fee payer diberi label `UnavailableWithoutWallet` atau dijalankan sebagai best-effort; runtime tidak boleh menyamakan quote validation/local instruction validation dengan keberhasilan on-chain simulation.

---

## 22. Trading Modes

### 22.1 Observe Only

- stream data aktif;
- agent analysis aktif;
- proposal dapat dibuat;
- tidak ada dry-run/paper/live order;
- wallet signing tidak diperlukan.

### 22.2 Dry Run — Tanpa Wallet

Dry Run menjalankan workflow trading sedekat mungkin dengan runtime produksi, tetapi seluruh efek finansial diarahkan ke virtual ledger dan hard-stop sebelum signer/submission boundary.

Karakteristik wajib:

- tidak mewajibkan wallet connection, public wallet address, signer, SOL fee reserve, atau execution lease;
- market stream, scanner, agent orchestration, `TradeProposal`, risk evaluation, fresh quote, position supervisor, TP/SL/trailing/time-stop, dan post-trade evaluator tetap berjalan;
- menggunakan `VirtualAccount` dengan starting capital yang dikonfigurasi pengguna;
- menghasilkan simulated orders, fills, fees, slippage, PnL, drawdown, dan multi-position;
- tidak membuat signature dan tidak mengirim transaction ke RPC, Jito, Jupiter execute, atau DEX;
- transaction building/on-chain simulation bersifat optional best-effort dan tidak boleh membuat wallet menjadi dependency;
- setiap UI, API, WebSocket event, Telegram notification, audit event, order, fill, position, dan PnL diberi label `DRY_RUN`;
- mode ini dapat dijalankan terus-menerus atau sebagai session dengan waktu tertentu;
- session dapat di-reset/diarsipkan tanpa memengaruhi paper/live ledger;
- aktivasi Dry Run tidak meminta live authorization.

Dry Run berbeda dari Observe Only karena ia membuat hypothetical order/fill/position dan menguji lifecycle entry sampai exit. Dry Run berbeda dari Paper Trading karena fokus utamanya adalah validasi wiring dan perilaku runtime tanpa wallet; hasilnya disimpan dalam session terisolasi dan tidak boleh dicampur dengan benchmark paper/live. Paper Trading merupakan lingkungan simulasi portofolio berkelanjutan untuk evaluasi strategi yang lebih formal.

Dry-run session lifecycle:

```text
Created → Validating → Ready → Running ↔ Paused → Stopped / Completed
                     ↘ Failed
Stopped / Completed / Failed → Archived
Any non-running terminal session → ResetAsNewSession
```

`ResetAsNewSession` tidak menghapus audit/history. Runtime membuat session ID baru dengan `parent_session_id` menunjuk session sebelumnya dan menyalin configuration snapshot sesuai pilihan pengguna.

Contoh konfigurasi:

```rust
pub struct DryRunConfig {
    pub session_name: String,
    pub starting_balances: Vec<VirtualBalance>,
    pub fill_model: FillModel,
    pub latency_model: LatencyModel,
    pub fee_model: FeeModel,
    pub persist_session: bool,
    pub maximum_duration: Option<Duration>,
    pub allow_best_effort_transaction_build: bool,
    pub allow_best_effort_rpc_simulation: bool,
}

pub enum TradingMode {
    Observe,
    DryRun,
    Paper,
    Shadow,
    Live,
}
```

Capability contract:

```rust
pub struct ModeCapabilities {
    pub can_create_virtual_orders: bool,
    pub can_build_unsigned_transaction: bool,
    pub can_request_wallet_signature: bool,
    pub can_submit_onchain: bool,
    pub requires_wallet: bool,
    pub requires_execution_lease: bool,
}

// Invariant untuk Dry Run:
// can_create_virtual_orders = true
// can_request_wallet_signature = false
// can_submit_onchain = false
// requires_wallet = false
// requires_execution_lease = false
```

### 22.3 Paper Trading

- menggunakan market/quote pipeline nyata;
- execution disimulasikan;
- virtual balance dan fills dicatat;
- fee, slippage, latency, dan failed fills dimodelkan;
- wallet signer tidak diperlukan; optional read-only wallet context dapat digunakan;
- default pertama setelah instalasi untuk evaluasi strategi berkelanjutan, sedangkan Dry Run menjadi default smoke validation end-to-end tanpa wallet.

### 22.4 Shadow Mode

- proposal dan risk decision produksi dibuat;
- transaction dapat dibangun dan disimulasikan;
- tidak disubmit;
- hasil dibandingkan dengan market movement berikutnya.

### 22.5 Live Mode

Hanya dapat diaktifkan jika:

- semua required adapters ready;
- wallet ready;
- execution lease acquired;
- risk policy lengkap;
- kill switch available;
- reconciliation sehat;
- required test gates telah lulus;
- owner melakukan explicit activation.

Live activation harus time-limited session atau dapat memerlukan re-authorization setelah restart, sesuai policy.

---

## 23. Opportunity Discovery Lifecycle

```text
Universe Sources
→ Candidate Discovery
→ Deterministic Pre-filter
→ Market Hydration
→ Agent Analysis
→ Proposal or Discard
```

### 23.1 Universe Sources

- fixed allowlist;
- manually managed watchlist;
- Jupiter trending/category;
- GMGN trending;
- pool creation;
- smart-money activity;
- cross-DEX deviation;
- volume/liquidity anomaly.

### 23.2 Deterministic Pre-Filter

- minimum liquidity;
- maximum price impact;
- pool age;
- token program/extension checks;
- mint/freeze/delegate risk;
- holder concentration;
- route availability;
- data freshness;
- denylist;
- existing portfolio correlation;
- fee/transfer tax impact;
- minimum/maximum market cap jika data tersedia.

### 23.3 Market Hydration

Kandidat yang lolos diperkaya dengan:

- recent swaps;
- OHLCV multi-timeframe;
- DEX/pool distribution;
- executable price impact samples;
- oracle/reference price;
- wallet intelligence;
- current portfolio state;
- active position conflicts;
- route availability.

---

## 24. Trading Cycle State Machine

```text
Created
→ Hydrating
→ Analyzing
→ Synthesizing
→ RiskReview
→ Rejected / Expired / Approved
→ Quoting

Mode branch setelah Quoting:
- Observe: ProposalRecorded
- Dry Run: Simulating → DryRunRecording → VirtualFilled → VirtualReconciled
- Paper: Simulating → VirtualFilled → VirtualReconciled
- Shadow: Building → Simulating → ShadowRecorded
- Live: Simulating → AwaitingAuthorization (optional) → Signing → Submitting → Confirming → Reconciled
```

Failure states:

```text
DataStale
AdapterUnavailable
AgentFailed
RiskRejected
QuoteExpired
SimulationFailed
SigningFailed
SubmissionFailed
BlockhashExpired
ConfirmationUnknown
ReconciliationRequired
```

Setiap state transition bersifat idempotent dan dicatat dalam audit log.

Pada Dry Run, state sesudah `Quoting` adalah `Simulating` menggunakan fill model dan optional best-effort transaction simulation, lalu `DryRunRecording`, `VirtualFilled`, dan `VirtualReconciled`. State `AwaitingAuthorization`, `Signing`, `Submitting`, serta `Confirming` tidak dapat dicapai. Percobaan untuk masuk ke state tersebut harus ditolak sebagai `ModeCapabilityViolation`, dicatat sebagai security/audit event, dan tidak boleh di-retry otomatis.

---

## 25. Risk Engine

### 25.1 Authority

Risk engine adalah satu-satunya komponen yang dapat menghasilkan `RiskApproved` authorization untuk execution engine.

```rust
pub enum RiskDecision {
    Approved(ApprovedTradeIntent),
    Rejected { reasons: Vec<RiskRejection> },
}
```

### 25.2 Risk Checks

- maximum notional per trade;
- risk per trade;
- maximum open positions;
- token exposure;
- portfolio gross exposure;
- correlated exposure;
- daily realized loss;
- rolling drawdown;
- consecutive loss cooldown;
- trade frequency;
- pending transaction count;
- available balance after reservation;
- SOL fee reserve;
- liquidity and price impact;
- maximum slippage;
- token safety;
- route program allowlist;
- data/quote freshness;
- adapter health;
- wallet execution lease;
- strategy-specific constraints.

Dalam Dry Run, pemeriksaan risk yang berhubungan dengan modal menggunakan virtual balance dan virtual reservation. Check wallet execution lease serta SOL fee reserve diberi status `NotApplicable`, bukan dilewati secara diam-diam. Liquidity, quote freshness, price impact, token safety, exposure, loss limit, drawdown, dan strategy constraint tetap diberlakukan agar hasil Dry Run representatif.

### 25.3 Suggested Initial Policy for Testing

Nilai final wajib ditetapkan pengguna dan tidak dianggap rekomendasi investasi. Contoh konfigurasi development:

```text
Mode                         Paper
Maximum open positions       5
Maximum pending transactions 2
Maximum notional per trade   configurable
Maximum token exposure       configurable
Daily loss limit             mandatory
Minimum SOL fee reserve      mandatory
Maximum quote age            very short, slot-aware
Program allowlist            enabled
Automatic withdrawal         disabled
```

Preset Dry Run menggunakan policy structure yang sama, tetapi mengganti `Mode` menjadi `DryRun`, menggunakan virtual starting balance, dan menetapkan `signing=false`, `submission=false` sebagai invariant yang tidak dapat dioverride melalui UI, API, Telegram, strategy, maupun LLM output.

### 25.4 Kill Switch

Kill switch memiliki level:

1. `PauseEntries`: hentikan entry baru.
2. `CancelPending`: batalkan intent yang belum disubmit.
3. `ReduceOnly`: hanya izinkan tindakan mengurangi exposure.
4. `EmergencyClose`: tutup posisi sesuai emergency policy.
5. `RuntimeLockdown`: signer terkunci dan memerlukan owner unlock.

Kill switch dapat dipicu oleh:

- owner UI;
- authorized Telegram `/kill`;
- daily loss limit;
- reconciliation mismatch;
- execution lease loss;
- severe adapter divergence;
- wallet anomaly;
- repeated submission failures.

---

## 26. Quote, Build, dan Execution

### 26.1 Jupiter Routing

Primary implementation menggunakan Jupiter Swap V2.

- `/order` + `/execute`: meta-aggregator dan managed landing.
- `/build`: raw on-chain routing instructions dan full transaction control.

Untuk requirement on-chain controlled execution, primary design menggunakan `/build`. Direct DEX quote adapters dapat ditambahkan untuk arbitrage/latency-sensitive phase.

Referensi: [Jupiter Swap V2](https://developers.jup.ag/docs/swap)

### 26.2 Execution Workflow

1. Load `ApprovedTradeIntent`.
2. Verifikasi intent belum expired dan belum consumed.
3. Reserve balance dan risk budget.
4. Ambil fresh executable quote.
5. Validasi route, program IDs, accounts, price impact, fee, dan min-out.
6. Ambil recent blockhash.
7. Tentukan compute unit dan priority fee.
8. Build versioned transaction.
9. Simulate menggunakan commitment yang konsisten.
10. Validasi simulation logs dan balance effects.
11. Minta optional manual authorization jika policy mensyaratkan.
12. Sign di signer boundary.
13. Submit melalui Jito atau configured primary route.
14. Submit fallback hanya berdasarkan retry policy yang idempotent dan tidak menghasilkan duplicate intent.
15. Track signature sampai terminal status.
16. Fetch confirmed transaction dan post balances.
17. Reconcile.
18. Commit position/portfolio changes.
19. Release reservation.

### 26.2.1 Dry-Run Execution Branch

Untuk `TradingMode::DryRun`, langkah 1–10 tetap digunakan sejauh tidak menimbulkan dependency wallet. Setelah quote/instruction validation, dispatcher harus melakukan alur berikut:

1. Load `ApprovedTradeIntent` berlabel `DRY_RUN`.
2. Reserve virtual balance dan virtual risk budget.
3. Ambil fresh executable quote dari data/provider nyata.
4. Validasi liquidity, route, program IDs, price impact, fee estimate, min-out, dan quote expiry.
5. Terapkan latency, slippage, partial-fill, rejection, dan fee model.
6. Jalankan optional unsigned build/local instruction validation atau best-effort RPC simulation jika dapat dilakukan tanpa wallet.
7. Buat `SimulatedOrder` dan `SimulatedFill`; jangan membuat transaction signature.
8. Commit perubahan ke isolated virtual ledger.
9. Buka, kurangi, atau tutup simulated logical position.
10. Reconcile terhadap virtual ledger dan release virtual reservation.

`SignerAdapter`, `JitoSubmitAdapter`, `RpcSubmitAdapter::send_transaction`, dan endpoint provider yang mengeksekusi transaksi tidak boleh dipanggil. Enforcement dilakukan pada command handler, execution dispatcher, serta adapter capability layer sebagai defense in depth.

### 26.3 Priority Fees

Priority fee diestimasi menggunakan recent fee samples dan writable accounts relevan. Fee ceiling harus menjadi bagian risk policy.

Referensi: [Solana getRecentPrioritizationFees](https://solana.com/docs/rpc/http/getrecentprioritizationfees)

### 26.4 Simulation

Simulation wajib untuk live transaction kecuali emergency policy secara eksplisit mengizinkan jalur terbatas. Simulation memeriksa:

- program error;
- compute limit;
- token-account creation;
- transfer fees;
- min-out;
- expected balance delta;
- unexpected writable/signing accounts;
- allowlisted program IDs.

Referensi: [Solana simulateTransaction](https://solana.com/docs/rpc/http/simulatetransaction)

### 26.5 Transaction Delivery

- `JitoSubmitAdapter`: primary untuk low-latency/MEV-aware delivery.
- `RpcSubmitAdapter`: fallback.
- `JupiterSubmitAdapter`: optional configured route.

Referensi: [Jito low-latency transaction send](https://docs.jito.wtf/lowlatencytxnsend/)

---

## 27. Multi-Position Management

### 27.1 On-Chain vs Internal Position

Spot wallet menyimpan aggregate token balance. Beberapa strategy positions pada mint yang sama direpresentasikan sebagai logical lots pada internal ledger.

```text
On-chain balance: 15 SOL

Internal ledger:
- Position A / momentum: 10 SOL
- Position B / smart-money: 5 SOL
```

### 27.2 Position Model

```rust
pub struct Position {
    pub id: PositionId,
    pub wallet_id: Option<WalletId>,
    pub virtual_account_id: Option<VirtualAccountId>,
    pub trading_mode: TradingMode,
    pub strategy_id: StrategyId,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub entry_notional: Decimal,
    pub acquired_amount: Decimal,
    pub remaining_amount: Decimal,
    pub average_entry_price: Decimal,
    pub realized_pnl: Decimal,
    pub exit_plan: ExitPlan,
    pub status: PositionStatus,
    pub opened_at: DateTime<Utc>,
}
```

Invariant ownership posisi:

- posisi Live/Shadow yang wallet-backed memiliki `wallet_id=Some`;
- posisi Dry Run memiliki `wallet_id=None` dan `virtual_account_id=Some`;
- tepat satu account reference harus aktif untuk setiap posisi;
- query portfolio wajib memfilter `trading_mode` dan account scope agar simulated state tidak pernah tercampur dengan saldo on-chain.

### 27.3 Position Lifecycle

```text
Planned
→ EntryApproved
→ EntrySubmitted
→ Open
→ Managing
→ Reducing
→ ExitSubmitted
→ Closed
→ Reconciled
→ Evaluated
```

### 27.4 Exit Rules

- fixed stop-loss;
- take-profit bertingkat;
- trailing stop;
- break-even adjustment;
- time stop;
- thesis invalidation;
- liquidity collapse;
- oracle/DEX deviation;
- smart-money distribution;
- risk-engine forced reduction;
- emergency close.

Contoh rule representation:

```rust
pub enum ExitRule {
    PriceBelow(Decimal),
    PriceAbove(Decimal),
    ProfitPercent(Decimal),
    LossPercent(Decimal),
    TrailingPercent(Decimal),
    MaximumHoldingTime(Duration),
    LiquidityBelow(Decimal),
    CustomDeterministic(PolicyId),
}
```

### 27.5 Take-Profit Bertingkat

Contoh:

```text
TP1: close 25% pada +5%
TP2: close 25% pada +10%
TP3: close 25% pada +18%
Remainder: trailing stop 6%
```

Setiap exit membuat execution intent baru dan selalu mengambil fresh quote, melakukan simulation, signing, confirmation, dan reconciliation.

### 27.6 Balance Reservation

```text
Available balance
= confirmed on-chain balance
- pending transaction reservations
- open exit reservations
- minimum fee reserve
- locked operational reserve
```

Reservation key minimal mencakup wallet, mint/token-account, dan intent. Execution queue harus mencegah dua transaction menggunakan amount sama.

### 27.7 Multi-Position pada Dry Run

Dry Run mendukung jumlah dan lifecycle logical position yang sama dengan Paper/Live, termasuk beberapa strategi pada mint yang sama. Reservation dihitung dari virtual account dan tidak pernah menyentuh on-chain balance. Exit supervisor menjalankan TP, SL, trailing stop, time stop, partial close, serta close-all terhadap simulated executable quote. Semua position ID memiliki immutable `mode=DRY_RUN` dan `dry_run_session_id`.

---

## 28. Portfolio Accounting dan Reconciliation

### 28.1 Portfolio Ledger

Ledger menyimpan:

- confirmed balances;
- reserved balances;
- logical position lots;
- cost basis;
- realized PnL;
- unrealized PnL;
- fees;
- deposits/withdrawals;
- manual/external wallet movements.

Untuk Dry Run, field yang sama disimpan pada namespace virtual ledger terisolasi. `PortfolioSnapshot` harus menyertakan `account_kind`, `trading_mode`, dan `dry_run_session_id`. Endpoint aggregate default tidak boleh menjumlahkan virtual equity dengan on-chain equity.

### 28.2 Reconciliation Triggers

- application startup;
- transaction confirmed/finalized;
- periodic scheduled check;
- WebSocket/Yellowstone reconnect;
- balance mismatch;
- external wallet transfer;
- unknown signature;
- unclean shutdown.

### 28.3 Mismatch Policy

Jika actual balance tidak cocok:

1. Stop entry.
2. Mark affected positions `ReconciliationRequired`.
3. Fetch confirmed signatures dan transactions.
4. Classify external transfer vs missed internal event.
5. Rebuild ledger.
6. Require owner review jika ambiguity tersisa.

---

## 29. Market Data Model dan Aggregation

### 29.1 DataFreshness

```rust
pub struct DataFreshness {
    pub adapter_id: AdapterId,
    pub source_slot: Option<u64>,
    pub source_timestamp: Option<DateTime<Utc>>,
    pub received_at: DateTime<Utc>,
    pub latency_ms: u64,
    pub stale: bool,
}
```

### 29.2 Derived State

Market State Engine menghasilkan:

- last trade;
- synthetic bid/ask;
- OHLCV;
- rolling volume;
- volatility;
- buy/sell pressure;
- executable liquidity bands;
- cross-DEX spread;
- oracle deviation;
- price impact curve;
- data-quality score.

### 29.3 Historical Data

Runtime menyimpan normalized market events untuk:

- candle construction;
- indicator warm-up;
- backtest/replay;
- agent input reconstruction;
- incident analysis.

Retention harus configurable dan dapat melakukan compaction dari raw events menjadi candle/aggregate.

---

## 30. Persistence Model

### 30.1 Core Tables/Collections

```text
runtimes
runtime_leases
users
sessions
roles
permissions
adapters
adapter_capabilities
adapter_health_samples
credential_refs
wallets
wallet_policies
wallet_balance_snapshots
dry_run_sessions
virtual_accounts
virtual_balances
tokens
token_risk_snapshots
markets
pools
market_events
candles
wallet_labels
wallet_signals
strategies
strategy_versions
agent_definitions
agent_model_bindings
agent_cycles
agent_runs
agent_artifacts
trade_proposals
risk_decisions
execution_intents
balance_reservations
orders
simulated_orders
simulated_fills
transactions
transaction_attempts
positions
position_lots
exit_rules
position_events
portfolio_snapshots
telegram_connections
telegram_principals
notification_rules
notifications
audit_events
system_settings
```

Semua tabel trading yang digunakan lintas mode wajib memiliki `trading_mode` atau foreign key menuju execution/session scope. `dry_run_sessions` minimal menyimpan config snapshot, strategy version, risk-policy version, starting balances, fill/latency/fee model, start/end time, status, reset lineage, dan aggregate result. Simulated entity tidak boleh memiliki transaction signature atau status yang menyiratkan on-chain confirmation.

### 30.2 Append-Only Events

Critical domains menggunakan append-only event table untuk:

- trade proposal lifecycle;
- risk decisions;
- execution attempts;
- position transitions;
- wallet configuration;
- live mode activation;
- adapter credential changes;
- Telegram commands;
- user authentication/security events.

### 30.3 Schema Versioning

- semua agent output memiliki schema version;
- semua stored strategy config memiliki version;
- migration harus forward-only pada production;
- event payload perubahan besar menggunakan new event type/version;
- replay test wajib terhadap historical fixture.

---

## 31. REST API Design

Base path:

```text
/api/v1
```

### 31.1 Authentication

```text
POST   /auth/login
POST   /auth/logout
POST   /auth/refresh
GET    /auth/session
POST   /auth/totp/setup
POST   /auth/totp/verify
```

### 31.2 Runtime

```text
GET    /runtime
GET    /runtime/readiness
POST   /runtime/pause
POST   /runtime/resume
POST   /runtime/kill
POST   /runtime/mode
```

`POST /runtime/mode` harus menolak kombinasi capability yang tidak valid. Memilih `dry_run` tidak membutuhkan `wallet_id`; request yang menyertakan `enable_signing=true` atau `enable_submission=true` ditolak.

### 31.2.1 Dry Run

```text
POST   /dry-runs
GET    /dry-runs
GET    /dry-runs/{id}
POST   /dry-runs/{id}/start
POST   /dry-runs/{id}/pause
POST   /dry-runs/{id}/resume
POST   /dry-runs/{id}/stop
POST   /dry-runs/{id}/reset
GET    /dry-runs/{id}/portfolio
GET    /dry-runs/{id}/positions
GET    /dry-runs/{id}/orders
GET    /dry-runs/{id}/performance
```

Create request minimal berisi `starting_balances`, `strategy_ids`, `risk_policy_id`, dan simulation model. `wallet_id` bersifat opsional dan, jika diberikan, hanya boleh dipakai sebagai sumber read-only snapshot setelah explicit consent. Dry Run response selalu menyertakan `wallet_required=false`, `signing_enabled=false`, dan `submission_enabled=false`.

### 31.3 Adapters

```text
GET    /adapters
POST   /adapters
GET    /adapters/{id}
PATCH  /adapters/{id}
POST   /adapters/{id}/test
POST   /adapters/{id}/enable
POST   /adapters/{id}/disable
POST   /adapters/{id}/reconnect
GET    /adapters/{id}/health
GET    /adapter-types
```

Secret fields are write-only. GET response hanya mengembalikan masked metadata.

### 31.4 Wallets

```text
GET    /wallets
POST   /wallets/watch-only
POST   /wallets/automation
POST   /wallets/external/pair
GET    /wallets/{id}
GET    /wallets/{id}/balances
GET    /wallets/{id}/readiness
PATCH  /wallets/{id}/policy
POST   /wallets/{id}/lock
POST   /wallets/{id}/unlock
```

Tidak ada API untuk membaca private key.

### 31.5 Agents

```text
GET    /agents
GET    /agents/{id}
PATCH  /agents/{id}
GET    /agent-cycles
GET    /agent-cycles/{id}
POST   /agent-cycles/run
GET    /agent-runs/{id}
```

### 31.6 Market

```text
GET    /markets
GET    /markets/{pair}
GET    /markets/{pair}/candles
GET    /markets/{pair}/liquidity
GET    /markets/{pair}/signals
POST   /watchlist
DELETE /watchlist/{mint}
```

### 31.7 Proposals dan Risk

```text
GET    /proposals
GET    /proposals/{id}
POST   /proposals/{id}/approve
POST   /proposals/{id}/reject
GET    /risk/policy
PATCH  /risk/policy
GET    /risk/status
```

Approval endpoint tidak menerima arbitrary transaction bytes.

### 31.8 Positions dan Transactions

```text
GET    /positions
GET    /positions/{id}
POST   /positions/{id}/close
POST   /positions/{id}/reduce
POST   /positions/close-all
GET    /transactions
GET    /transactions/{signature}
GET    /portfolio
GET    /portfolio/history
```

Endpoint portfolio/position/order wajib menerima atau menginferensikan mode/account scope. Default response tidak boleh menggabungkan Dry Run, Paper, Shadow, dan Live. Mutating command terhadap Dry Run position diarahkan ke virtual executor meskipun client mencoba menyertakan wallet atau transaction field.

### 31.9 Telegram

```text
POST   /telegram/connections
POST   /telegram/connections/{id}/test
POST   /telegram/connections/{id}/pairing-code
GET    /telegram/principals
PATCH  /telegram/principals/{id}
DELETE /telegram/principals/{id}
POST   /telegram/webhook
```

---

## 32. Tauri IPC Contract

Tauri command menggunakan application use-case yang sama dengan REST API.

Contoh commands:

```text
get_runtime_status
list_adapters
create_adapter
test_adapter
enable_adapter
list_wallets
create_automation_wallet
unlock_wallet
list_positions
close_position
run_agent_cycle
set_trading_mode
create_dry_run_session
start_dry_run_session
stop_dry_run_session
reset_dry_run_session
activate_kill_switch
```

Rules:

- command input divalidasi di Rust;
- frontend permission tidak dianggap authorization boundary;
- Tauri capabilities/permissions dibuat minimum;
- secret commands tidak mengembalikan secret;
- signing hanya melalui intent ID.

---

## 33. WebSocket Event Protocol

Endpoint:

```text
GET /ws
```

Envelope:

```json
{
  "id": "evt_01...",
  "type": "agent.run.updated",
  "version": 1,
  "timestamp": "2026-09-08T14:30:00Z",
  "correlation_id": "cycle_1842",
  "payload": {}
}
```

Event categories:

```text
runtime.*
adapter.*
market.*
agent.cycle.*
agent.run.*
proposal.*
risk.*
execution.*
dry_run.*
transaction.*
position.*
portfolio.*
notification.*
security.*
```

Requirements:

- authenticated connection;
- RBAC filtering;
- bounded client queue;
- heartbeat/ping;
- resume cursor atau snapshot-after-reconnect;
- sequence number per runtime stream;
- tidak mengirim secrets;
- UI harus dapat recover setelah missed events.

---

## 34. Web dan Desktop UI Requirements

### 34.1 Design Language

- shadcn/ui primitives.
- Nuansa Jatevo.ai.
- Off-white background dan white operational surfaces.
- Near-black primary actions/text.
- Bright yellow untuk safety/status strip, bukan dekorasi umum.
- Purple/cyan/pink/orange sebagai small identity accents.
- Operational labels menggunakan mono typography.
- Radius kecil dan border tipis.
- Tidak menggunakan gradient berlebihan atau glassmorphism.
- Animasi hanya menyampaikan state.
- Accessibility dan reduced-motion support.

### 34.2 Primary Navigation

```text
Overview
Agent Field
Markets
Signals
Portfolio
Positions
Orders
Risk
Adapters
Wallets
Notifications
Audit Log
Settings
```

### 34.3 Command Center Dashboard

Menampilkan:

- trading mode;
- runtime status;
- portfolio equity;
- daily PnL;
- exposure;
- risk budget;
- market chart;
- liquidity/reference deviation;
- agent signal field;
- deterministic execution rail;
- latest proposal/decision;
- adapter warning;
- kill switch.

Mode selector harus menampilkan `Observe`, `Dry Run`, `Paper`, `Shadow`, dan `Live` sebagai state yang berbeda. Ketika Dry Run aktif:

- header menampilkan persistent badge `DRY RUN · NO ON-CHAIN EXECUTION`;
- wallet card menampilkan `Wallet not required`, bukan warning;
- equity, PnL, order, fill, dan position memakai label `Virtual`/`Simulated`;
- tombol atau command yang dapat menandatangani/mengirim transaksi tidak dirender dan tetap ditolak backend jika dipanggil langsung;
- warna Dry Run harus informatif dan tidak sama dengan hijau Live atau merah incident;
- perpindahan ke Live menggunakan activation flow terpisah dan tidak membawa simulated balances/positions.

### 34.3.1 Dry-Run Setup Flow

```text
Choose Dry Run
→ Select strategy
→ Set virtual starting capital
→ Select fee/slippage/latency preset
→ Review required market + LLM adapters
→ Start session
```

Setup flow tidak menampilkan wallet sebagai langkah wajib. UI boleh menawarkan `Use wallet as read-only context` sebagai opsi terpisah yang default-nya off. Sebelum start, readiness summary harus membedakan `Required`, `Optional`, dan `Not required`.

### 34.4 Agent Animation

Setiap agent memiliki state visual:

```text
Idle
Queued
Working
Waiting
Complete
Failed
TimedOut
Disabled
```

Animasi:

- subtle pulse untuk `Working`;
- moving signal pada edge dependency aktif;
- no animation untuk idle;
- error state tidak berkedip agresif;
- click agent membuka current task, input summary, output summary, duration, model, dan evidence;
- animation state berasal dari WebSocket runtime events.

### 34.5 Adapter UI

Per adapter:

- provider icon/name;
- status;
- capabilities;
- endpoint masked;
- credential configured indicator;
- latency;
- current/last slot;
- last event;
- rate-limit state;
- error detail;
- test, enable, disable, reconnect controls.

### 34.6 Wallet UI

- public address;
- wallet type;
- balance;
- fee reserve;
- lock status;
- execution lease owner;
- spend policy;
- allowed programs;
- readiness checklist;
- funding instructions;
- withdrawal hanya melalui explicit protected flow, bukan Telegram.

Jika mode aktif adalah Dry Run dan belum ada wallet, halaman ini menampilkan empty state edukatif dan tombol optional `Connect later`; tidak boleh memblokir navigasi atau menampilkan runtime error.

### 34.7 Responsive Requirements

- Desktop target utama 1024px ke atas.
- Tablet tetap dapat menampilkan core operational state.
- Mobile web bersifat monitoring-first.
- Risk-increasing configuration dapat dibatasi pada layar kecil.

---

## 35. Docker Deployment

### 35.1 One-Port Requirement

Default bind:

```text
0.0.0.0:8080
```

Satu port menyajikan:

```text
/                 React SPA
/assets/*          Frontend assets
/api/v1/*          REST API
/ws                WebSocket
/health/live       Liveness
/health/ready      Readiness
/telegram/webhook  Optional webhook
```

### 35.2 Image Build

Multi-stage build:

1. Build React/shadcn frontend.
2. Build optimized Rust server binary.
3. Copy binary, frontend dist, migrations, dan minimal runtime assets.
4. Run sebagai non-root user.
5. Expose port 8080.

### 35.3 Persistent Volume

```text
/data/
├── database/
├── vault/
├── market-history/
├── audit-log/
├── strategies/
├── models/          optional
└── backups/
```

### 35.4 Internal Dependencies

Local LLM, PostgreSQL, Redis, atau service lain dapat berada pada Docker internal network tanpa port host. Hanya Kairos Agent port yang diekspos.

### 35.5 Production Network

Jangan mengekspos raw HTTP port langsung ke internet. Rekomendasi:

```text
Internet/VPN
→ HTTPS reverse proxy atau private overlay network
→ kairos-agent:8080
```

Pilihan akses:

- Tailscale/private VPN;
- reverse proxy TLS;
- IP allowlist;
- identity-aware access proxy.

### 35.6 Server Readiness

`/health/live` hanya menilai process hidup.

`/health/ready` menilai:

- storage ready;
- vault ready;
- required adapters ready/degraded according to mode;
- market state warmed;
- reconciliation complete;
- execution lease untuk live mode;
- signer unlocked jika policy mengharuskan.

Pada Dry Run, readiness tidak menunggu wallet, signer, execution lease, confirmation adapter, maupun submission adapter. Response harus mengembalikan alasan eksplisit bahwa capability tersebut `NotRequired` atau `Prohibited` oleh mode aktif.

Container orchestrator tidak boleh restart hanya karena satu optional adapter degraded.

---

## 36. Security Requirements

### 36.1 Security Boundaries

1. Browser/React is untrusted input boundary.
2. Tauri IPC is permissioned but still validates all inputs.
3. LLM is untrusted/non-deterministic component.
4. External adapters are untrusted data sources.
5. Signer is highest-sensitivity boundary.
6. Telegram is remote control surface with limited authority.

### 36.2 Credential Rules

- encrypted at rest;
- never returned after creation;
- displayed masked;
- no plaintext logs;
- rotation supported;
- environment-specific separation;
- credential references instead of inline secrets;
- audit create/update/delete without secret value.

### 36.3 Web Authentication

- password hashing menggunakan memory-hard password hashing;
- secure, HttpOnly, SameSite session cookie;
- CSRF protection;
- session rotation;
- idle and absolute expiry;
- login rate limit;
- optional/required TOTP 2FA untuk Owner;
- RBAC server-side;
- audit authentication events.

### 36.4 Transaction Security

- transaction intent authorization;
- program ID allowlist;
- writable/signing account validation;
- maximum amount/slippage/fee;
- simulation;
- short expiry;
- consume-once authorization;
- fresh blockhash;
- signature confirmation;
- post-balance reconciliation.

### 36.4.1 Dry-Run Safety Invariants

- `TradingMode::DryRun` tidak dapat resolve `SignerAdapter` dari dependency container.
- Submission capability tidak diregistrasikan pada Dry Run execution context.
- Semua intent memiliki immutable mode dan session scope sejak dibuat.
- Intent Dry Run tidak dapat dikonversi atau dipromosikan menjadi Live intent; Live harus membuat proposal, risk decision, quote, dan authorization baru.
- API/IPC/Telegram tidak dapat mengubah mode pada existing intent.
- Simulated order tidak boleh memiliki signature field berisi nilai non-null.
- Egress call ke transaction submission method selama Dry Run menghasilkan hard error dan high-severity audit event.
- UI disablement bukan security boundary; backend selalu memverifikasi mode capability.

### 36.5 LLM Security

- prompt injection tidak dapat memberikan tool execution authority;
- agent tools are allowlisted and read-only by default;
- structured schema validation;
- output size and time limits;
- evidence refs must resolve to known data;
- no secrets in prompts;
- no arbitrary network access from agent runtime;
- model output cannot create arbitrary transaction.

### 36.6 Telegram Security

- numeric user ID authorization;
- one-time pairing;
- callback nonce dan expiry;
- role permissions;
- command rate limiting;
- no withdrawal;
- risk-increasing commands default disabled;
- `/resume` local-owner gated setelah severe kill;
- audit every command and response outcome.

### 36.7 Supply Chain

- dependency lockfiles committed;
- automated vulnerability scanning;
- signed application updates/releases;
- reproducible build target where feasible;
- minimal Docker image;
- non-root runtime;
- explicit outbound network policy.

---

## 37. Observability dan Audit

### 37.1 Metrics

Runtime:

- task restarts;
- event queue depth;
- CPU/memory;
- database latency;
- WebSocket clients;
- runtime mode.
- dry-run session count/status;
- prohibited signing/submission attempt count.

Adapters:

- connect status;
- request rate/error rate;
- latency p50/p95/p99;
- rate-limit remaining;
- stream lag;
- current slot;
- reconnect count;
- freshness.

Agents:

- runs;
- duration;
- timeout/error rate;
- invalid structured output rate;
- token usage/cost where available;
- confidence calibration;
- fallback usage.

Execution:

- quote latency;
- simulation latency/failure;
- submission latency;
- landing success;
- confirmation latency;
- blockhash expiry;
- expected vs actual output;
- realized slippage;
- priority fee.

Trading:

- PnL;
- drawdown;
- win/loss;
- exposure;
- position duration;
- strategy attribution;
- rejection reason distribution.
- dry-run virtual equity/PnL/drawdown;
- simulated fill rate dan modeled slippage;
- quote-to-simulated-fill latency;
- per-mode performance attribution tanpa cross-mode aggregation.

### 37.2 Structured Logging

Every log event includes when relevant:

- timestamp;
- level;
- runtime ID;
- correlation/cycle/proposal/position IDs;
- adapter ID;
- wallet public identifier;
- source slot;
- error class;
- no secret/private key.

### 37.3 Audit Log

Audit log wajib untuk:

- login/logout/failure;
- live mode activation;
- dry-run session create/start/pause/resume/stop/reset;
- setiap `ModeCapabilityViolation`, termasuk percobaan signing/submission saat Dry Run;
- risk policy change;
- wallet add/remove/unlock;
- adapter credential change;
- Telegram pairing/permission change;
- proposal approval/rejection;
- transaction signing/submission;
- kill/resume;
- user/role changes;
- execution lease acquisition/loss.

Audit record idealnya append-only dan tamper-evident hash-chain pada fase lanjutan.

---

## 38. Reliability dan Performance Targets

Initial engineering targets, bukan guarantee:

| Area | Target awal |
|---|---|
| Runtime availability Docker | 99.5% selama infrastructure tersedia |
| Adapter failure isolation | Tidak menjatuhkan seluruh runtime |
| Market event internal propagation | p95 < 250 ms setelah event diterima |
| UI live update | p95 < 500 ms dari internal state event |
| Agent cycle timeout | Configurable per agent dan cycle |
| Quote freshness | Slot/time-aware dan sangat pendek |
| Confirmation tracking | Sampai terminal status atau reconciliation queue |
| Data loss | Tidak kehilangan committed portfolio/position event |
| Restart recovery | Semua pending execution direkonsiliasi sebelum entry baru |

Latency target yang lebih agresif ditentukan setelah provider dan lokasi server dipilih.

---

## 39. Failure Scenarios

### 39.1 Yellowstone Disconnect

- mark degraded;
- switch fallback WebSocket jika policy memungkinkan;
- pause new entries jika freshness requirement tidak terpenuhi;
- reconnect dan resync snapshot;
- retain open-position deterministic protection melalui alternate price source.

### 39.2 RPC Divergence

- compare slot/state;
- quarantine outlier provider;
- no live action jika quorum hilang untuk critical data;
- alert operator.

### 39.3 LLM Timeout/Invalid JSON

- retry bounded;
- fallback model;
- validate schema;
- no trade jika tetap gagal;
- position exits tetap deterministic.

### 39.4 Jupiter Unavailable

- no new entry;
- direct DEX route hanya jika implemented, enabled, dan independently tested;
- emergency exit dapat memakai configured fallback route;
- never reuse stale quote.

### 39.5 Jito Failure

- classify submission result;
- check signature/status before retry;
- fallback RPC sesuai policy;
- prevent duplicate transaction intent.

### 39.6 Wallet Locked

- analysis/dry run/paper dapat lanjut;
- live signing unavailable;
- no attempt to bypass lock;
- UI/Telegram alert.

### 39.7 Database Unavailable

- no new live transaction if durable intent/audit cannot be recorded;
- runtime enters safe mode;
- in-flight confirmation may continue in controlled memory flow and reconcile after storage returns.

### 39.8 Telegram Compromised

- revoke principal dari UI;
- rotate bot token through BotFather then update connection;
- Telegram cannot withdraw funds;
- risk-increasing actions remain disabled by default.

### 39.9 External Wallet Movement

- detect balance mismatch;
- freeze affected reservations/positions;
- classify deposit/withdrawal;
- reconcile before new trade.

### 39.10 Wallet Tidak Dikonfigurasi

- Observe, Dry Run, dan Paper tetap dapat mencapai readiness sesuai dependency mode;
- UI menampilkan `Wallet not required for current mode`;
- Shadow features yang membutuhkan account-specific simulation diturunkan capability-nya secara eksplisit;
- Live tetap unavailable;
- sistem tidak membuat implicit wallet/keypair dengan private key;
- Dry Run membuat atau memuat virtual account tanpa cryptographic signing material.

---

## 40. Testing Strategy

### 40.1 Unit Tests

- fixed-point amount conversion;
- risk rules;
- position allocation;
- PnL/cost basis;
- reservation accounting;
- state transitions;
- token extension parsing;
- adapter error classification;
- Telegram authorization;
- transaction account validation.

### 40.2 Property-Based Tests

- balance never negative;
- reserved amount never exceeds confirmed available amount;
- position remaining amount never exceeds acquired amount;
- risk-approved amount never exceeds policy;
- repeated idempotent command does not duplicate position/order;
- reconciliation converges for known event sequences.
- Dry Run intent tidak pernah mencapai signing/submission state.
- virtual balance tidak pernah tercampur dengan wallet balance.

### 40.3 Adapter Contract Tests

Setiap adapter harus lulus common suite:

- connect/disconnect;
- authentication failure;
- timeout;
- rate limit;
- malformed response;
- schema drift fixture;
- reconnect;
- stale data;
- capability report;
- secret redaction.

### 40.4 Simulation/Devnet Tests

- transaction build;
- blockhash expiry;
- simulation failure;
- signature tracking;
- token account creation;
- Token-2022 transfer fee;
- RPC/Jito failover where supported.

### 40.5 Replay Tests

Recorded normalized event stream direplay untuk memastikan:

- market state deterministik;
- agent input dapat direkonstruksi;
- risk result konsisten;
- position lifecycle benar;
- fork/gap handling.

### 40.6 Paper Trading Soak Test

Sebelum live:

- runtime berjalan terus dalam periode yang disepakati;
- restart recovery diuji;
- adapter disconnect disimulasikan;
- LLM outage disimulasikan;
- RPC/Jito failure disimulasikan;
- position TP/SL diuji;
- no unbounded memory/queue growth;
- PnL model dibandingkan terhadap executable quote/market outcome.

### 40.6.1 Dry-Run Test Suite

- fresh install dapat memulai Dry Run tanpa wallet record;
- wallet adapter, signer, Jito submitter, dan RPC submitter dapat tidak dikonfigurasi;
- scanner, agents, risk engine, quote engine, virtual execution, multi-position, TP/SL, dan evaluator tetap berjalan;
- startup/restart memulihkan persistent Dry Run session atau menutup ephemeral session sesuai config;
- switching Dry Run ke mode lain tidak membawa virtual balance/reservation/position;
- direct REST/IPC attempt untuk sign/submit pada Dry Run ditolak;
- malicious LLM/tool output tidak dapat mengubah execution mode;
- notification, metrics, audit, orders, fills, positions, dan PnL memiliki mode label;
- reset session tidak menghapus audit trail dan menghasilkan reset-lineage event;
- optional best-effort RPC simulation gagal secara aman tanpa memblokir virtual fill policy yang telah dikonfigurasi.

### 40.7 Security Tests

- secret leakage scan;
- authorization/RBAC;
- CSRF/session tests;
- custom URL SSRF tests;
- arbitrary transaction signing attempts;
- Telegram replay/callback nonce;
- program allowlist bypass attempts;
- malicious LLM output;
- dependency vulnerability scan.

---

## 41. Acceptance Criteria

### 41.1 Desktop

- Aplikasi dapat dipasang dan berjalan pada target Windows/macOS yang ditetapkan.
- User dapat memilih local atau remote runtime.
- UI menampilkan live adapter/agent/position state.
- Closing window behavior dan tray mode jelas.

### 41.2 Docker/Web

- Image dapat dijalankan menggunakan Docker/Compose.
- Hanya satu application port perlu diekspos.
- SPA, REST, WebSocket, health endpoint bekerja pada port sama.
- Restart container tidak menghapus konfigurasi/ledger jika volume terpasang.
- Runtime dapat berjalan tanpa Tauri/GUI dependency.

### 41.3 Adapters

- User dapat menambah adapter melalui UI.
- Credential field write-only dan masked setelah save.
- Test connection menampilkan identity, capability, dan health.
- Adapter reconnect tidak menjatuhkan runtime.
- Required adapter degradation memblokir action yang bergantung padanya.

### 41.4 LLM

- User dapat input base URL, API key, dan model.
- Local provider dapat digunakan tanpa API key.
- Agent dapat dipetakan ke primary/fallback model.
- Invalid structured output tidak mencapai risk/execution pipeline.
- Total LLM outage menghasilkan no-trade, bukan random fallback decision.

### 41.5 Telegram

- Bot token dapat dikonfigurasi dari UI.
- Bot identity dapat diuji.
- User dapat pairing tanpa input manual chat ID.
- Unauthorized user tidak dapat menggunakan command.
- Monitoring dan alert berfungsi.
- Sensitive callback memiliki nonce dan expiry.
- Tidak ada withdrawal command.

### 41.6 Wallet

- Watch-only wallet dapat dimonitor.
- Dedicated automation wallet dapat dibuat dan disimpan terenkripsi.
- Private key tidak pernah dikembalikan lewat API setelah secure setup flow.
- Wallet readiness menilai balance, fee reserve, signer, lease, dan policy.
- Satu wallet tidak dapat memiliki dua execution authority aktif.
- Ketiadaan wallet tidak memblokir Observe, Dry Run, atau Paper readiness.

### 41.7 Trading

- Observe, dry run, paper, shadow, dan live state dapat dibedakan.
- Dry Run dapat dimulai pada fresh install tanpa wallet, signer, fee reserve, atau execution lease.
- Dry Run menjalankan discovery hingga simulated entry/exit dan menghasilkan virtual PnL.
- Dry Run tidak pernah memanggil signer atau transaction submitter.
- Dry-run ledger, balance, order, fill, position, dan performance terisolasi dari Paper/Live.
- Semua surface menampilkan label Dry Run secara persisten.
- Multi-position pada token berbeda didukung.
- Logical positions pada token sama dapat dialokasikan dan ditutup terpisah.
- Balance reservation mencegah double allocation.
- TP/SL/trailing/time-stop dapat berjalan tanpa LLM.
- Tidak ada transaction signing tanpa consumed-once approved intent.
- Confirmed transaction direkonsiliasi dengan actual balance.

### 41.8 Risk dan Security

- Risk engine dapat memveto seluruh proposal.
- Kill switch menghentikan entry baru segera.
- Severe mismatch memasukkan runtime ke safe mode.
- Audit log merekam semua critical state changes.
- Secret tidak muncul pada logs, API responses, atau WebSocket events.

---

## 42. Roadmap Implementasi

### Phase 0 — Foundation dan Threat Model

- Finalisasi scope strategi awal.
- Rust workspace dan domain types.
- Error taxonomy.
- Runtime supervisor.
- Storage abstraction dan migrations.
- Security/threat model.
- React/shadcn design system.

### Phase 1 — Observe-Only Market Runtime

- Solana RPC HTTP/WebSocket.
- Yellowstone adapter.
- Market state normalization.
- Raydium/Orca/Meteora baseline decoder.
- Pyth dan Jupiter Price.
- Token intelligence baseline.
- Dashboard market/adapter health.
- Event persistence dan replay.

### Phase 2 — Multi-Agent Analysis

- LLM adapter UI.
- Provider capability detection.
- Agent definitions/bindings.
- Four-agent MVP orchestration.
- Structured outputs.
- Agent field animation dari real runtime state.
- Proposal generation tanpa execution.

### Phase 3 — Paper Trading dan Portfolio

- Deterministic risk engine.
- Dry Run tanpa wallet dan isolated virtual-account sessions.
- Mode-aware readiness serta execution capability guard.
- Paper execution adapter.
- Portfolio ledger.
- Multi-position manager.
- TP/SL/trailing/time-stop.
- PnL dan performance reporting.
- Backtest/replay comparison.

### Phase 4 — Wallet dan Live Transaction Pipeline

- Dedicated automation wallet.
- Secure vault.
- Jupiter `/build` integration.
- Simulation dan transaction validation.
- Jito/RPC submission.
- Confirmation dan reconciliation.
- Devnet/integration tests.
- Shadow mode.

### Phase 5 — Telegram dan Remote Operations

- Bot token UI.
- Long polling.
- Pairing dan RBAC.
- Alerts dan monitoring commands.
- Pause/kill/reject.
- Optional close command dengan confirmation.

### Phase 6 — Docker/Web Production Mode

- Axum host.
- REST/WebSocket authentication.
- Single-port serving.
- Docker multi-stage build.
- Persistent volume.
- Remote Tauri client mode.
- Execution lease.
- TLS/reverse-proxy deployment guide.

### Phase 7 — Controlled Live Pilot

- Paper soak-test exit criteria.
- Small dedicated wallet.
- Conservative limits.
- Manual approval mode.
- Incident drill.
- Gradual automation enablement.

### Phase 8 — Advanced Intelligence

- GMGN integration.
- wallet PnL/scoring;
- clustering;
- Pump.fun/PumpSwap/LaunchLab;
- social signal;
- six-agent separation;
- advanced backtesting/calibration.

---

## 43. Open Decisions

Keputusan berikut belum final dan harus ditetapkan sebelum implementation detail terkait dikunci:

1. Universe token pertama.
2. Timeframe strategi pertama.
3. Apakah MVP hanya SOL/USDC atau beberapa aset likuid.
4. Entry/exit strategy pertama.
5. Risk budget dan maximum exposure.
6. Take-profit tiers dan stop-loss policy.
7. Manual approval vs automatic entry pada controlled live pilot.
8. Primary Yellowstone/RPC provider.
9. Primary LLM provider dan local model runtime.
10. SQLite-only vs early PostgreSQL support untuk Docker.
11. External-wallet pairing mechanism yang paling kompatibel dengan Tauri.
12. Desktop tray behavior.
13. Telegram close-position permission default.
14. Data retention dan raw market-event storage budget.
15. Server deployment target/location untuk latency ke Solana infrastructure.
16. Licensing/distribution model produk.
17. Regulatory dan compliance assessment sebelum digunakan pihak lain.

---

## 44. Definition of Ready untuk Live Trading

Live trading tidak boleh tersedia hanya karena wallet sudah funded. Seluruh gate berikut harus terpenuhi:

```text
[ ] Strategy version immutable dan approved
[ ] Backtest/replay results reviewed
[ ] Paper soak test completed
[ ] Dry Run end-to-end test completed tanpa wallet
[ ] Shadow mode completed
[ ] Risk policy configured
[ ] Kill switch tested
[ ] Wallet uses limited dedicated capital
[ ] Fee reserve configured
[ ] Required adapters redundant/ready
[ ] Quote/simulation pipeline validated
[ ] Program allowlist enabled
[ ] Confirmation/reconciliation tested
[ ] Restart recovery tested
[ ] Telegram/security roles verified
[ ] Audit logging operational
[ ] Owner explicitly activates live mode
```

---

## 45. Example End-to-End Scenario

### 45.1 Discovery sampai Entry

1. Yellowstone menerima swap/volume events SOL/USDC.
2. Market State Engine memperbarui candle, volatility, dan liquidity.
3. Opportunity Scanner melihat momentum/liquidity criteria terpenuhi.
4. Token risk checks lulus.
5. Orchestrator memulai cycle `#1842`.
6. Market Structure Agent dan On-chain Intelligence Agent berjalan paralel.
7. Strategy Agent menghasilkan BUY proposal dengan expiry pendek.
8. Risk Analyst memberi advisory.
9. Deterministic Risk Engine memeriksa exposure dan loss limits.
10. Risk Engine menghasilkan `ApprovedTradeIntent` dengan maximum notional.
11. Quote Engine meminta Jupiter `/build` quote terbaru.
12. Route/program/accounts diverifikasi.
13. Transaction disimulasikan.
14. Dedicated wallet signer menandatangani authorized intent.
15. JitoSubmitAdapter mengirim transaction.
16. Confirmation tracker melihat `confirmed`.
17. Reconciliation membaca actual output balance.
18. Position Manager membuka logical position.
19. Telegram mengirim notifikasi entry confirmed.

### 45.1.1 Dry Run Tanpa Wallet

1. Pengguna memilih Dry Run pada fresh install tanpa menambah wallet.
2. Pengguna membuat virtual account dengan starting balance 10.000 virtual USDC.
3. Runtime memastikan signer dan submission capability tidak diregistrasikan.
4. Market adapters tersambung dan Market State Engine warm.
5. Scanner menemukan candidate dan Orchestrator memulai cycle.
6. Specialist agents menganalisis data real-time.
7. Strategy Agent menghasilkan `TradeProposal` berlabel `DRY_RUN`.
8. Risk Engine memeriksa proposal terhadap virtual balance dan risk policy.
9. Quote Engine mengambil fresh executable quote.
10. Dry-run executor menerapkan fee, slippage, latency, dan fill model.
11. `SimulatedOrder` dan `SimulatedFill` dicatat tanpa signature.
12. Virtual ledger membuka logical position dan mengurangi virtual quote balance.
13. Position Supervisor memonitor TP/SL terhadap executable market price.
14. Ketika exit condition tercapai, simulated close fill dibuat.
15. Virtual PnL, drawdown, dan evaluation report diperbarui.
16. UI dan Telegram menampilkan label `[DRY RUN]` pada seluruh hasil.

### 45.2 Take Profit

1. Position Supervisor memonitor executable price.
2. TP1 condition terpenuhi.
3. Supervisor membuat risk-reducing exit intent untuk 25% position.
4. Quote Engine meminta fresh sell quote.
5. Transaction dibangun dan disimulasikan.
6. Signer menandatangani.
7. Transaction disubmit dan confirmed.
8. Position remaining amount dikurangi.
9. Realized PnL dan fee dicatat.
10. Telegram mengirim TP notification.
11. Post-Trade Evaluator menilai expected vs actual execution.

### 45.3 Adapter Failure saat Posisi Terbuka

1. Yellowstone stream berhenti.
2. Adapter berubah `Degraded`.
3. Entry baru dihentikan.
4. Fallback WebSocket/Pyth/Jupiter price tetap memberi monitoring input.
5. Open position supervisor terus mengevaluasi hard exit rules jika freshness policy terpenuhi.
6. Adapter supervisor reconnect.
7. Snapshot/resync dijalankan.
8. State dibandingkan dan adapter kembali `Ready`.
9. Entry baru hanya diaktifkan setelah readiness dependency terpenuhi.

---

## 46. Glossary

| Istilah | Definisi |
|---|---|
| Adapter | Implementasi konektivitas terhadap provider/protocol tertentu |
| Agent | Komponen AI dengan peran analisis/koordinasi tertentu |
| TradeProposal | Proposal terstruktur dari Strategy Agent, bukan transaksi |
| ApprovedTradeIntent | Otorisasi deterministik dan terbatas dari Risk Engine |
| Execution Lease | Hak eksklusif satu runtime untuk mengeksekusi wallet tertentu |
| Reservation | Saldo/risk budget yang sementara dikunci untuk pending intent |
| Reconciliation | Proses menyamakan state internal dengan state on-chain |
| Reference price | Harga indikatif/fair price, bukan executable quote |
| Executable quote | Estimasi output aktual dari route dengan amount, fee, dan slippage |
| Safe Mode | Mode yang menghentikan risk-increasing action karena ketidakpastian |
| Dry Run | Mode end-to-end dengan virtual account dan simulated execution, tanpa kewajiban wallet serta tanpa signing/submission on-chain |
| Virtual Account | Account ledger internal tanpa keypair/private key yang menyimpan simulated balances |
| Simulated Fill | Hasil eksekusi hipotetis berdasarkan quote dan fill/fee/slippage/latency model |
| Shadow Mode | Pipeline produksi tanpa submit transaksi |
| Position lot | Alokasi internal token balance untuk strategy/position tertentu |
| Freshness | Usia dan relevansi slot/timestamp sebuah data |

---

## 47. Referensi Teknis Resmi

### Solana

- [Solana RPC overview](https://solana.com/docs/rpc)
- [Solana WebSocket methods](https://solana.com/docs/rpc/websocket)
- [Solana sendTransaction](https://solana.com/docs/rpc/http/sendtransaction)
- [Solana simulateTransaction](https://solana.com/docs/rpc/http/simulatetransaction)
- [Solana priority fees](https://solana.com/docs/rpc/http/getrecentprioritizationfees)
- [Solana confirmation levels](https://solana.com/docs/payments/production-readiness)
- [Solana Token Extensions](https://solana.com/docs/tokens/extensions)
- [Solana Wallet Standard migration](https://solana.com/docs/frontend/web3-compat)

### Solana DeFi Infrastructure

- [Jupiter Swap V2](https://developers.jup.ag/docs/swap)
- [Jupiter Price API](https://developers.jup.ag/docs/price)
- [Jupiter Tokens API](https://developers.jup.ag/docs/tokens)
- [Pyth Price Feeds](https://docs.pyth.network/price-feeds)
- [Jito low-latency transaction send](https://docs.jito.wtf/lowlatencytxnsend/)
- [Raydium documentation](https://docs.raydium.io/)
- [Orca developer overview](https://docs.orca.so/developers/overview)

### Application Platform

- [Tauri 2 documentation](https://v2.tauri.app/)
- [Tauri Stronghold](https://v2.tauri.app/plugin/stronghold/)
- [Tauri security](https://v2.tauri.app/security/)
- [Reown Solana integration](https://docs.reown.com/appkit/networks/solana)

### Telegram

- [Telegram Bot API](https://core.telegram.org/bots/api)
- [Telegram Bot features](https://core.telegram.org/bots/features)
- [Telegram webhook guide](https://core.telegram.org/bots/webhooks)

---

## 48. Kesimpulan Arsitektur

Kairos Agent dibangun sebagai shared Rust trading core yang dapat dijalankan oleh Tauri desktop atau Axum Docker server. React/shadcn UI yang sama digunakan pada desktop maupun browser. Docker server expose satu port untuk UI, API, WebSocket, health endpoint, dan optional Telegram webhook.

Enam AI agent production menangani orchestration, market analysis, on-chain intelligence, strategy synthesis, advisory risk analysis, dan post-trade evaluation. Semua aktivitas finansial kritis tetap dikendalikan oleh deterministic Rust services.

Adapter dikonfigurasi melalui UI, memiliki capability-level readiness, serta lifecycle connect, authenticate, sync, warm, ready, degraded, reconnect, dan fail. Credential tersimpan terenkripsi dan tidak dikembalikan ke frontend.

Sistem mendukung multi-position melalui internal position ledger, balance reservation, deterministic exit supervision, fresh quotes, simulation, signing boundary, confirmation tracking, dan reconciliation. Untuk live automation, wallet utama dipisahkan dari dedicated automation wallet dengan modal terbatas.

Telegram menjadi remote observability dan risk-control surface. LLM menjadi pluggable analysis provider. Tidak satu pun dari keduanya dapat melewati risk engine, signer policy, dan execution validation.

Dokumen ini menjadi baseline product dan engineering. Semua perubahan pada authority model, signer access, execution path, risk policy, atau live-mode gate harus memperbarui PRD dan menghasilkan review keamanan baru.

Mode Dry Run menjadi jalur onboarding dan validasi utama sebelum wallet dikonfigurasi. Ia mempertahankan fidelity pada discovery, multi-agent analysis, deterministic risk, quote, multi-position, exit supervision, dan performance evaluation, tetapi secara structural tidak memiliki kemampuan signing maupun submission on-chain.
