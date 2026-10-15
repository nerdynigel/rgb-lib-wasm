# Incoming history refusal prerequisite — 15 October 2026

PARTIAL / NOT SAFE TO ACCEPT. This is a prerequisite correction for the generic
exact incoming NIA consumer authorized on THE-944. The consumer is not complete.

At predecessor a7210b82c36f602229135c49065946972b0f044f, incoming confirmation
syncs BDK, clears pending_witness and accepts Stock before checking UnsafeHistory.
It checks only warnings[0]; a preceding Custom warning can bypass refusal.

The confirmation path now validates and examines every warning before any of
those mutations. UnsafeHistory still returns Ok(None) (existing refusal result),
but it cannot reach sync/TXO/Stock effects. Safe history and Custom-only warning
policy are preserved. The predicate does not supply confirmation proof. Existing
network confirmation lookup still precedes validation and may fail read-only.
Other errors after this guard may still leave partial state; this patch does not
make broad refresh atomic or suitable for the authoritative journal consumer.

Inventory: MODIFY src/wallet/online.rs and src/wallet/mod.rs; ADD
src/wallet/incoming_history.rs and this note. No Cargo.toml/Cargo.lock, feature,
asset/schema, protocol, signer or dependency changes. No new public API.

Executed with process-only CARGO_HOME=/home/nigel/.cargo,
RUSTUP_HOME=/home/nigel/.rustup,
RUSTUP_TOOLCHAIN=1.97.1-x86_64-unknown-linux-gnu:

- cargo test --offline --locked --lib incoming_history -- --nocapture: PASS 2/2.
  UnsafeHistory at all four positions in a warning vector refuses; empty and
  Custom-only status retain existing policy. Synthetic Status values, not live
  consignments, prove that predicate boundary.
- cargo test --offline --locked --lib: PASS 149/149 (default features).
- cargo clippy --offline --locked --lib -- -D warnings: PASS.
- rustfmt --edition 2024 --config skip_children=true --check on the three changed
  Rust files: PASS; git diff --check: PASS.

Initial rustfmt invocation lacked process toolchain selection and failed before
formatting. Correct invocation above succeeded. Its recursive formatting of two
unrelated files was restored to the exact clean predecessor; excluded from diff.
No lint suppression added; existing crate lint policy unchanged.

These native checks do not exercise actual network-refresh/UnsafeHistory/full
snapshot preservation, prospective/actual post equality, WASM/IndexedDB, signing,
financial settlement or crash recovery. No artifact build/adoption, financial
operation, mainnet, PR approval or merge. Facade/wallet remain pinned to the prior
engine pending grouped incoming consumer completion; do not adopt this head as a
complete candidate.

Next SAME-owner source: exact receipt selector, bounded immutable chain/history
observation and detached full Stock+BDK+TXO expected post; then opaque source-bound
one-shot application, exact full equality and journal-first post acknowledgement.
Keep existing incoming/default fences and ambiguous recovery refusal. Do not use
broad sync and mask unrelated changes or reconstruct consumed signing authority.

Rollback: revert only these source files, preserving journals/checkpoints/debits
and all consumed or ambiguous cases. Reverting restores the documented defect;
it is not a financial-state rollback.
