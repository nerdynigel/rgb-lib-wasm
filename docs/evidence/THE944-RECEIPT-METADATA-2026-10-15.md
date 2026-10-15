# Local receipt metadata correction

PARTIAL / NOT SAFE TO ACCEPT. Same-owner integration continuation; no financial,
artifact, independent-acceptance or merge release.

## Inventory and behavior

- PRODUCT `src/wallet/online.rs`, `src/wallet/rust_only.rs`: the existing local-only
  exact-byte import assigns unknown-asset `added_at` from the established receive
  batch's `created_at`. Normal network receive still passes None and uses the
  existing wall clock. Contract identity/genesis timestamp, validation, assignment,
  stock, signing and settlement behavior are unchanged. This is a product-path
  correction, **not a TEST-only delta**. No dependency upgrade or new API.
- TEST `src/wallet/offline.rs`: the funded seller constructor installs its initial
  stock before the established consumer persistence handoff. The address-only
  constructor deliberately removes stock; calling the handoff first refused.
  No change to ordinary runtime/persistence refusal or general funding helper.
- TEST `src/wallet/receipt_metadata_tests.rs`: real seller issuance/color/export,
  separate real buyer receive, older receiving event established before import,
  two independent complete-state restorations and actual local imports. Checks
  added_at, WaitingConfirmations and full result equality including stock and
  metadata. Finds actual colored recipient vout (coloring can insert OP_RETURN).
  No accepted authority/result injection, network, IDB, signing or settlement.

## Executed evidence

Existing Rust1.97.1 binaries; CARGO_HOME=/home/nigel/.cargo,
RUSTUP_HOME=/home/nigel/.rustup, RUSTUP_TOOLCHAIN=1.97.1.

`cargo test --offline --locked --lib --features test-fixtures receipt_metadata_tests -- -q`
PASS 1/1, 158 filtered. Prior clock behavior deliberately restored for reproduction:
FAIL at added_at equality (current wall timestamp versus receive-event1), exit101;
correction restored: PASS1/1. Assertions and all snapshot fields retained.
Initial test exposed existing TEST seller setup refusal (stock absent); next test
exposed incorrect unshifted vout in this new regression. Both setup issues fixed;
no failure evidence represented as product acceptance.

Scoped rustfmt --check and git diff --check PASS. `cargo clippy --offline --locked --lib --features test-fixtures -j 1 -- -D warnings` PASS.
Broader `--lib --tests` Clippy FAILED exit101 because existing WASM integration
tests import wasm32-only IDB APIs on the native target (E0432/E0425/E0599);
no tests or target gates weakened. The new native unit regression compiled and ran. Full SDK/default+TEST build, native/browser/controller/
root/CAS/IDB positives and negatives, served nonzero bilateral terms, same-case
accounting, immediate process kill, R1/R4/live reverse and independent acceptance
remain OPEN. R3 unavailable absent approved policy authority.

Prior native input11/11 and pure economics24/24 remain their original scopes.
Earlier source/artifact tuples do not validate this product correction. Re-pin
facade and wallet coherently; grouped actual build/browser validation must include
this native regression. Do not adopt existing generated artifacts. Roll back
source correction and dependent pins together, retaining all pending/ambiguous/
consumed records. No import/recovery replay authorized.
