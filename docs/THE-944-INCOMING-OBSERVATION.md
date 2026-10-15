# Frozen incoming chain/history validation — 15 October 2026

PARTIAL / NOT SAFE TO ACCEPT. Continuation of the authorized generic incoming
NIA consumer on THE-944; source predecessor 8bf795f476f0c969aaa38428c8a19da8e87a3f1e.
No execution caller, artifact adoption or financial operation is enabled.

Inventory: ADD src/wallet/incoming_observation.rs and this note; MODIFY
src/wallet/mod.rs. Cargo manifest/lock, dependency, feature, schema, signer,
asset identity and protocol delta NONE. Runtime facade remains on a7210b82.

Wallet::require_confirmed_pending_witness_receive is an additive read-only Rust
API. It composes the exact local receipt selector with bounded indexer GETs and
established RGB validation. Internal FrozenIncoming retains complete local source,
validated transfer and PreFetchResolver for the next detached derivation. It is
not a spending/application capability and is not exposed to WASM or the facade.

The validator requires the independently accepted complete Bitcoin transaction,
exact txid/vout/nonzero local output, canonical stored NIA consignment and checked
allocation units. A maximum of 128 unique bundled historical witness IDs may be
resolved; missing or duplicate IDs refuse. Consignments retain the selector's
8,000,000-byte bound; the accepted witness has a 4,000,000-byte bound. Network
reads use the existing configured Esplora client. Genesis must match the wallet
network; custom signet refuses because challenge authority is not configured.
Each RGB witness must resolve to its txid, confirmed positive height/time and
canonical block hash, with checked confirmation arithmetic. The accepted witness
must match all transaction bytes including Bitcoin witness data. No missing
witness becomes Tentative and there is no bundled/offchain fallback. Validation
uses the existing NIA trusted type system and refuses every UnsafeHistory warning.
All statuses, tip height/hash and genesis are reread; movement refuses, without
an explicit retry loop. Existing client transport/retry policy is unchanged.

This trusts the configured indexer for chain observations, as existing wallet
online operations do; it does not add independent consensus/SPV verification.
No claim of a chain-wide atomic read or future chain stability is made. A future
consumer must bind/revalidate this observation and the accepted authority before
any external effect. The validator performs no POST, broad sync, graph/Stock/DB
mutation, persistence, sequence allocation or signing.

Complete local source includes EVERY snapshot field plus live BDK staged changes.
Final equality and online-handle validation refuse a changed source. The new real
unfunded Wallet/Stock/BDK test demonstrates why the staged changes matter: address
revelation changes the live BDK stage while the persisted snapshot stays equal.
Repeated source reads preserve sequence and state. No field masks are used.

Executed with existing host Rust1.97.1, process-only PATH=/home/nigel/.cargo/bin:$PATH,
CARGO_HOME=/home/nigel/.cargo, RUSTUP_HOME=/home/nigel/.rustup and
RUSTUP_TOOLCHAIN=1.97.1-x86_64-unknown-linux-gnu:

- cargo test --offline --locked --lib incoming_observation: PASS3/3.
  Two boundary tests use synthetic TxStatus / an empty real PreFetchResolver;
  one uses an actual unfunded Wallet/Stock/BDK. They do not execute the async
  observation or successful RGB validation against a network.
- cargo test --offline --locked --lib: PASS157/157 (29.99s).
- cargo clippy --offline --locked --lib -- -D warnings: PASS.
- Changed-file rustfmt --edition 2024 --config skip_children=true --check and
  git diff --check: PASS.
- Initial targeted compile failed E0283 at ambiguous network Into conversion;
  explicit BdkNetwork::from fixed it before the passing runs. No assertion or
  lint weakened. Prior native --tests Clippy failure on WASM-only snapshot
  integration APIs remains; not rerun or claimed resolved.

Remaining: genuine valid-entry and async chain/history positive/negative proof;
complete detached Stock+BDK+TXO derivation with every local input/output accounted;
SAME-engine source-bound one-shot intent/debit-first application, exact actual/post
comparison, journal CAS/read-back and served acknowledgement/recovery. Incoming
and default execution remain fenced. New code is source-only, not a coherent
integrated candidate. Keep facade/wallet pins until the grouped consumer is ready.

Chief retains grouped SDK default+TEST/strict SDK Clippy (historical FAIL40), WASM
ABI/artifact identity/native-IDB/browser execution. Original construction/live,
SAME-case seller/buyer/fee BTC+RGB accounting, items5/16/18/19, R1/R4, immediate-kill,
required live reverse and independent THE-945 remain open; R3 unavailable. THE-872
source hold and THE-945 source+readiness holds persist. No consumed-case replay,
release, GitHub APPROVE or merge. Rollback source only; preserve all financial
journals, checkpoints, consumed debit and pending obligations.
