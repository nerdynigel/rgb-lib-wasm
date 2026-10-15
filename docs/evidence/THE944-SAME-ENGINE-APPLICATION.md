# THE-944 same-engine application source — 13 Oct 2026

PARTIAL / NOT SAFE TO ACCEPT. Chief comments 06da2728 then dd7e51ef applied.
This source does not complete the grouped authority/controller/pending outcome.
No signing, funds, broadcast, artifact distribution, consumer repin or adoption.

## Product delta

`src/wallet/offline.rs`: `snapshot_for_review()` exports every existing snapshot
field at the current sequence without incrementing it. Persistence still allocates
checked monotonically increasing sequences. The active facade must use the new
view for immutable review, retaining full state comparison. Missing/detached stock
refuses. The engine persistence ownership flag becomes visible only to sibling
wallet modules, not public callers.

`src/wallet/pending_fill.rs`: Rust-only, regtest-only
`apply_consented_pending_fill(association_id, consent_digest, now)` uses the SAME
retained wallet and pending record. It requires consumer-owned persistence,
Consented state, exact association/digest and unexpired intent; validates bounded
unsigned two-input PSBT, distinct seals, exact previous transaction txids and
witness amounts/scripts, independently inspected consignment/witness facts,
ContractId and every allocation. It recomputes exact prospective PSBT, transaction
and consignment before using established RGB consume and outgoing registration.
Registration uses the actual colored change vout, including implicit host insertion.
It returns the exact colored PSBT, never a signature or durable acknowledgement.

Before entering engine export/consume it retains the complete intent as Applying.
Success becomes Applied only after exact byte checks and outgoing registration.
Errors after that point retain Applying and refuse another application. No erase,
replace, restore-based transition or guessed refund is introduced. This is an
embedding primitive, NOT authenticated wallet-owned policy/ownership/receive
approval. The consumer must independently validate those full records and must
commit the complete snapshot before any key operation. Do not expose this seam
through JS or use it to replace the missing authority bridge.

## Executed checks and limits

From this engine worktree, existing tooling only:

```sh
cargo +stable test --offline --locked --lib
cargo +stable clippy --offline --locked --lib -- -D warnings
cargo +stable fmt --all -- --check
git diff --check
```

Final native144/144 PASS (32.10s); strict native Clippy PASS (1.74s); formatter and
diff PASS. Earlier143/143/Clippy PASS before review-view addition. Two initial new
negative tests failed because their snapshot calls intentionally allocate different
sequences (0/2/exit101); corrected fixtures verify exactly one sequence per snapshot
and compare every other field. This was a fixture correction, not an application
failure baseline. An initial compile failed on private field visibility; corrected
to sibling-only visibility. All failures are recorded, none waived.

New tests cover complete stable review views with retained pending intent, detached
stock refusal and continued persistence sequencing; application refusal for wrong
association/digest, exact expiry boundary, Applying/Applied, legacy persistence,
absent intent, detached stock and malformed storage-only requests. Existing older
snapshot/rollback preservation now also covers Applying. No successful real RGB
application, interrupted actual consumption, IDB recovery or wallet signature was
executed. Negative application checks use explicitly synthetic storage fixtures.

Actual unfunded facade regression (retained test-only review fixture; real engine,
AEAD/IDB/current_review) reproduced a primary panic in unchanged current_review:
its persistence snapshots incremented sequence and made every comparison drift.
Baseline browser3/9 PASS, one primary failure/five panic cascades, launcher exit1.
After using review snapshots: browser9/9 PASS, 7.91s, Chromium154.0.8037.0. This
checks repeated read-only review retrieval plus rejection after a real address
mutation; it is not an actual atomic transfer review or approval. Facade details
are in its coordinated evidence document. Old distributed binaries are unchanged.

## Compatibility / source recipe / rollback

The existing engine snapshot schema gains Applying as an explicit enum variant.
Old engine/facade readers reject this variant, preserving bytes; they must never
map it to absent intent. Ordinary active restore still refuses every pending state.
No existing durable records are migrated. Required consumer: prospective facade
`active_wallet_store.rs` review/current_review only, using this exact engine source
through a run-owned scratch manifest. Consumer manifests/locks/released pins stay
unchanged. No production caller invokes the application seam in this packet.

Next coherent source work on the same four branches: wallet-owned full accepted
policy/ContractId/association, ownership/prevout and receive registry; guarded
SAME-engine confirmation; recoverable approval consumption plus pending/application
journal; then actual async controller confirm/apply with post-await revalidation.
The separate RGB and RLN IndexedDB databases cannot supply a single transaction.
Use a full authenticated recovery journal or one authoritative combined record;
two acknowledged writes or a flush-after-sign wrapper do not satisfy the contract.
Validate successful actual engine application and failure recovery before caller
release. Full heads and source inventories must be reconciled by Chief before any
artifact/financial operation. Rollback is source revert; retain all durable cases.

All original bilateral SERVED nonzero/both real approvals/own signatures,
construction binding, SAME-case all-party BTC+RGB accounting, items5/16/18/19,
R1/R4/immediate crash and mandatory live reverse remain OPEN; R3 UNAVAILABLE.
Unwaived facade Clippy failures and THE-872/THE-945 holds remain. Security chain,
pins, locks, artifacts and consumed cases are unchanged. No acceptance or merge.
