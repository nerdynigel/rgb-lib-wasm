# THE-944 designated-input local availability query

PARTIAL candidate source; no financial release or acceptance uplift.
Authority: [Chief scope reconciliation](https://paperclip.ing/THE/issues/THE-944#comment-65616366-9555-4be6-a412-e9ee7dd22c57), saved direction revision `6a224162-fc29-4d0e-9a3c-fb4e2e07dd95`.
Existing branch `feat/the-876-transfer-export`, PR3; predecessor
`073e2d10c23fb5e3b0dd36730a0dc506ae717115` independently matched origin before publication.

## Inventory and behavior

Product: `src/wallet/input_availability.rs` and its private module declaration in
`src/wallet/mod.rs`. Public Rust `Wallet::require_available_rgb_input` returns
`Result<(), Error>`, with no serialized receipt/capability or JS export.

The query synchronously borrows the SAME stock directly (no runtime detach,
replacement stock, schema initialization or lock across an await), reads all DB
tables with errors propagated, and requires exactly one existing, non-spent,
non-pending designated output. All live coloring provenance must resolve to a
settled incoming allocation of the exact ContractId. Pending blinded reservations,
missing provenance, non-fungible/inflation assignments, duplicate/missing inputs,
zero/excess/insufficient quantity and checked-add overflow refuse. The DB total
must equal the fungible stock total on that outpoint. Enumerating ALL locally
known stock contracts also refuses any known assignments on other inputs or any
other asset on the designated input. Input count is bounded to 1024.

No transfer creation, consumption, reservation, database write, automatic save,
sequence allocation, network call, signing or external effect. The only lock is a
scoped immutable RefCell stock borrow. Detached/mutably borrowed stock refuses
without installing replacement stock. Existing pending-fill obligations refuse.

This establishes local RGB ownership/availability, NOT independently authenticated
chain unspentness, key ownership, accepted economic policy or absence of foreign
RGB state unknown to this engine. Consumers must retain SAME engine/session/full
snapshot, authenticate prevouts/signing ownership independently, and recheck after
awaits and at effects. Existing financial fences remain necessary. No dependency,
asset identity, protocol or cryptographic primitive changed. Cargo.lock unchanged.

## Executed checks (toolchain 1.97.1)

Commands from this engine worktree, `CARGO_HOME=/home/nigel/.cargo`, toolchain bin on
PATH, `CARGO_BUILD_JOBS=1`; Clippy additionally selects the existing
`RUSTUP_HOME=/home/nigel/.rustup RUSTUP_TOOLCHAIN=1.97.1` (no installation/config change).

- `cargo check --offline --locked --lib -j 1`: PASS on query source.
- `cargo test --offline --locked --lib --features test-fixtures input_availability::tests -- -q`: PASS **6/6**, 147 filtered, 1.54s.
- `cargo clippy --offline --locked --lib -j 1 -- -D warnings`: PASS.
- Feature variant `cargo clippy --offline --locked --lib --features test-fixtures -j 1 -- -D warnings`: FAIL at unchanged `offline.rs:3673`, existing `Arc<InMemoryDb>` is not Send/Sync. No suppression or unrelated helper change. This check remains open.
- Scoped rustfmt `--check --edition 2024 --config skip_children=true` on both changed files and `git diff --check`: PASS.

Six engine-bound tests issue genuine clearly-fake NIA state through the established
in-memory builder. They exercise exact input + full snapshot preservation, spent/
missing/pending outputs, all four non-settled statuses/broken provenance, second
real asset-bearing input and DB overflow, pending fill/blinded reservation, and
other/detached/borrowed/changed engines. These are native in-memory tests, NOT funded
chain, browser, persistence/crash, signing, fee or settlement acceptance.

Preserved failed iteration evidence: initial compile found two new test constructor
API mismatches; corrected without product relaxation. Initial 0/5 test run refused
the existing convenience helper's detached-stock persistence precondition. New
tests now explicitly initialize TEST stock before consumer handoff. A subsequent
4/5 run exposed an ineffective test update using ActiveValue::Unchanged; the test
now uses the established ActiveValue::Set update and proves all statuses refuse.
The final run passes 6/6. No earlier assertions or fixtures were edited.

Initial default CARGO_HOME lacked the local index; offline resolution failed, and a
normal locked invocation downloaded public crates before being interrupted (130).
The final checks used the existing user cache offline; no private signer repository
or denied SSH operation was accessed/retried. Initial Clippy invocation lacked a
selected rustup toolchain; selecting the existing explicit toolchain fixed only
that invocation. No privileged/environment administration occurred.

## Remaining scope and rollback

Facade integration and immutable engine/facade/wallet candidate source pins belong
to the same Lead. Full independent economic acceptance, controller approval/parent
budget, ONE-root readback/CAS and authority-aware recovery/signing remain OPEN and
fenced. Old facade4f10cd2e compilation does not cover this source. No artifact
adoption, Reliability operation, mainnet, APPROVE or merge. Original THE-945,
accounting/items5/16/18/19, R1/R4/process-kill/live reverse holds remain; R3 unavailable.

Rollback: revert this source commit and corresponding consumer source pins together;
preserve all durable roots, obligations, consumed cases and historical evidence.
