# THE-944 prepared SAME-engine application — 13 Oct 2026

PARTIAL / NOT SAFE TO ACCEPT. Chief comments 4258134a then 0a5095bb applied.
Same four source branches, owner and workspace. Full authority/controller/journal
integration and original financial proof remain open. No financial or key operation.

## Product correction and exact inventory

`src/wallet/pending_fill.rs`: replaces the prospective Rust-only
`apply_consented_pending_fill` with `prepare_consented_pending_application` and
`apply_prepared_pending_fill`. Preparation performs the established full request,
prevout, inspected allocation and exact prospective-byte checks, retains complete
Applying intent and returns a noncloneable, nonserializable private-field Rust
capability. It performs export only, never RGB consumption or outgoing registration.
The host can now commit Applying plus approval consumption before consuming RGB.
Application moves the capability, binds the actual stock instance by Weak/Rc
identity, compares EVERY complete snapshot field except persistence sequence,
rechecks consumer-owned persistence and exact expiry, then performs established
consumption and outgoing registration. Errors preserve Applying; restored Applying
cannot recreate a capability. No JS export, signing surface, default authority,
restore-based application or new cryptography.

`src/wallet/offline.rs`: updates the two existing negative application tests and
adds three guard/recovery tests. The new tests use a clearly synthetic storage
capability constructed ONLY by cfg(test); they do NOT exercise successful actual
transfer preparation or consumption. They exercise real snapshot/stock/BDK state,
wrong-engine refusal, state drift, exact expiry, detached stock, dropped capability,
fresh same-wallet snapshot restore, sequence-only persistence changes, and actual
empty-coloring engine error retaining Applying and refusing preparation replay.

This fixes a source control-flow gap at predecessor255a048: its single application
call marked Applying and then consumed before returning, so the embedding host had
no acknowledgement point before consumption. No failed live/crash baseline is
claimed. Two acknowledged stores still do not establish a combined transaction.
The trusted host must commit a full authenticated intent/controller journal or one
combined authority record and revalidate full wallet authority AFTER each await.
This primitive cannot prove the host acknowledged anything.

## Executed checks and failures

Existing tools, no installation, network/funds or distributed artifact build:

```sh
cargo +stable test --offline --locked --lib application
cargo +stable test --offline --locked --lib
cargo +stable clippy --offline --locked --lib -- -D warnings
cargo +stable fmt --all -- --check
git diff --check
```

Targeted final5/5 PASS (0.63s). Initial new fixture compile failures: get_address
has no bool argument; reuse-address map uses KeychainKind rather than text.
Intermediate3/5 PASS/2 FAIL: fresh random wallet descriptor mismatch on restore,
and get_address invokes browser imports on native. Corrected to fresh restore
using the ORIGINAL WalletData and direct durable reuse-index mutation. No production
checks weakened, no fixture represents actual financial authority. Full native,
Clippy and formatter outcomes recorded in the coordinated BTCX report.

## Compatibility, evidence reuse and remaining finite source recipe

No shared wire/protocol/schema/pin/lock change. Prospective Rust embedding API
changes; no released product caller used the removed one-call method. The facade
source gate must select this exact new engine source in run scratch; distributed
facade/engine and wallet consumers stay pinned to the existing security tuple.
Previous144/144 engine evidence does not cover this split; new native results do.
Existing browser/controller/wallet evidence retains its ORIGINAL levels and cannot
certify the new host orchestration or successful engine application.

Next source operation: independently wallet-owned full accepted policy/ContractId,
role/order association, authenticated own/foreign prevouts and receive registry;
SAME-engine immutable review/confirmation; full recoverable approval/pending journal
using this prepare -> durable acknowledge -> post-await validation -> consume seam;
then durable Applied snapshot before any backend key operation. An IDB or backend
error must preserve consumed debit and complete intent; never auto-resign/reconsume.
Production signing stays fenced until the coherent integration is validated.

Necessary held operation for actual positive/crash proof: a fresh disposable
regtest wallet case with a real fake-asset allocation and independently authenticated
spend/receive records, full wallet-owned accepted terms, exact prospective transfer,
controller confirmation, journal checkpoint, SAME-engine consumption, process
interruption and fresh authenticated reconciliation. No existing consumed case may
be replayed or historical construction bytes relabelled as current authority.
Chief must reconcile full source/consumer/hash tuple and release applicable holds
before that financial envelope/artifact/real-signing operation. Unfunded synthetic
guard checks cannot substitute. No current operation release is requested here.

Original SERVED bilateral nonzero/both approvals/own signatures, exact construction,
SAME-case all-party BTC/RGB accounting, items5/16/18/19/R1/R4/immediate crash and
mandatory live reverse remain OPEN; R3 unavailable. Facade Clippy23 remains unwaived.
THE-872/THE-945 holds and reconnect chain/pins/artifacts retained. No adoption,
serving, rebuild/repin, funding/mining/broadcast, acceptance or merge. Rollback is
source revert, retaining all durable intent and consumed/failure cases.
