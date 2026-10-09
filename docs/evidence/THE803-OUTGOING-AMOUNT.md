# THE-803 collaborative outgoing amount

Prior engine: b00362e9bb0ecf614c17a958ea1c045401a74c44.
Authoritative candidate receipt and companion refs are published in btcx-wallet
`docs/evidence/the-803/DELIVERY.md` on `feat/the-756-fixture-free-wallet` (PR35).

`register_collaborative_outgoing_transfer` saves Input1000 and Change900
colorings and one outgoing recipient-less DbTransfer. Previously
`get_transfer_data` excluded Input and exposed Change900 in `assignments`.
`list_transfers` therefore returned a Send900. The RLN facade serializes these
assignments unchanged; the wallet mapper reads the first Fungible assignment.

The correction projects only recipient-less Send rows to checked total fungible
Input minus Change, yielding Send100. Change colorings and balance accounting
remain independent. It also corrects reads of existing snapshots without a
migration. Normal recipient-bearing sends, receives, issuance and inflation
retain their current projection. No signature/schema/shared protocol change.
Registration checks sum overflow and excessive change before creating rows.
Exact txid replay compares asset, consumed outpoints, change vout/amount and
minimum confirmations; changed terms refuse instead of hiding behind a no-op.

Executed regression on prior source plus only the new sent100 assertion:
`cargo test --lib collaborative_registration_writes_surfaces_and_is_idempotent -- --nocapture`
failed with actual `[Fungible(900)]`, expected `[Fungible(100)]` (exit101).

Final validation commands (absolute cargo binary on this host):

- `cargo test --lib`: 112 passed, exit0.
- `cargo test --lib collaborative_ -- --nocapture`: targeted registration,
  exact arithmetic and overflow refusal tests; production list_transfers.
- `cargo check --lib --target wasm32-unknown-unknown`: exit0.
- `git diff --check`: exit0.

Coverage: production list_transfers Send100; independent Change900; one batch
and row; exact replay; altered change amount/vout, confirmations, asset and
spent outpoint refusal; serialized production WalletSnapshot restored to a
fresh wallet with the same test-generated identity, followed by three stable
reads preserving row/batch ids and amount; restored future900; no-change
Send1000; u64::MAX input minus (MAX-100) change gives100; over-change and input
sum overflow refuse before writes. Test-generated keys remain in memory and
are never exported in evidence. Public transfer JSON is captured by the test.

Limits: native tests use snapshot()/restore_from_snapshot(), not the browser
IndexedDB transaction or secure-session unlock API. Wallet mapper coverage uses
captured production engine JSON with a stub SDK. Actual facade transport and
fresh-pair live browser flush/unlock remain THE-800 after Chief reconciliation;
independent THE-742 remains later. Pending balances are not confirmed settlement.
No consumed pair, wallet import, signing, broadcast or operational setup was used.

Rollback: repin RLN/wallet to the prior frozen trio (fork b00362e, RLN060f1ef,
wallet439bb70). This restores the defective projection; Chief must keep live
proof held. Database/snapshot format is unchanged. No migration or reset needed.
