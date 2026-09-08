# Kairos Agent

Command center Solana dengan dua mode: **Dry Run** dan **Live**. React/TypeScript menggunakan atomic design; domain, policy, ledger, dan execution berada di Rust bersama untuk Tauri dan Axum.

**Status: implementation preview, bukan rilis live-production.** Jalur Live benar-benar memiliki build, simulation, signing, submission, dan reconciliation. Pengujian dalam repository menggunakan fixture, tidak menggunakan dana dan tidak membuktikan kompatibilitas mainnet. Integrasi penuh multi-agent AI dalam PRD masih belum selesai; lihat [cakupan implementasi](IMPLEMENTATION.md).

## Menjalankan

Prasyarat: Node 24+, Rust stable dengan dukungan edition/dependency lockfile saat ini, dan C++ Build Tools/WebView2 untuk Tauri Windows. Di PowerShell yang memblokir `npm.ps1`, gunakan `npm.cmd` seperti contoh berikut.

```powershell
npm.cmd ci
npm.cmd run build
npm.cmd run server
```

Buka **http://127.0.0.1:8080**. Server membuat owner token acak di `.kairos-data/owner-token`. Salin token dari file itu ke formulir login; token tidak dicetak ke log atau disimpan dalam localStorage. Session browser menggunakan cookie HttpOnly, SameSite=Strict dan expiry satu jam.

Frontend development, di terminal kedua:

```powershell
npm.cmd run dev
```

Buka http://localhost:1420; Vite mem-proxy API/WebSocket ke server 8080.

Desktop standalone:

```powershell
npm.cmd run tauri -- dev
# Build executable debug tanpa installer:
npm.cmd run tauri -- build --debug --no-bundle
```

Desktop menggunakan IPC langsung ke core dan data directory aplikasi OS (`com.kairosagent.desktop`). Menutup aplikasi menghentikan supervisornya; tidak ada tray/headless desktop. Desktop dan server adalah runtime terpisah, jangan mengoperasikan wallet sama di keduanya tanpa lease directory yang sama.

## Dry Run

1. Klik **New dry run**, isi nama, virtual capital, fee dan slippage.
2. Klik **Start session**, lalu **Run cycle**.
3. Periksa proposal; **Approve simulation** melewati risk engine sebelum virtual fill.
4. Posisi dapat ditutup manual atau oleh TP/SL/trailing/time-stop supervisor.
5. **Pause** menghentikan entry, tetapi supervisor posisi tetap berjalan. **Kill** membatalkan proposal dan mengunci entry.
6. Dari Settings, **Stop session → Reset as new session** membuat account baru dengan parent ID. Ledger lama dan audit tetap tersimpan.

**Dry Run dan Live sama-sama menggunakan data pasar live Jupiter**, tanpa harga awal dummy atau fallback replay. Price V3 diperiksa terhadap confirmed slot RPC mainnet; tiap fill mengambil quote baru. Dry Run mengestimasi hasil dari output quote dikurangi slippage dan fee konfigurasi, tanpa signer, signature, wallet lease, atau submission on-chain. Quote dry menggunakan endpoint quote-only `/swap/v2/order` tanpa taker; Live juga memakai referensi yang sama lalu memvalidasi kembali route Whirlpool lewat `/build` sebelum signing. Hasil virtual tetap estimasi, bukan jaminan fill Live.

Session replay dari versi lama dipertahankan sebagai histori dan dihentikan. Gunakan **Stop session → Reset as new session**; data replay tidak menjadi memori trading live.

### Koneksi market dan AI untuk kedua mode

Atur environment pada host Rust sebelum menjalankan server atau Tauri. Konfigurasi tidak disimpan di browser atau trade log. Lihat [.env.example](.env.example). Docker Compose membaca `.env`; server/Tauri membaca environment proses, bukan file `.env` secara otomatis.

| Variabel | Isi |
|---|---|
| `KAIROS_JUPITER_API_KEY` | API key Jupiter untuk data read-only, tidak memerlukan wallet |
| `KAIROS_MARKET_RPC_URL` | RPC HTTPS Solana mainnet untuk validasi cluster dan freshness |
| `KAIROS_AI_URL` | URL lengkap endpoint chat completions dengan JSON mode; HTTPS remote atau HTTP loopback untuk model lokal |
| `KAIROS_AI_MODEL` | Nama model yang didukung provider tersebut |
| `KAIROS_AI_API_KEY` | Bearer key provider; opsional untuk model lokal tanpa autentikasi |

