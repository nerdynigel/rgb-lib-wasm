# Detached incoming Stock/DB checkpoint — 15 October 2026

PARTIAL / NOT SAFE TO ACCEPT. Same authorized source owner, predecessor
c515410ab8f6efc41017dd20240ffedff449b2d1. Source only; runtime still uses
a7210b82c36f602229135c49065946972b0f044f. No consuming capability, active
application, persistence, signing, external financial action or artifact adoption.

Inventory: ADD src/wallet/incoming_post.rs and this note; MODIFY
src/wallet/incoming_observation.rs, src/wallet/incoming_selection.rs,
src/wallet/incoming_bdk.rs (test fixture visibility only), src/wallet/mod.rs.
No dependency, Cargo/lock, schema, protocol, signer, feature or asset delta.

Frozen observation now composes a complete prospective WalletSnapshot using
its validated transfer, frozen resolver and exact BDK changeset. Stock is
strict-decoded into a detached copy and accepted through established
Stock::accept_transfer. Exact received ContractId/OS_ASSET/checked units required;
other known contracts at the receipt refuse. Every transaction input is checked
for local DB colorings, pending blind reservations and Stock allocations across
all known contracts. This incoming consumer never silently consumes those
obligations as BTC funding.

Detached DB projection requires all existing External BDK outputs to correspond
exactly to DB TXOs (amount, spent, exists, pending) and refuses duplicate or
unrepresented TXOs. It updates exact spent External funding bits and the one
existing receipt placeholder's amount/exists/pending flags, removes its pending
script, and marks only its selected batch Settled in the prospective copy.
Extra new External outputs refuse: this narrow single-receipt consumer cannot
invent their provenance. Internal BTC change stays represented in the complete
BDK graph. No row insertion/counter allocation occurs. Original timestamps are
retained deterministically; no ambient time or backup is introduced. The exact
received consignment is removed only from the prospective snapshot. Sequence,
intent, artifacts, metadata, unrelated rows and reuse indices are retained.
Complete active snapshot PLUS BDK stage is compared before and after derivation.
Prospective Settled is not a durable or economic settlement acknowledgement.

Executed with process-only PATH=/home/nigel/.cargo/bin:$PATH,
CARGO_HOME=/home/nigel/.cargo, RUSTUP_HOME=/home/nigel/.rustup,
RUSTUP_TOOLCHAIN=1.97.1-x86_64-unknown-linux-gnu:

- cargo check --offline --locked --lib: PASS.
- cargo test --offline --locked --lib: PASS164/164 (32.80s), before adding
  the fourth additional refusal test; product source unchanged by that test.
- cargo test --offline --locked --lib incoming_post: final PASS4/4 (0.35s),
  161 filtered. Full current suite contains165 tests; full165 NOT rerun.
- cargo clippy --offline --locked --lib -- -D warnings: PASS after final source.
- Scoped rustfmt --edition2024 --config skip_children=true --check and
  git diff --check: PASS.

Initial new-test compile failed E0603 on shared fixture visibility; corrected
only cfg(test) visibility and reran successfully. No assertion weakening.
Tests use real Wallet/BDK/InMemoryDb with SYNTHETIC transaction graphs and empty
Stock. Positive DB test compares ALL serialized DB fields/counters and complete
active snapshot+stage; refusal tests cover inconsistent/missing/duplicate TXOs,
colored funding and an extra External output. These do NOT exercise successful
RGB Stock acceptance, valid asynchronous observation, full prospective/actual
WalletSnapshot equality, consuming application or financial acceptance.

NEXT: construct a valid real-engine NIA receipt/consignment and frozen resolver
using existing established test primitives (no runtime seeding export), exercise
complete Stock+BDK+DB prospective state and active-state preservation, then prove
actual transition equality. Only after that add opaque SAME-engine/source-bound
preparation and intent/debit-first one-shot application; exact full post journal
CAS/read-back, served acknowledgement and ambiguous journal-first recovery remain
open. Keep incoming/default execution fences and do not re-pin runtime until
this grouped consumer is complete. Do not replay consumed/ambiguous cases.

Native --tests WASM-only imports and SDK ClippyFAIL40 remain unresolved, not rerun.
Chief retains grouped SDK/default+TEST/strict SDK Clippy/WASM/ABI/artifact/native-IDB/
browser checks and independent THE-945 acceptance. THE-872 source hold and
THE-945 source+readiness holds remain. Construction/live binding, SAME-case
seller/buyer/fee BTC+RGB accounting, items5/16/18/19/R1/R4/immediate-kill/required
live reverse remain OPEN; R3 unavailable. No APPROVE, merge or release.
Rollback source only; preserve original financial journals/debits/obligations.
