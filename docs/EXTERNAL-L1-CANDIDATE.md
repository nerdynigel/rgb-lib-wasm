# Browser external-L1 candidate

This candidate extends `1bfe909196d4d868f1e537f9f8123691acab0f4a`, not the broad
experimental external-send branch. It is not financial acceptance or a mainnet release.

The initial supported external profile is one NIA contract, one transition,
revealed witness recipients, exact conservation and wallet-owned RGB change.
Prepare uses the real database's transfer/coloring records and retains the
actual standard consignment bytes alongside the exact transaction/fascia and
recipient digest. Standard offchain RGB validation precedes DB preparation.
Concurrent reuse of pending inputs and conflicting retries fail closed.
Identical retries must flush before acknowledging success. Finalization never
broadcasts: the exact transaction must already be observed by the indexer.

Engine backups now use envelope version 2, bind network and both account xpubs,
and include pending transfer artifacts and received consignments. Restore first
validates a temporary engine and only accepts an empty target. Version 1 backups
cannot satisfy this recovery profile; they are not silently promoted. Restore
must be followed by an acknowledged flush. The secure facade does this.

This is BTC/RGB engine state only. A complete L1 wallet recovery additionally
needs its encrypted key vault, refusal journal and encrypted original partials.
It does not provide Lightning/channel recovery and must not overwrite a live
wallet with newer or ambiguous obligations. Consumer recovery must enforce those
boundaries and prove a fresh independent restore and spend.