Restart runtime setelah konfigurasi. **Adapters** menampilkan status koneksi; feed dipoll setiap lima detik di kedua mode, termasuk sebelum wallet Live di-unlock. **Run cycle** memanggil model sungguhan dengan pasar, posisi, policy, dan histori trade. Model dapat memilih **open**, **close**, atau **hold**. Open/close menghasilkan proposal yang memerlukan approval owner. Supervisor TP/SL/trailing/time-stop tetap deterministik. Ketiadaan konfigurasi, provider error, data stale, JSON invalid, atau keputusan di luar batas risk tidak diganti respons dummy.

### Log harian dan memori AI

Empat agent aktif dalam satu siklus, masing-masing dengan prompt, pemanggilan model, hasil, status, dan cycle ID yang dapat ditelusuri:

1. **Orchestrator** menyusun fokus analisis dari market, posisi, policy, dan outcome trade sebelumnya.
2. **Market Analyst** menilai harga Jupiter, histori pengamatan, exposure, serta pengaruh biaya/PnL historis.
3. **On-chain Analyst** membaca mint WSOL/USDC dari RPC mainnet (`getMultipleAccounts`, confirmed, jsonParsed): token program, initialization, decimals, supply, mint/freeze authority, dan source slot. Data ini bukan audit lengkap holder, pool, atau wallet flow; informasi yang belum diambil dinyatakan sebagai batas analisis.
4. **Strategy & Evaluation** menyatukan tiga laporan dan evidence terbaru untuk open/close/hold. Keputusan yang mengabaikan assessment `block` ditolak oleh Rust.

