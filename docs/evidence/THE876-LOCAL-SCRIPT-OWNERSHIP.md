# Read-only local script membership — 12 Oct 2026 UTC

Authority: Chief comment b8de132b and chief-reconciliation source contract,
12 Oct 2026 11:48 AEST; registered engine workspace f85beea3, branch
feat/the-876-transfer-export, starting baa841bb4794de6d8f7a3a120916b55eabd00918.
Chief accepted RLN025954d4 audit PARTIAL: remote/PR48, four-file diff/Python AST
and 4/4 expected compiler outcomes with rustc1.98.1; cached artifact only.
That accepted missing API boundary is superseded solely for this delegate.

`Wallet::is_local_script(&Script)` calls existing BDK2.0.0 `is_mine` on the live
wallet. It returns false for absence; caller must refuse. It has no JS export,
caller authentication, chain/amount attestation, address reveal, sync, persistence,
receive, RGB transition, signing or broadcast. Membership is BDK indexed-script
membership, not proof of confirmed ownership or authority to spend.

Executed validation: cargo +stable test --locked --lib
local_script_membership_is_read_only -- --nocapture: PASS1. Tests real in-memory
engine wallet and existing BDK, both external/internal index0, independently
constructed foreign wallet, unknown OP_TRUE and empty scripts, repeated queries.
Exact serialized BDK store/staged changes/database and strict stock bytes,
reuse map and sequence remain unchanged. No snapshot construction or flush.
Existing color_export_unfunded_refusals_preserve_stock_and_records: PASS1.
Locked workspace wasm32 check PASS. Locked wasm32 unit-test compilation and
fmt/diff status are recorded in the published task receipt. No browser run.
Initial compilation failed E0308 (&Script vs ScriptBuf) and E0599 (test export
method); corrected with borrowed-script conversion and real database serde.
No dependency or lock edits, no version changes. Earlier engine test increments
at baa841bb are preserved, not replayed as fresh evidence.

Acceptance map: engine source delegate + in-memory membership/nonmutation proven;
WASM runtime membership, facade lifecycle/secure guards, complete output coverage,
caller and isolated consumer adoption remain OPEN until separately delivered.
Financial driver and original bilateral/all-recipient/accounting/interruption/
flush/immediate-crash/native/loader/independent acceptance remain OPEN.
No new browser/profile/financial operation. THE-870/872/873/874 holds retained;
NOT SAFE TO ACCEPT/merge/release. Production/default artifacts and pins unchanged.

Next sequence: engine push/remote verify FIRST, supported registered facade routing,
exact isolated candidate engine pin + guarded default-OFF facade/caller/tests,
locked default/test build, publication, then wallet test chain if needed.
Chief reconciles immutable source before returning the same tranche to Reliability.
Rollback: revert additive delegate/test and any subsequent isolated consumer pin
as a matching set; preserve all profiles/journals and never replay operations.

Lock SHA256: 6287c1b3bebd571efc5a78f325f52ca0274c87d77ed35d10f40d72408125014c

Final fmt --all -- --check and git diff --check: PASS. Locked wasm32 unit-test
compilation --no-run: PASS (not executed); artifact SHA256 08c2877ea7f94be08971ac797cba232ae77266dd756e0837d7e13c6c83dc5cd1
