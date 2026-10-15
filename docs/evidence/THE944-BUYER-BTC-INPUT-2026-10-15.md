# Buyer BTC input availability — source evidence

PARTIAL / NOT SAFE TO ACCEPT. SAME Lead, Chief continuation6cf3de8d and
read-only engine-query authority6a224162. Existing branch
`feat/the-876-transfer-export`, draft PR3; predecessor
`eaa353fff655c05d5f258eb35b3de3121a5d63f6` matched origin before edits.

Product and tests: `src/wallet/input_availability.rs` only. Adds Rust-only
`Wallet::require_available_btc_input(designated, expected_TxOut, complete_inputs)`.
It checks the SAME BDK graph for the exact owned unspent value/script, refuses
empty/duplicate/oversized input lists, zero value, retained pending obligations,
detached/borrowed stock, database amount/state conflicts, pending receiving scripts,
colorings/blinded reservations and every known stock assignment on any proposed
input. No sync, transfer creation, reserve, consume, snapshot sequence allocation,
DB write, address derivation, network, key operation or JS capability export.
Foreign RGB state unknown to this engine and current chain unspentness remain
outside this local read-only query; callers revalidate SAME session/snapshot.

Executed with existing1.97.1 toolchain bin on PATH, CARGO_HOME=/home/nigel/.cargo,
RUSTUP_HOME=/home/nigel/.rustup, RUSTUP_TOOLCHAIN=1.97.1:

- `cargo test --offline --locked --lib --features test-fixtures input_availability::tests -- -q`: final PASS10/10 (147 filtered). Includes six existing RGB tests and four new genuine BDK/DB/stock tests: exact owned input and full snapshot preservation; actual graph spend + retained obligation; other engine/unknown prevout/script/value/list errors; known issued RGB on another input; detached/borrowed stock; DB spent/missing/pending/amount conflict and pending receive script.
- `cargo clippy --offline --locked --lib --features test-fixtures -j 1 -- -D warnings`: final PASS.
- `cargo check --offline --locked --lib -j 1`: PASS on initial query; final strict feature Clippy also compiles default features including the subsequent pending-script/DB amount checks.
- Scoped rustfmt and `git diff --check`: PASS. Cargo manifests/lock unchanged.

First new test run: six existing PASS, three new FAIL because TEST funding used a
coinbase-shaped TxIn with a mempool last_seen value (BDK panic). Corrected the TEST
funding prevout to a non-null input; product query/assertions unchanged. Next9/9
PASS; added conflict test and final10/10 PASS. No financial action occurred.

These tests prove native local predicates only, not full facade/PSBT/decoder,
controller/root, browser/IDB, served consent, chain/financial/crash acceptance.
Consumers and source pins must advance coherently. All financial fences and
original acceptance hold. Roll back only this query/source pin group; preserve
all roots, pending obligations, consumed cases and failed test receipts.
