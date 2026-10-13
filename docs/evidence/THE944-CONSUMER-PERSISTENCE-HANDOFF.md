# THE-944 same-wallet persistence handoff — 13 Oct 2026

PARTIAL source integration; candidate acceptance remains OPEN. Descends from
Chief-selected f885c3fe through d74e014 and 6adf6b2. Existing branch
`feat/the-876-transfer-export`, draft [PR3](https://github.com/nerdynigel/rgb-lib-wasm/pull/3).
No consumer adoption, pin/artifact change or financial authority.

## Necessary boundary

The saved facade's secure default wallet delegates `secureWalletFlush` to
`Wallet::flush`; the authenticated `accounting_snapshot` writer owns a separate
watch-only engine. Default-engine automatic `save_to_idb` calls would remain a
competing legacy writer if a consumer merely added encoded CAS to explicit flush.
Neither a sequence counter nor this handoff authenticates state.

`Wallet::use_consumer_owned_persistence` adds a one-way handoff on the SAME engine.
It refuses while an existing automatic save is in flight or the RGB runtime owns/
borrows stock. After handoff automatic saves return before serialization, sequence
increment or scheduling; legacy flush refuses before snapshot or IndexedDB access.
Explicit `snapshot_for_persistence` still serializes the same complete state.
In-place restore preserves ownership. There is no API to switch back.

The mode is local to the instance, not a persisted authority flag. Newly created
or strict-restored wallets start in legacy mode for compatibility; the consumer
must select ownership before sharing each restored engine or dispatching work.
It must authenticate state with the established codec, retain the exact restored
ciphertext for CAS, serialize the entire financial lifecycle and poison failed or
ambiguous writers. THIS CHANGE DOES NOT IMPLEMENT THAT CONSUMER or confer consent,
signing authority, a recovery classification or financial crash proof.

## Inventory, validation and compatibility

Product: `src/wallet/offline.rs` (ownership mode, guards, three boundary tests).
Evidence: this file. No dependencies, schema, cryptography or generated artifacts.
Legacy mode behavior is retained; active pending rollback/preservation guards remain.

Executed commands with installed tools, no installation:
- `cargo +1.97.1 test --lib --locked --offline consumer_ -- --nocapture`: PASS3/3.
- `cargo +1.97.1 test --lib --locked --offline --quiet`: PASS135/135.
- `cargo +1.97.1 check --target wasm32-unknown-unknown --locked --offline --quiet`: PASS.
- `cargo +stable clippy --lib --locked --offline -- -D warnings`: PASS, installed1.98.1.
- `cargo +1.97.1 fmt --all -- --check`; `git diff --check`: PASS.

The same-wallet test polls public async flush to refusal without browser I/O,
checks automatic-save suppression/unchanged sequence, compares complete financial
state, and checks ownership survives restore. Other tests exercise in-flight-save
and detached/borrowed-stock refusal without cancelling the existing operation.
Initial two-case run failed1/2 because the test helper has no stock; initializing
real empty stock fixed the fixture, without weakening the production predicate.
Intermediate full suite134/134 passed before the additional runtime guard; final
135/135 is the applicable result. Native synthetic evidence; no actual IDB/CAS,
WASM browser execution, funded financial operation or independent acceptance.

Historical driver/native/Lightning/security proof retains its exact recorded tuple
and evidence level. The existing consumer engine b377de6, facade f4332a1c and
wallet8028b45 remain unchanged. No peer update or protocol1.9.0 change. This mode
requires grouped consumer integration and an explicit candidate manifest before a
necessary rebuild; it must not be silently attached to the old shipped facade.

## Remaining and rollback

OPEN: SAME-active-default authenticated restore/flush/CAS and guarded confirm/apply;
serialized bilateral SERVED execution with both approvals/own-input signatures;
seller per-UTXO spent/change attribution; actual R1/R4 and mandatory live reverse.
R3 UNAVAILABLE. THE-945 independent acceptance and THE-872 operator holds remain.
Finite financial envelope unused; no serving/funding/mining, rebuild/repin, merge
or release. Software retains SAME-issue ownership; Chief reviews by comments.

Rollback before adoption: revert this source delta. Once an engine has handed off,
do not re-enable legacy writes to recover a failed CAS; authenticate/reconcile its
durable state through the consumer and restore a new engine under the same policy.
No migration or durable-state change was performed by these native tests.
