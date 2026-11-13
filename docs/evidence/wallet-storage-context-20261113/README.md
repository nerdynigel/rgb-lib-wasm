# Immutable browser wallet storage contexts

Candidate implementation, not deployed or accepted recovery evidence. The
legacy context keeps exact database/key names; new UUIDv4 contexts get separate
IndexedDB and localStorage names. One context per document/WASM lifetime; no live
context switch, migration, key export, cryptographic change or state deletion.
RGB snapshot/Stock/transaction logic is unchanged. Five pure Rust tests pass
(`rustc --edition=2024 --test src/wallet/storage_context.rs`), covering legacy
compatibility, one-time/idempotent binding, invalid IDs and cross-context key
filtering. Real SDK/IndexedDB/browser reload and recovery tests remain required
before guarded promotion. No funds or network operations.