Kedua analyst berjalan paralel setelah Orchestrator; Strategy berjalan setelah keduanya berhasil. Kegagalan/schema invalid menandai agent terkait Failed dan tahap yang bergantung padanya Skipped, tanpa proposal baru. Seluruh siklus dibatasi 75 detik; pasar diperbarui sebelum analisis dan sintesis akhir, lalu freshness/risk tetap dicek sebelum eksekusi. Semua peran memakai konfigurasi provider/model `KAIROS_AI_*` yang sama, dengan **empat request inference terpisah per siklus**. Tidak ada fallback fixture dalam runtime normal. Kontrak pembacaan mint mengikuti [Solana getMultipleAccounts](https://solana.com/docs/rpc/http/getmultipleaccounts).

Default server menyimpan log di **`.kairos-data/trade-log/YYYY/MM/DD/<event-uuid>.json`**; jika `KAIROS_DATA_DIR` diubah, folder mengikuti data directory tersebut. Desktop memakai `<app_data_dir>/trade-log/`; Docker `/data/trade-log/`. Tanggal folder mengikuti **Asia/Jakarta (UTC+7)**, sedangkan timestamp event adalah Unix UTC. Path aktif terlihat di Adapters.

Selama menunggu inference, runtime tetap menjalankan supervisor posisi setiap lima detik. Bila posisi ditutup oleh exit rule, state posisi dan trade memory diperbarui sebelum sintesis akhir. Keputusan close juga memeriksa ulang bahwa posisi masih Open; respons AI lama tidak boleh menutup posisi yang sudah selesai.

Catatan mencakup request execution, keputusan/model AI beserta ID memori yang dibaca, market/slot/quote, mode/account, policy, ID posisi/order, fill/penolakan/pending/confirmation, biaya, dan realized PnL. Private key, passphrase, API key, dan URL RPC bercredential tidak dimasukkan. Satu event menjadi satu JSON immutable agar retry tidak menduplikasi baris atau meninggalkan JSONL setengah tertulis.

Event `agent.started`, `agent.completed`, `agent.failed` dan `agent.cycle.failed/completed` merekam hasil tiap peran. Evidence keputusan yang terhubung ke order memuat ketiga laporan analyst/orchestration, snapshot pasar/on-chain yang dianalisis, evidence terbaru, dan ID histori yang digunakan. Outcome berikutnya membawa evidence ini kembali ke konteks keempat agent.

State, receipt, dan journal outbox di-commit bersama di SQLite. File disinkronkan dan dipublikasikan sebelum submission; kegagalan write menghentikan eksekusi. Restart menyelesaikan outbox dan memulihkan file hilang dari salinan SQLite. File yang isinya berbeda ditolak saat verifikasi, bukan ditimpa diam-diam. Backup SQLite dan direktori log bersama; jangan mengedit log untuk memberi instruksi kepada agent.

Siklus berikutnya benar-benar **membaca file log** dan mencocokkannya dengan outbox, lalu menyertakan hingga 32 outcome terbaru (maksimum 64 KB) dari kedua mode dengan penanda simulasi/aktual. Model menggunakan hasil entry, close, biaya, dan kerugian untuk pertimbangan berikutnya. Ini adalah memori kontekstual dengan retrieval, **bukan fine-tuning otomatis**. Data histori yang dipilih dikirim hanya ke provider AI yang Anda konfigurasi.

Kontrak upstream mengikuti dokumentasi [Jupiter Price V3](https://developers.jup.ag/docs/api-reference/price) dan [quote-only Swap V2](https://developers.jup.ag/docs/swap/order-and-execute).

## Live

Live pertama dibatasi pada **USDC ↔ wrapped SOL**, Jupiter Swap V2 `/build`, Orca Whirlpool, serta SPL Token klasik. Ini merupakan workflow dengan approval owner per proposal; belum unattended AI entry.

1. Pilih **Live → Set up live** atau Wallets. Konfigurasikan dua RPC HTTPS mainnet dari provider berbeda, API key Jupiter, dan passphrase vault minimal 12 karakter.
2. Import **dedicated** Solana keypair JSON, atau biarkan kosong untuk membuat wallet baru. Tidak ada endpoint ekspor private key. Untuk wallet baru, backup file vault terenkripsi bersama passphrase sebelum pendanaan.
3. Buat USDC dan wrapped-SOL associated token accounts melalui alat wallet yang Anda percayai. Keduanya harus sudah ada. Auto-wrap/unwrap, transfer umum, withdrawal, serta arbitrary transaction tidak disediakan. Simpan sedikitnya 0.0201 SOL untuk fee reserve.
4. Unlock wallet lalu **Refresh balances**. Cluster dan slot dari kedua RPC harus konsisten.
5. Selesaikan gate validasi di bawah. Baru kemudian **Activate live** dengan frasa `ACTIVATE LIVE`.
6. Aktivasi berlaku 15 menit. **Run live cycle** mengevaluasi pasar dan trade memory menggunakan provider AI yang dikonfigurasi. **Approve live trade** memicu fresh quote, risk checks, reference-price comparison, instruction validation, RPC simulation, signing, dan satu RPC submission.
7. Signature disimpan sebelum submission. Posisi baru tercatat setelah signature confirmed dan actual token-balance deltas direkonsiliasi. Hasil jaringan ambigu tetap pending; jangan membuat transaksi pengganti.

Quote maksimum 15 detik sebelum sign/submit. Deviasi terhadap Jupiter Price V3 dibatasi 150 bps dan source slot maksimum selisih 64. Top-level route dan CPI program dibatasi allowlist. Provider instruction setup/cleanup/tip ditolak. Compute budget dibangun lokal; ceiling fee 100.000 lamports. Restart, perubahan policy, lock wallet, atau kill switch membatalkan live authorization.

**Saat live authorization kedaluwarsa atau signer dikunci, live exits juga tidak dapat ditandatangani.** Pantau posisi dan perbarui authorization sebelum expiry. Kill switch Live adalah lockdown signing/submission, bukan emergency liquidation. Session stop pada Dry Run menghentikan simulasinya, bukan menutup seluruh posisi.

### Gate sebelum pendanaan/aktivasi produksi

Sesuai PRD §44, fixture smoke test tidak cukup untuk mengaktifkan produksi. Lakukan review source dan threat model, dry-run soak, validated strategy/risk policy, actual-provider unsigned simulation, devnet/local-validator transaction validation, reconciliation/restart recovery, failure drill, dan controlled mainnet pilot dengan explicit owner approval. Aplikasi ini belum memperoleh bukti pengujian tersebut.

Setelah reviewer/operator benar-benar menyelesaikannya, simpan catatan approval bertanggal dan berversi dalam **`live-validation-approved.txt`** di data directory runtime. File ini tidak dibuat oleh setup, UI, atau smoke test. Kehadirannya adalah attestation operator, bukan verifikasi otomatis hasil pengujian. Jangan menambahkan file kosong hanya untuk melewati gate.

Hanya satu execution authority diperbolehkan. Lock file runtime mencegah dua proses menulis database yang sama; wallet lease memakai public key di `KAIROS_LEASE_DIR` (default: direktori temp OS `/kairos-wallet-leases`). Semua runtime untuk wallet sama harus berbagi lease directory yang mendukung advisory file locks. **Multi-host dengan filesystem/lease terpisah belum didukung.**

## Docker

```powershell
docker compose up --build -d
```

Satu port lokal 8080 untuk SPA, API, WebSocket dan health. Image berjalan non-root; named volume menyimpan database/vault/token. Ambil owner token dari `/data/owner-token` secara lokal. Gunakan reverse proxy TLS/private network untuk remote access, dan set `KAIROS_SECURE_COOKIE=true` di lingkungan HTTPS. Jangan expose raw HTTP ke internet. Docker daemon harus aktif.

Konfigurasi server: `KAIROS_BIND`, `KAIROS_DATA_DIR`, `KAIROS_WEB_DIR`, `KAIROS_LEASE_DIR`, `KAIROS_SECURE_COOKIE`. Opsional `KAIROS_OWNER_TOKEN` minimal 32 karakter untuk provisioning; default file token lebih cocok untuk penggunaan lokal.

## API / IPC

REST: `POST /api/v1/auth/login`, `POST /api/v1/auth/logout`, `GET /api/v1/runtime`, `GET /api/v1/runtime/readiness`, `POST /api/v1/commands`, `GET /ws`, `/health/live`, `/health/ready`.

Semua mutasi memakai header `x-kairos-command: 1`, cookie session, dan UUID `request_id`. Command enum menolak field tak dikenal. UUID receipt tersimpan untuk idempotency. WebSocket mengirim versioned snapshots; polling memperbaiki event yang terlewat. API detail dalam PRD dirangkum menjadi command endpoint pada preview ini.

```json
{"request_id":"4a64c695-6f12-4e26-8206-c5f7a88ae194","action":"run_cycle"}
```

Tauri memakai `get_runtime_status`, `runtime_command`, dan event `runtime.snapshot`. Keduanya memanggil application use cases yang sama.

## Verifikasi

```powershell
npm.cmd run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked --features kairos-application/test-support
npm.cmd run smoke
```

Smoke test memakai Google Chrome terinstal pada Windows; pada mesin lain jalankan `npx playwright install chromium`. Test membuat data directory terisolasi `.smoke-data-*`, memakai server HTTP upstream khusus pengujian, dan tidak membaca wallet pengguna. Endpoint Jupiter runtime normal tidak dapat dioverride oleh `KAIROS_TEST_UPSTREAM`: override hanya dikompilasi dengan feature `test-support`. Build deployment tanpa feature tersebut. Lihat [hasil smoke test](SMOKE_TEST.md). Screenshots/trace ada di `test-results` dan `playwright-report`.

## Struktur

```text
src/components/atoms       Button, Badge, TokenIcon
src/components/molecules   Panel, Field, Modal, StatCard
src/components/organisms   AgentField, MarketPanel, PositionsTable, dialogs
src/components/templates  CommandLayout
src/pages                  Overview dan halaman operasional
src/hooks                  TanStack Query / event subscription
src/lib                    Zod contracts, transport, display formatting
crates/domain              Mode, fixed-point amounts, risk authorization
crates/application         Use cases, ledger, SQLite, audit, supervisor
crates/live                Encrypted vault, Jupiter/RPC, signer, validation
apps/server                Authenticated Axum host
src-tauri                  Tauri IPC host
```

Domain tidak mengimpor React, Tauri, Axum, provider API atau database. UI tidak mengubah saldo dan tidak membangun transaction bytes. Dependency lockfiles disertakan bersama kode. Button mengikuti komposisi shadcn/ui dengan semantic CSS; dialog menggunakan Radix untuk focus trap, Escape, dan restoration. Font disajikan lokal tanpa CDN.

Referensi implementasi: [Jupiter V2 build](https://developers.jup.ag/docs/swap/build), [Jupiter Price V3](https://developers.jup.ag/docs/price), [Tauri commands](https://v2.tauri.app/develop/calling-rust/), [Axum WebSocket](https://docs.rs/axum/latest/axum/extract/ws/).
