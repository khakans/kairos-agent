# Cakupan implementasi terhadap PRD

Instruksi owner menetapkan dua mode: Dry Run dan Live. Ini adalah implementasi awal yang bisa dijalankan dan diuji, **belum pemenuhan seluruh acceptance criteria PRD**.

| Area | Implementasi saat ini | Batas yang belum selesai |
|---|---|---|
| UI | Atomic design, 13 navigasi, forms, responsive, accessible modal, persistent mode labels | Advanced charts dan runtime animation tiap step belum tersedia |
| Hosts | Shared Rust core, Tauri standalone, authenticated Axum, SPA/API/WS satu port | Desktop remote-client selection dan tray belum tersedia |
| Dry Run | Data live Jupiter Price V3/quote-only V2, walletless virtual ledger, fee/slippage, AI open/close/hold, approval, exit supervisor, reset lineage | Estimasi fill tidak memodelkan latency/partial fills; market terbatas SOL/USDC |
| Live | Jupiter V2 build + Price V3, dua RPC, restricted route, actual signer, simulation, submission, confirmation, reconciliation | Actual-provider/mainnet validation belum dilakukan; tidak ada devnet Jupiter route |
| Wallet | Generate/import dedicated keypair, Argon2 + ChaCha20-Poly1305 vault, write-only secrets, lock/unlock | Watch-only, external-wallet pairing, Stronghold/OS secret integration, credential rotation belum tersedia |
| Risk | Notional/exposure/open positions/loss ceiling/slippage/freshness/fees, TP/SL/trailing/time-stop, kill latch | Loss ceiling menggunakan cumulative scoped realized PnL, lebih ketat dari reset harian; drawdown/correlation/cooldown belum tersedia |
| Persistence | SQLite WAL/FULL, atomic snapshot/receipt/journal outbox, daily JSON trade-log/YYYY/MM/DD, recovery/idempotency, restart pauses, namespace per account/mode | Ledger snapshot belum dinormalisasi; pagination/retention jangka panjang belum tersedia |
| Security | Strict commands, no arbitrary signing, server auth, origin/header CSRF guard, cookie expiry, login rate limit, process/wallet locks | Owner-only auth; multi-user RBAC/TOTP, distributed cross-host lease dan security audit belum tersedia |
| Agents | Empat peran dengan inference terpisah: Orchestrator, Market Analyst, On-chain Analyst, Strategy & Evaluation; kedua analyst paralel; live mint evidence; strict reports/open-close-hold; block veto; verified file retrieval (32 outcomes/64 KB); per-agent audit dan decision/order linkage | Siklus dipicu owner; satu konfigurasi provider/model bersama; fine-tuning, analisis holder/pool/wallet-flow mendalam belum tersedia |
| Other adapters | Connection readiness yang jujur; provider credentials terenkripsi | Yellowstone, Pyth, GMGN, Jito, Telegram, protocol decoder/discovery belum tersedia |
| Deployment | Dockerfile multi-stage/non-root, Compose persistent volume, CI check workflow | Docker image runtime belum diuji karena daemon lokal tidak aktif |

## Threat model dan fail-closed boundaries

- Browser, IPC inputs, provider JSON, trade memory, dan LLM output dianggap tidak terpercaya.
- Financial state hanya dimutasi Rust. Decimal/string wire representation menghindari float untuk saldo.
- Dry Run tidak memperoleh `LiveExecutor`; live prepare menolak mode dry sebelum panggilan jaringan.
- Authorization berasal dari `RiskPolicy`; approved intent tidak bisa dibuat langsung dari JSON.
- Entry dan exit authorization berbeda. Exit terikat jumlah token yang boleh dikurangi.
- Jupiter response dibandingkan dengan amount/mint/slippage intent, route discriminator, signer, source/destination ATAs dan program allowlist. Address lookup tables dibaca ulang dari RPC.
- Simulation wajib; token-account mint/owner/delegate/frozen state, balance delta, min output, fee reserve dan CPI program diperiksa.
- Private key tidak dikirim ke UI. Vault tidak dapat dioverwrite. API error tidak mencetak provider request URL/header/response secrets.
- Signature pending dipersist sebelum network send. Tidak ada automatic retry/re-sign. Ambiguous/failing transactions mempertahankan pending lock sampai tersedia rekonsiliasi yang benar; resolusi failed/expired yang aman masih perlu diperluas sebelum pilot.
- SQLite/journal write failure melatch kill switch. JSON log diperiksa terhadap durable outbox sebelum masuk konteks AI; log berisi data dan tidak memberi execution authority. File lock mencegah dua runtime pada satu database. Wallet lease berlaku pada filesystem yang dibagi, bukan distributed lease terpisah.
- Operator validation attestation dan activation phrase mencegah live activation hanya karena wallet funded. Attestation file bukan bukti otomatis pengujian.

## Simplifikasi yang perlu ditingkatkan sebelum unattended production

`ponytail:` satu application mutex menserialisasi financial use cases agar tidak ada double allocation. Kill switch menggunakan atomic flag di luar mutex agar dapat membatalkan sign/submit ketika RPC masih berlangsung. Ganti dengan queue per account bila throughput/latency diperlukan; jangan menduplikasi financial logic di host.

`ponytail:` snapshot ledger aktif di memori dan SQLite cocok untuk pilot terbatas; normalisasi/pagination/retention diperlukan untuk runtime jangka panjang. Audit memiliki durable rows tersendiri dan UI hanya menampilkan 150 event terakhir.

Jupiter V2 hanya menerima route variant exact-in yang dikenali; variant baru dan unsupported setup/cleanup ditolak, bukan dicoba otomatis. Parameter/provider schema drift membutuhkan fixture update serta actual unsigned simulation sebelum whitelist diperluas.
