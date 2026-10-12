# Strict existing-snapshot engine candidate — 12 October 2026 UTC

Board comment d70808c2 on THE-876 supersedes the implementation hold. This
increment delivers the engine prerequisite only; original acceptance stays OPEN.
Registered workspace f85beea3-1dd0-403d-aa2d-9f495e2cfef0, sole Software writer.
Starting clean HEAD 5d9e81189ab7ca68c2d4359761264a50e2a6587f preserves the
published byte-state and local-script increments descended from the wallet's
current pinned engine 1028ba6353a27d4f35ade9b10768dd75528478bc. No reset or
rewrite of either increment. Delivery uses existing branch
feat/the-876-transfer-export / draft PR3; final full SHA is in the issue receipt.

## Implemented boundary

`idb_store::read_existing_snapshot_bytes` opens ONE version1 connection, aborts
EVERY upgradeneeded transaction (including missing-database creation), refuses
blocked/open errors and missing stores, reads only in a readonly transaction,
waits for completion, and closes on success/error. A request succeeding after
blocked rejection closes without opening a transaction. No Rexie builder or
fresh-state fallback. `load_existing_snapshot` parses the existing JSON format;
read/decode failures propagate without raw stored values in errors.

`Wallet::load_existing_snapshot` derives the existing key without constructing
a wallet first. `restore_existing_snapshot` requires all three strict Stock
components and a loadable persisted BDK changeset; deserialization, Stock decode,
descriptor/genesis identity and BDK restore failures propagate. It loads BDK
from that changeset and Stock from those bytes, preserves sequence, database,
pending artifacts, received consignments and reuse indexes. It does not call
Wallet::new, import schemas, reconcile receipts, snapshot, flush or save. The
new path uses a discard logger and installs no panic hook. Ordinary restore/new,
production unlock and persistence behavior are unchanged.

The engine's existing storage is JSON, not an encrypted envelope. The raw-byte
read plus in-memory restore seam deliberately permits a consumer to authenticate
and decrypt using its existing codec before parsing. THIS increment does not
prove decryption or secure-facade integration and does not silently accept an
encrypted string as JSON. No cryptography or default encryption policy changed.

## Executed validation

User-installed cargo/rust toolchain; no packages/services/CI changed.

- /home/nigel/.cargo/bin/cargo check --locked --offline --target wasm32-unknown-unknown --workspace: PASS.
- /home/nigel/.cargo/bin/cargo test --locked --offline --target wasm32-unknown-unknown --test existing_snapshot_wasm --no-run: PASS.
- GECKODRIVER=/usr/local/bin/geckodriver MOZ_HEADLESS=1 WASM_BINDGEN_TEST_ONLY_WEB=1 NO_PROXY=localhost,127.0.0.1 no_proxy=localhost,127.0.0.1 /home/nigel/.cache/.wasm-pack/wasm-bindgen-8a74166b323833fc/wasm-bindgen-test-runner target/wasm32-unknown-unknown/debug/deps/existing_snapshot_wasm-024f3a2beba96ecc.wasm strict_existing_snapshot_real_idb --exact --nocapture: PASS1 real Firefox test /16 distinct cases.
- cargo fmt --all -- --check; git diff --check: PASS.
- Strict Clippy -D warnings: FAIL on unchanged online.rs:1304/1342,
  clippy::field_reassign_with_default. With ONLY that baseline lint allowed,
  locked/offline wasm workspace Clippy PASS. No blanket warning suppression.

Real browser cases: absent database (subsequent schema probe still absent),
version2 refusal (version/store unchanged), missing store (schema unchanged),
missing key, nonstring, malformed JSON, incomplete JSON, injected get exception,
actual readonly transaction abort, each missing Stock component, missing BDK,
invalid Stock encoding, foreign BDK descriptor identity, and valid in-memory
restore plus asset/transfer reads/drop. Every reached load is monitored to reject
non-readonly transactions; raw stored values compared byte-for-byte on invalid
record cases and across valid restore/reads/drop. Valid receipt:7296 raw bytes,
SHA256 bd1c3d482cccc9a4b3832a81cc9011d6cab7ba200e2c9329b7b64cb3c325a9fa.
Hash is a fresh-run receipt, never a deterministic expected fixture.

Two initial browser runs FAILED in TEST SETUP/CLEANUP: explicit baseline flush
encountered the intentionally corrupt prior record; fixed by resetting only
this disposable test database before setup. Final deletion was blocked by the
ordinary legacy setup flush's retained Rexie connection. The runner disposes
its entire fresh browser profile instead; default persistence is not modified.
The strict loader itself closes its connection. Two initial test compilations
used wrong list API enum/arity; corrected to current API before runtime proof.
Do not count those failed commands as passing. Existing init deprecation warning.

Test creates only fresh public watch-only WalletData and explicit empty-wallet
snapshot setup. No seed/private key/storage content/key is printed, no external
HTTP/Core/indexer/funding/signing/broadcast/financial fixture is used. Source
reads operate on a disposable runner profile, never a preserved acceptance wallet.

## Remaining / rollback

Engine storage/restore prerequisite proved. Software still owns encrypted
secure-facade reader, isolated wallet pin/build/browser adoption, actual pending
mutation, persistence-failure, crash/recovery and full served proof. No mock or
unfunded no-mutation result closes financial accounting. No recipe renewed,
consumed926dd93d/84467299 replayed or downstream hold lifted. Chief reconciles
this delivered prerequisite and maintains serialized same-task continuation.
Rollback reverts only these source/test additions; do not delete wallet state.

Compatibility:
- btcx-wallet-protocol:1.9.0 @2a849f20a1d891748454f13e3d45b0d2a84c9c1b unchanged
- btcx:source418247aab0516c2fd20d8d984d1ee629f3d089d2; wallet pin dd4c12b846da9a1e1956766a35296893702102a4 unchanged
- btcx-wallet:c368ee72c557dcca6fa8da0e8174ae41d3d1a973 unchanged
- RLN:wallet pin c8bad5125d85f90c2d879d0615321d5ad3f8ab2e unchanged
- signer:074de663746b654bbd1cb9756e34c531101edec8 unchanged
- rust-lightning:0939f7751fb876e02caec9cd0a778794b81eee9e unchanged
- breaking protocol change:NO
- peer repo update required:YES isolated facade/wallet candidate adoption remaining
