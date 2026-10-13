# THE-944 active-engine consent installation — 13 Oct 2026

PARTIAL source integration, no candidate/financial acceptance. Existing branch
`feat/the-876-transfer-export`, [draft PR3](https://github.com/nerdynigel/rgb-lib-wasm/pull/3).
Parent `0e95e408ec5288a914ae0f94fd398e27edddafae` persistence ownership handoff is
preserved, including Chief-selected f885c3fe ancestry and d74e014/6adf6b2 restore
corrections. No consumer pin or artifact adoption.

## Boundary and trust

Previously a consumer could preserve a pending record through snapshot restoration,
but there was no direct consent installation operation on the active engine. This
is an integration gap, not a reproduced financial-loss claim. No baseline failure
is claimed for a previously absent API.

`Wallet::install_consented_pending_fill` is a Rust-only trusted-embedding seam on
the SAME wallet. It requires irreversible consumer-owned persistence and refuses
in-flight legacy saving or borrowed/detached RGB stock. It accepts only Consented
records, refuses replacement or state substitution, and treats an exact repeat as
idempotent. It neither replaces BTC/RGB state nor increments the persistence
sequence, schedules a legacy write, consumes RGB, signs, broadcasts or acknowledges
a durable commit. A restored Applied record cannot be downgraded by installing the
same economic terms as Consented. Read-only `pending_fill()`/`digest()` support exact
consumer comparison without exposing mutable wallet state.

The embedding consumer MUST validate current policy/role/approval, ownership,
authenticated prevouts and exact inspected economics before calling; MUST commit
the complete snapshot with the existing authenticated codec and exact ciphertext
CAS before signing release. Failed/ambiguous commits must poison its handle while
retaining intent. A typed record or digest authenticates neither consent nor spend.
This adds no wasm-bindgen surface, application API, cryptography, dependency or
state-erasure/Applied transition mechanism. No isolated watch-only writer is attached.

## Inventory and executed validation

Product: `src/wallet/offline.rs` (installation/read operation and four regression
cases); `src/wallet/pending_fill.rs` (read-only digest and Consented predicate).
Evidence: this file. Engine schema/encoding unchanged.

Actual commands on installed tools (all exit0):
- `cargo +1.97.1 test --lib --locked --offline install_consented_pending -- --nocapture`: PASS4/4.
- `cargo +1.97.1 test --lib --locked --offline --quiet`: PASS139/139.
- `cargo +1.97.1 check --target wasm32-unknown-unknown --locked --offline --quiet`: PASS.
- `cargo +stable clippy --lib --locked --offline -- -D warnings`: PASS (installed1.98.1).
- `cargo +1.97.1 fmt --all -- --check`; `git diff --check`: PASS.

Initial targeted3/3 passed before adding restored-Applied coverage. Final4/4 is the
applicable targeted result. No test failure/waiver, installation or test weakening.
The cases exercise whole snapshot financial equality, pending strict restore,
no installation sequence/auto-save side effect, legacy/busy/runtime refusals,
replacement refusal and restored Applied preservation. Synthetic native state
only, not authenticated IDB, real approval, served browser, funding or recovery proof.

## Exact remaining-proof map / reuse

| Requirement | Current evidence / remaining owner |
|---|---|
| Same-engine pending storage/preservation | Source predicates/serializer, native139/139; Software still must wire current facade confirmation |
| Active-default authenticated restore/flush/CAS | OPEN; Software must retain exact restored ciphertext, use established codec and serialize lifecycle; separate accounting writer is not equivalent |
| Guarded confirm/apply, both approvals, own signatures | OPEN; this operation does not validate approval or consume |
| Bilateral served coordinator/bridge non-zero path | OPEN; historical driver proof is unchanged at driver level |
| Seller per-UTXO spent/change and all-party accounting | OPEN; actual same-case accounting still required |
| R1 abrupt OS-kill, R4 financial persistence/refresh, immediate crash, live reverse | OPEN; no financial invocation in this change |
| R3 | UNAVAILABLE, unchanged |
| Final frozen-candidate independent acceptance | THE-945 held; THE-872 operator hold retained |

Prior native/driver/security/browser evidence keeps its own revision and level;
full139/139 covers this engine source and preserved ownership/restore guards only.
Consumer b377de6, saved facade f4332a1c, wallet8028b45, protocol1.9.0 and the accepted
Lightning reconnect/security chain remain unchanged. No peer update or breaking
protocol change. No rebuild/repin, serving/funding/mining, financial envelope,
merge or release. Software continues SAME issue under comment-based Chief review;
removed native policy stays removed, no automatic per-slice stop.

Rollback before adoption: revert these source changes. No durable migration/state
change occurred. Never erase pending intent or enable legacy persistence to recover
a failed CAS. Group facade integration and exact source/artifact/operation manifests
before any later authorized rebuild or affected financial proof.
