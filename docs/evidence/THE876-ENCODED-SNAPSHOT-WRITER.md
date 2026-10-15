# Explicit encoded snapshot persistence prerequisite — 12 October 2026 UTC

Plan35 / Chief comments b851d216 and 90a40124 applied. Sole Software writer,
registered engine f85beea3, branch feat/the-876-transfer-export; base
 a4fa12cc19927d6ec214f15d8159bebc54b8a690. Original acceptance OPEN.

Wallet::snapshot_for_persistence reuses flush's complete serialization, without
writes or scheduled backup. idb_store::save_encoded_snapshot is an explicit
consumer-codec seam: insert-only or byte-exact compare-and-replace inside the
same readwrite transaction as put; resolves ONLY on transaction completion.
Actual native request/transaction/open/schema errors propagate as static errors.
A blocked open that later succeeds closes without transaction. Explicit fresh
setup alone can create version1/snapshots; existing schema upgrades refuse.
No change to legacy flush/autobackup, strict reader, crypto, signing or defaults.
Codec authentication and monotonic sequence remain the facade's responsibility;
this low-level Rust seam alone does not establish encrypted financial durability.

Executed in the registered engine workspace:
- cargo test --locked --offline --target wasm32-unknown-unknown --test existing_snapshot_wasm --no-run: PASS.
- wasm-bindgen-test-runner ...existing_snapshot_wasm-024f3a2beba96ecc.wasm encoded_snapshot_commit_real_idb --exact --nocapture: PASS1 actual Firefox test.
- cargo check --locked --offline --target wasm32-unknown-unknown --workspace: PASS.
- cargo clippy --locked --offline --target wasm32-unknown-unknown --workspace -- -D warnings -A clippy::field_reassign_with_default: PASS, only previously disclosed baseline lint allowed.
- cargo fmt --all; git diff --check: PASS.

New test exercises real complete empty engine serialization/restore, absence of
implicit persistence, fresh insert, insert collision, stale expected record,
actual successful native put request followed by transaction abort (old bytes
preserved), successful replacement, missing-store refusal and higher-version
refusal with unchanged schema. These are UNFUNDED public watch-only states,
not pending-fill financial accounting. No external service, issuance, funding,
block, signing/broadcast or consumed case access. All existing loader tests
remain unchanged; no unchanged test claimed rerun.

Explicit retained Firefox profile: .state/the876-encoded-writer/
bbf98f60-fa47-49e8-93ef-509779f6718e, ignored; profile contents never published.
Runner capabilities via WASM_BINDGEN_TEST_WEBDRIVER_JSON run-scratch file;
GECKODRIVER=/usr/local/bin/geckodriver, MOZ_HEADLESS=1,
WASM_BINDGEN_TEST_ONLY_WEB=1, NO_PROXY/no_proxy=localhost,127.0.0.1. No install.

NEXT facade secure-session-bound codec/writer and fresh wallet release-artifact
positive/adversarial proof. Financial mutation/failure/crash recipe still needs
Chief release; consumed roots and all downstream/merge holds retained.
Rollback reverts only added seams/tests, preserving records and legacy paths.

Compatibility:
- btcx-wallet-protocol:1.9.0 @2a849f20a1d891748454f13e3d45b0d2a84c9c1b unchanged
- btcx:wallet pin dd4c12b846da9a1e1956766a35296893702102a4 unchanged
- btcx-wallet:f01e82272e8f6de5c6a8a7b753f53fb5d2335ad0 unchanged
- RLN:b424f66689e88f608d50d7a38be503f5ddbd497f; engine publication follows this source
- signer:074de663746b654bbd1cb9756e34c531101edec8 unchanged
- rust-lightning:0939f7751fb876e02caec9cd0a778794b81eee9e unchanged
- breaking protocol change:NO
- peer repo update required:YES isolated facade/wallet candidate adoption

## Insert-only undefined-record correction

Source review found that IDB get alone conflates absent keys with stored
undefined. The explicit writer now issues count(key) BEFORE get in the same
readwrite transaction, requires count0 for insert and count1 plus exact bytes
for replacement. Undefined/corrupt existing records cannot be silently replaced.
Correction to initial e2fd4f6 receipt: the test insertion did not apply, so that
rerun covered EXISTING cases only, not undefined collision. Profilee8935efc-bc6c-4bfe-bcf0-3f176f628770 retained. The explicit new regression is added and
executed below; do not attribute undefined proof to the preceding rerun.
Prior profile and initial source receipt preserved; neither result is financial
acceptance. Test compile/scoped fmt/diff PASS. Follow-up wallet release-artifact
regression reproduces the prior defect before the corrected candidate rebuild.

Explicit new undefined-record regression compiled and actually ran PASS in
fresh retained profile74a76e50-0a7b-45f6-bd45-66543c87b229. Test commits native
undefined, requires insert refusal and reads unchanged undefined afterwards.
Wallet old release artifact31371eaf separately reproduced FAIL (producerError
empty; before/after setup hash changed) on fresh retained profiledaf8c1da-0251-4662-b448-49fad5220e2e; corrected release wallet proof follows downstream.
