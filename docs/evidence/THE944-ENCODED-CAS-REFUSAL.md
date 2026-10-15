# THE-944 encoded CAS refusal — 13 Oct 2026

PARTIAL source, not financial acceptance. Chief comments 78d8121b and
0502e072 applied together. Existing source ancestry through selected f885c3fe
and consumer restore9bba8cb retained. Pins/artifacts unchanged.

## Reproduction and source boundary

Production inline-JS commitEncodedSnapshot opened a missing database for an
existing-record CAS and created snapshots before rejecting its absent record.
Baseline Node transaction doubles:6/7 PASS, missing-database assertion FAIL
(created1 instead of0, exit1). This is a source-contract defect, not an observed
browser/financial loss. Source now aborts schema creation when expected ciphertext
is present; only explicit insert-only fresh setup may create a schema. Existing
byte-exact comparison and put remain in one readwrite transaction, with success
only at oncomplete. Authentication belongs to the facade; this API does not
validate ciphertext or grant signing authority.

Product inventory: src/wallet/idb_store.rs (CAS upgrade refusal).
Regression: tests/encoded_snapshot_contract.mjs extracts and executes the EXACT
production inline function against controlled transaction events; no duplicated
CAS algorithm, dependency addition, wallet or financial input.

Seven checks: missing database refuses before creation; exact matching record
waits for completion; changed ciphertext refuses before put; insert-only refuses
an existing record; put exception refuses; abort after put refuses; explicit fresh
insert creates schema and waits for completion. Node24.21.0 --test PASS7/7.
Doubles do not establish real IndexedDB request ordering, browser concurrency,
crash persistence or active-default authentication.

Validation commands: node --test tests/encoded_snapshot_contract.mjs;
cargo +1.97.1 test --lib --locked --offline --quiet;
cargo +1.97.1 check --target wasm32-unknown-unknown --locked --offline --quiet;
cargo +stable clippy --lib --locked --offline -- -D warnings;
cargo +1.97.1 fmt --all -- --check; git diff --check.
WASM check, strict clippy (installed1.98.1), fmt/diff PASS; native result recorded
in the containing BTCX evidence after completion. No distributed WASM rebuild.

## Reuse, remaining proof and rollback

Historical source, security and driver evidence remains valid at its original
revision/level. This source affects only encoded CAS schema refusal. Existing
wallet8028b45, consumer engineb377de6 and facadef4332a1c remain unadopted/unchanged.
Actual saved facade source4fb1e8f still owns an isolated accounting writer;
its default create uses load_snapshot/restore_from_snapshot and default flush
uses wallet.flush. Neither satisfies the new consumer-owned restore/commit seam.
No separate writer may be attached as active-wallet authority.

OPEN: authenticate/decrypt and retain the exact original ciphertext; construct
ONE active engine via restore_consumer_owned_snapshot; serialize every user;
flush complete engine state through encoded CAS and poison failed/ambiguous
writers; guarded current confirm/apply; both approvals/own signatures; bilateral
SERVED non-zero execution; seller per-UTXO/all-party accounting; R1/R4/immediate
crash and required live reverse. R3 UNAVAILABLE. THE-945 and THE-872 holds retained.
This source correction alone closes NONE of those final acceptance requirements.
Software owns remaining source/proof; Chief owns continuation and operation recipe.
No signing release, repin, funding/mining/serving, financial envelope, merge or
candidate promotion. Revert this source before adoption if required; never replace
missing durable authority by fresh state or erase pending intent after a CAS failure.
