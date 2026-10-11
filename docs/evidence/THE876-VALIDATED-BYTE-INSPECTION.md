# Validated byte inspection — browser source gate, 2026-10-11 UTC

THE-876; registered workspace f85beea3, branch feat/the-876-transfer-export,
starting clean 8e4b9a06d5214ff50a3c4b7d6cbeb629808b1061. Authority: Chief
reconciliation revision428b65e5 and accepted execution scope cd57d63f.
Full acceptance remains OPEN. This is additive source, not spending authority.

## Established API boundary

`inspect_transfer_offchain` takes immutable strict transfer file bytes and exact
consensus-encoded COLORED transaction bytes. Limits: transfer8MB, transaction400KB.
It uses pinned rgb-ops0.11.1-rc.10 FileContent::load/save, compares round-trip
canonical bytes (the upstream loader alone accepts trailing bytes), and runs the
existing Consignment::validate with the known schema's trusted typesystem and
existing OffchainResolverWasm. Facts come from the returned ValidConsignment,
not a peer JSON assertion or a separately decoded transfer.

Exactly one matching witness bundle is required. Its revealed FULL transaction
must equal the supplied transaction byte-for-byte, including witness data;
txid equality alone is insufficient. Its known operation IDs must equal the
input-map operation IDs, with exactly one known terminal transition for the
contract. Other historical witness bundles remain available for validation.
Only OS_ASSET fungible assignments are supported: unknown/extra assignment
types, concealed recipient seals, external transaction seals, zero amounts,
duplicate vouts, OP_RETURN destinations and out-of-range outputs refuse.
Secret terminal modes refuse. Every remaining assignment is returned, including
principal/change/nonzero fee outputs; consumer expectation comparison must
reject missing/extra/unapproved rows. The engine does not infer commercial roles.

Result binds SHA256 of the original canonical bytes, contract ID, exact witness
transaction, txid, bundle ID, validated anchor JSON, operation ID, assignment
index, complete revealed seal, vout, decimal amount and transaction output
script. Anchor JSON is an opaque STRING and seal/amount are strings, preserving
u64 values without JS number rounding. Additive JS `inspectTransferOffchain`
exposes the same gate. No RGB validity rules or dependencies changed.

The function has no Wallet/Stock/database/persistence handle, async operation,
consume/import/flush/accept/register/sign/broadcast call. Decoding, validation
and local allocations are its only effects. Existing wallet construction,
initialization and explicit persistence are separate effects, documented in
THE876-TRANSFER-EXPORT.md. Secure facade consumers must retain lifecycle guards.

## Executed affected validation

Commands at the delivered source tree:

- cargo test --locked --lib inspect_transfer: PASS4, FAIL0.
- cargo check --locked --workspace --target wasm32-unknown-unknown: exit0.
- cargo test --locked --target wasm32-unknown-unknown --test wallet_wasm --no-run:
  exit0; affected real-browser test compiled.
- Existing Firefox wasm-bindgen0.2.117 runner, GECKODRIVER=/usr/local/bin/geckodriver,
  MOZ_HEADLESS=1, WASM_BINDGEN_TEST_ONLY_WEB=1 and loopback NO_PROXY, running
  target/wasm32-unknown-unknown/debug/deps/wallet_wasm-7976edaa0d409ec1.wasm
  test_color_export_fresh_wallet_refusal --exact --nocapture: PASS1, FAIL0.
- cargo fmt --all -- --check and git diff --check: exit0.

Malformed Bitcoin/file bytes and strict-decodable invalid synthetic transfer
bytes refuse. Trailing file bytes refuse before schema/validity checks.
Structural vectors cover revealed row extraction and concealed, zero, duplicate,
wrong-tx, out-of-range, OP_RETURN and nonfungible refusals. Transaction vectors
cover changed outputs, absent full witness, and changed witness with SAME txid.
Those vectors exercise private extraction checks but are NOT validated RGB
consignments. No positive strict RGB or valid-bundle ambiguity negative was run.
Real Firefox uses fresh random UNFUNDED wallet keys, baseline explicit flush,
malformed inspection and unowned export refusal, complete durable snapshot and
transfer/PSBT/contract preservation, explicit flush sequence+1, drop and restore.
This is refusal state evidence, not positive transfer/accounting/crash proof.

During development one WASM compile hit ambiguous Hash::hash (qualified), one
structural test import used upstream-private AssignVec (replaced by public serde
construction), and the vector's serde tag was corrected. Final checks above
passed; initial failures are not represented as passes.

## Remaining gates / rollback

Software owns secure facade adoption, isolated wallet artifacts, local receive
registry, authenticated own/foreign prevout amount/script facts, and exact
COLORED transaction consent/policy/association binding. No consumer pin changed
before this published source gate. Signing/executable capability remains disabled.
Fresh positive fixture mutation still needs Chief's itemized Reliability receipt
reconciliation. Native-origin provisioning, financial interruption/flush/crash
and independent acceptance remain open; THE-870/872/873/874 holds unchanged.

Rollback: restore matching isolated source pins/lock/manifest/JS/WASM together
and keep capability disabled. Never reset wallet state or replay actions.
Protocol1.9.0, native RLN, signer and rust-lightning pins unchanged. No schema or
shared protocol migration; isolated facade/wallet adoption required after gate.
