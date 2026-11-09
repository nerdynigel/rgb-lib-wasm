# Selected RGB inventory proof candidate

This additive REGTEST engine API supplies standard RGB evidence needed by the
market's independent inventory verifier. It is not a remote wallet attestation,
signing approval, reservation, executable depth or settlement result. No live
wallet or Lab dependency is promoted by this source change.

`Wallet::asset_inventory_proof(contract_id, outpoints)` accepts1–32 explicitly
selected, canonical sorted outpoints. It first reuses `asset_spend_view`, including
the actual BDK/DB/Stock agreement, maturity, other-contract/right exclusion and
native pending-original fences. An unknown, unavailable or duplicate point makes
the complete request fail. Only the supported NIA schema on REGTEST is exported.

The existing `Stock::transfer` exporter builds a standard RGB transfer file for
those seals. It does not create or consume a fascia. Serialization stops at524288
binary bytes, matching the dedicated public verifier's1048576hex-character bound.
The response includes the selected exact integer input observations, checkpoint,
contract and standard file. It contains no seed, private key, signer or standing
authority. No address, transaction, network request, database write, persistence
sequence advance or native reservation is created.

Consignments may disclose transaction ancestry. The secure wallet consumer must
obtain explicit local disclosure consent and exclude its durable browser holds.
The market must independently validate the standard RGB history and exact asset,
fresh confirmed/mature unspent Core view, wallet control, current online session
and shared reservations before describing any offered amount as verified. No
self-reported total, this response's scope label or connection signature supplies
those missing guarantees. General executable depth remains unavailable meanwhile.

Tests use real local BDK/DB/Stock graphs and upstream standard RGB validation,
with explicit REGTEST and the pinned NIA type system. They check selected totals,
repeated exact export, malformed/foreign/duplicate/mainnet/unconfirmed/immature
refusals, other-contract and retained-original exclusion, bounded serialization,
and unchanged DB/Stock/BDK staging/artifacts/persistence sequence. They are not
funded network or remote ownership evidence.

An existing preparation test assumed the first BDK output was a recipient. The
full suite exposed a legitimate shuffled change output at position0, causing its
changed-value assertion to test the wrong economic output. The test now finds
the exact approved recipient script and checks all four output-position
permutations against all six original tampering cases. Production preparation,
signing, fee and recovery checks remain unchanged; the original failure is kept.

Base engine: b44667b9f5c19cd06e3a33c3a5487b57fb86d42a. Dependencies and lockfiles
are unchanged. Validation and exact logs are recorded in the BTCX delivery branch
under `docs/evidence/asm-product-lab-release-20261101/maker-inventory-20261109`.
SDK wrapping, wallet disclosure/hold integration, standard remote verification,
ownership proof, freshness/deduplication/reservation integration, actual Lab proof
and independent acceptance remain necessary. Do not merge/promote this component
as completed liquidity readiness.

Verified candidate checks: `cargo fmt --all --check` PASS;
`cargo clippy --locked --lib -- -D warnings` PASS;
`cargo test --locked --lib` PASS257/257. Targeted proof/recovery subset PASS38;
all24 preparation tamper/permutation combinations PASS. Initial compile,
validator-default and full-suite shuffled-output failures are retained separately.
No assertion was removed, retry hidden or financial outcome relabelled.
