# Configured-indexer incoming observation proof — 15 October 2026

PARTIAL / NOT SAFE TO ACCEPT. Predecessor: 165c67b0c16da9d83d934568f3e2274f4d382283.
Phase 6 remains active; no runtime adoption or financial acceptance.

Inventory (TEST ONLY): ADD src/wallet/incoming_test_indexer.rs; MODIFY cfg(test)
fixtures in incoming_bdk.rs and incoming_post.rs; register the native-only test
module in wallet/mod.rs. ADD this evidence. No product-path, dependency, lock,
feature, schema, signer, asset or protocol delta. Existing direct projection and
independent primitive/consumer tests remain present with their assertions intact.
The synthetic foreign prevout now has a reproducible transaction whose hash is the
prevout's actual txid; this changes disposable test data only, no historical case.

The former positive tests constructed FrozenIncoming directly. They did not execute
the async configured-indexer/history observer. Six new tests use a loopback HTTP
server on an ephemeral localhost port, the existing go_online/Esplora client, actual
NIA consignment/history validator and complete Stock/BDK/DB projection. Server
accepts GET only, rejects unexpected paths, bounds header/socket/deadline work,
closes connections, and joins its thread on drop. All data is synthetic, including
chain hashes/inclusion/transactions: this is a simulated indexer, NOT live regtest.
No signing, broadcast, persisted financial journal or IndexedDB is exercised.

Positive: complete source snapshot PLUS staged BDK remains unchanged after async
observation; full prospective checkpoint equals the independently derived projection;
that observed result feeds the private SAME-engine one-use application and its
COMPLETE actual checkpoint equals the projection. No masked fields.
Negative: changed exact witness bytes (same txid), moving tip, changing inclusion,
wrong genesis/network and missing previous transaction all refuse with COMPLETE
source+stage unchanged. Every negative asserts the intended fault was actually
served; an incidental earlier refusal cannot satisfy it.

Executed using existing Rust 1.97.1, process-only PATH=/home/nigel/.cargo/bin:$PATH,
CARGO_HOME=/home/nigel/.cargo, RUSTUP_HOME=/home/nigel/.rustup:
- cargo +1.97.1 test --offline --locked --lib configured_indexer -- --nocapture:
  PASS6/6 (1.21s); initial three cases also PASS3/3 before adding other negatives.
- cargo +1.97.1 test --offline --locked --lib: PASS173/173 (30.34s).
- cargo +1.97.1 clippy --offline --locked --lib -- -D warnings: PASS (2.54s).
- scoped rustfmt --edition2024 --config skip_children=true --check and git diff --check: PASS.
- cargo +1.97.1 clippy --offline --locked --lib --tests -- -D warnings: FAIL101,
  same four existing WASM-only API import/call errors in tests/existing_snapshot_wasm.rs
  (lines101/191/196/251). No test removed, suppressed or weakened. This is NOT a
  strict all-tests lint pass. Historical facade SDK ClippyFAIL40 remains unresolved.

Next source remains the SAME owner's durable incoming intent/debit and post-journal
composition. Inspection confirms existing facade reconciliation intents bind only
source/witness hashes and represent outgoing_post; incoming require_post refuses.
Do not store incoming bytes in outgoing_post or mask Stock/BDK fields. A full incoming
prospective/actual commitment must be validated under the accepted authority before
an authenticated journal read-back can acknowledge it. Retain exclusive store+engine
custody, live session/expiry/revocation checks, exact original source/root/debit and
error/cancellation quarantine. No restored intent may recreate consuming authority.
The private consumer remains unexported; this test closes the simulated async path
proof gap only, not host persistence or live-indexer correctness.

Runtime remains wallet8c08a0ba / facadec7c04c07 / enginea7210b82 / protocol1.9.0.
Chief retains grouped SDK/default+TEST/WASM/ABI/artifact/native-IDB/browser checks and
independent THE-945. THE-872 source hold/consumed plan55 and THE-945 both prerequisites
remain. Original construction/live/SAME-case seller,buyer,fee BTC+RGB accounting,
items5/16/18/19/R1/R4/immediate-kill/required live reverse OPEN; R3 unavailable.
No heavy/build/browser/financial replay, pin change, artifact adoption, source
completion, financial release, APPROVE or merge. Rollback only these test/evidence
files; preserve financial journals, obligations and consumed cases.
