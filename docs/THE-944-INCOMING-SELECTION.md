# Exact incoming local selection — 15 October 2026

PARTIAL / NOT SAFE TO ACCEPT. This is the local selection stage of the already
approved generic exact incoming NIA consumer. It is not the consuming lifecycle.
Predecessor: 22b261814bdf7b185d0e008359f6a471bd5b91e2, existing PR3 / branch
feat/the-876-transfer-export. The runtime facade remains on engine a7210b82.

Added read-only Wallet::require_exact_pending_witness_receive, using the actual
engine DB types, RGB strict decoder and BDK script-membership check. It checks:

- one exact incoming WaitingConfirmations NIA receipt with nonzero confirmation
  threshold, canonical ContractId, witness txid/vout, exact consignment bytes
  (8,000,000-byte bound and canonical strict encoding) and checked integer units;
- unique positive row indexes, non-dangling transfer/asset/batch/coloring/TXO
  relationships, exactly one selected asset/transfer, no unrelated pending batch;
- the imported unconfirmed output placeholder, exclusively matching Receive
  colorings, no rejected ACK or incompatible requested assignment;
- one pending witness script, local indexed script membership and exact
  script-to-network-recipient correspondence; no pending seller intent, transfer
  artifacts or other retained incoming consignment.

The method takes shared self, performs no await/network call, snapshot-sequence
allocation, Stock/BDK mutation, persistence or signing. It returns only Result<()>;
it grants no consuming authority. Existing broad refresh/import paths are unchanged.
No consuming caller or facade integration is claimed. Local DB/byte agreement is
not RGB allocation/history validation, chain inclusion, expiry/consent validation
or settlement. Missing Stock/complete-source validation belongs to preparation.

Inventory: ADD src/wallet/incoming_selection.rs; MODIFY src/wallet/mod.rs;
ADD this evidence note. Dependency/lockfile/feature/schema/asset/signer/protocol
changes: NONE. Existing crate-wide lint settings unchanged; no new suppression.

Executed on the final source using existing host Rust1.97.1 and Cargo cache,
process-only PATH=/home/nigel/.cargo/bin:$PATH,
CARGO_HOME=/home/nigel/.cargo, RUSTUP_HOME=/home/nigel/.rustup,
RUSTUP_TOOLCHAIN=1.97.1-x86_64-unknown-linux-gnu:

- cargo test --offline --locked --lib incoming_selection -- --nocapture: PASS5/5
  before the final requested-assignment refusal cases; those final cases were
  executed by the full library run below.
- cargo test --offline --locked --lib: PASS154/154, including all five new tests.
  Four tests use synthetic actual DbData values; one uses an unfunded actual
  native Wallet/Stock/BDK and verifies refusal preserves every serialized
  snapshot field plus the BDK staged changeset. Its missing receipt/foreign
  recipient/changed bytes/empty-script refusals do not exercise a valid incoming
  consignment, successful full entry, network observation or application.
- cargo clippy --offline --locked --lib -- -D warnings: PASS.
- cargo clippy --offline --locked --lib --tests -- -D warnings: FAIL101 on
  tests/existing_snapshot_wasm.rs native compilation: four errors importing or
  calling wasm32-gated snapshot APIs (E0432/E0425/E0599). No test assertion or
  target gate changed to conceal that result; WASM validation remains open.
- changed-file rustfmt --edition 2024 --config skip_children=true --check and
  git diff --check: PASS. Initial unqualified rustfmt/cargo commands were absent
  from run PATH (exit127); explicit existing host paths resolved it, no install.

Next same owner: compose this local selector with one bounded frozen chain/history
observation using existing private PreFetchResolver. Derive the complete detached
Stock+BDK+TXO expected post without broad sync; bind an opaque preparation to the
exact source/SAME engine. Only consume after durable intent/debit acknowledgement,
then complete actual/post equality, journal CAS/read-back and served acknowledgement.
No sequence/timestamp masks, inferred rollback, replay or reconstructed authority.

Required affected checks remain: real valid incoming entry/prospective/actual
complete state and unrelated-state preservation; unsafe/missing/changed chain and
history refusal before mutation; stale source/session/expiry/revocation and
one-shot/cancellation behavior; cross-domain journal recovery. Chief releases
one grouped SDK default+TEST/strict Clippy/WASM ABI/artifact/native-IDB/browser
operation after source composition. Old SDK Clippy FAIL40 is unresolved.
Financial/accounting/R1/R4/immediate-kill/live-reverse and THE-945 remain open;
R3 unavailable. No artifact adoption, financial operation, PR approval or merge.
Rollback source only; preserve all journals/checkpoints/debits and consumed cases.
