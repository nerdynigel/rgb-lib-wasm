# Select the outgoing batch for an RGB self-transfer

Actual isolated BTCX REGTEST R originalf73a963e481c17f4c194a088d19c198e87130156265aef903d84734750e31454
was submitted once and Core-confirmed at174 with767sat miner fee. Its own standard
consignment was acknowledged; actual native balances remain98233sats/4fakeRGB.
The wallet's final outgoing reconciliation refused. Original input/attempted
record, signed bytes and complete-v8 encrypted backups remain retained.

The old lookup chose the first batch for a Bitcoin txid. A self-transfer creates
separate incoming and outgoing batches with that same txid, so an earlier receive
row can be substituted for the sender operation. The sender's batch index and
lifecycle then differ from its immutable preparation and correctly fail review.

The lookup now selects the unique outgoing graph using persisted transfer roles.
Incoming-only records are never borrowed; empty, mixed-direction or duplicate
sender graphs fail closed. It does not alter DB/Stock rows, asset identity,
transaction/signature bytes, custody, transport, expiry or one-use/recovery gates.
No API/backup schema change. Existing synthetic tests now include the outgoing
transfer graph that a real preparation always persists.

Four regression cases cover an earlier incoming receipt with different status,
order-independent lookup/read immutability, incoming-only absence, mixed/duplicate
senders and incomplete graphs.21focused/native lifecycle tests and138full native
library tests PASS; rustfmt and cargo clippy --locked --lib PASS. These are local
unfunded tests, not actual RGB recovery acceptance. Next: controlled exact SDK
source pin/build/audit, then observe the existing R original and fresh complete
signed-file restore without any new prepare/sign/submission/mining.

Base67121bd50600502d922e71abf637b2258011bd6b. RLN5f0a3ae and wallet8d8f486
are still installed; this engine correction is NOT installed or financially
accepted at this checkpoint. Protocol1.19.1/contract identity unchanged. Mainnet,
real assets, unknown replay and automatic main merge remain excluded.
