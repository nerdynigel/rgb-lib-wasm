# Exact incoming Rust embedding boundary — 15 October 2026

PARTIAL / NOT SAFE TO ACCEPT. Predecessor engine a9cc83961bcf1b810711c099116c3c79f1eb4418.

Product inventory: incoming_application.rs gains the Rust-only opaque
PreparedIncoming export, complete prospective accessor, complete original/staged
source check and async prepare entrypoint; wallet/mod.rs re-exports the type.
No constructor, Clone, serde or JavaScript binding exists. Prepare refuses outside
regtest or consumer-owned persistence before indexer observation. Existing exact
configured-indexer/history/Stock/BDK projection and one-use apply are reused.
The exclusive mutable Wallet borrow survives all host awaits. Host acknowledgement
is NOT inferred by the engine: the embedding host must persist/read back intent
and actual debit, revalidate session/expiry/revocation, consume once and persist/
read back the COMPLETE post. Error after mutation remains ambiguous. No caller
may recreate this value from a recovered intent, raw snapshot or success flag.

Test inventory: incoming_test_indexer.rs factors the existing GET-only synthetic
server runner; incoming_post.rs retains its original independent projection and
read-only observation assertions and exercises the public Rust entry separately.
The positive case compares COMPLETE prospective/actual checkpoints, checks legacy
persistence refusal and unchanged source/stage after dropping preparation. Five
served fault cases remain. No assertion weakened or comparison masks introduced.

Executed existing host toolchain, process-only PATH=/home/nigel/.cargo/bin:$PATH,
RUSTUP_HOME=/home/nigel/.rustup and CARGO_HOME=/home/nigel/.cargo:
- cargo +1.97.1 check --offline --locked --lib: PASS.
- cargo +1.97.1 test --offline --locked --lib: PASS173/173,30.65s before the final
  test-only legacy/drop assertions; targeted configured_indexer after those
  assertions: PASS6/6,2.19s.
- cargo +1.97.1 clippy --offline --locked --lib -- -D warnings: PASS.
- scoped rustfmt edition2024/skip_children and git diff --check: PASS.

SYNTHETIC chain/indexer only; no durable host/journal/IndexedDB/live financial proof.
Native --tests WASM-only import failure and historical SDK ClippyFAIL40 remain
unresolved. No dependency, lock, feature, schema, asset, signer, protocol or crypto
change. The facade must select this immutable source revision before compiling
its private consuming integration. No distributed artifact adoption, financial
release, actual APPROVE or merge. Chief retains grouped affected SDK/default+TEST,
Clippy, WASM/ABI/artifact/IndexedDB checks and independent THE-945. Original
accounting/R1/R4/immediate-kill/live reverse gaps remain; R3 unavailable.
