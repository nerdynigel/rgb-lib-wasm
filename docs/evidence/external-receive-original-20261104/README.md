# Durable incoming RGB original — source candidate

The complete-input buyer previously performed a read-only RGB inspection and could sign without retaining the standard consignment. The new Native operation validates the standard NIA proof and live ancestry, binds the exact existing owned witness receive, authoritative contract, integer amount, output and batch, and flushes the original before acknowledgement. It cannot sign, broadcast or credit a balance. It refuses a replacement proof or reuse of the same receive for another transaction.

The existing incoming state machine uses the retained original after independently observed confirmations, without depending on the sender proxy or posting an ACK. Standard RGB checks and the existing receive/Stock path remain in use. An absent/unconfirmed transaction or expired receive never releases this original. The caller must still reconcile actual RGB settlement and every input.

Backup6 and managed snapshot4 preserve the exact bytes and refuse older-version downgrades. Complete backups3/4/5 remain readable. File imports fence new signing, while retaining exact original observation and confirmed registration. Snapshot CAS rejects stale writes, changed proofs, removed originals and lost restore fences. Actual owned-script/standard-proof/database checks run before installing a restored snapshot.

215 Native unit cases, strict library Clippy, formatter and wasm32 library compilation pass. New codec/CAS fixtures are explicitly synthetic; malformed-proof and offline/non-REGTEST tests do not prove positive async persistence. Funded sender-disappearance, file-only restore, complete-input settlement, recovery and independent acceptance remain required. No Lab operation, financial dispatch or main change occurred.
