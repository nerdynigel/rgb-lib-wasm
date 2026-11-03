# Incoming RGB confirmed-conflict and balance correction

Candidate component only. Real preceding Lab recovery9a431647 and a later ordinary
spend44b05600 proved the source recovery and restored new spending. Recipient S still
reported old incomingd98 as WaitingConfirmations and future6 while settled/spendable2.
A pending witness was counted both as a requested amount and an actual Receive coloring.
The exact failing old-formula negative control and corrected regression are retained.

Normal refresh now checks a missing incoming NIA witness on REGTEST only. The actual
retained consignment must validate through standard RGB and live indexer ancestry,
exact contract/witness/owned External recipient bindings. Outspend hints alone cannot
change history: independently fetched distinct transaction bytes must spend the same
original input, be confirmed at a nonzero height with at least the requested minimum,
and the original must still be absent. Unconfirmed/unrelated/same-txid variants and
absence/expiry alone cannot retire a transfer. Only that incoming DB batch is Failed;
Stock, colorings, original consignments and every outgoing obligation remain intact.

Append-only detailed conflict receipts survive snapshots and complete encrypted backups.
Snapshot v2 and authenticated Native backup v4 block older readers that could drop
critical metadata. Existing no-receipt Native v2/v3 files remain readable. Restore
structurally validates receipts and their original DB/consignment/input bindings before
mutating the wallet. Malformed restore, metadata loss/revival, downgrade, raw404 and
confirmation identity boundaries have regressions.189 library tests, strict library
Clippy, wasm32 library check, formatter/diff pass. Initial compile/hash/visibility and
snapshot fixture failures are retained; none are called financial acceptance.

No dependencies or cryptographic primitives changed. No Lab transaction, signature,
funding, mining, ledger restore or hold release ran for these source tests. The exact
SDK/wallet pin migration, immutable source-only build, browser checks, guarded deployment,
actual S native history/balances and complete-file restore remain required. BTCX main
and independent financial acceptance remain held.
