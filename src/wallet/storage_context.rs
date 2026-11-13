//! Immutable browser storage partition. Not an identity, key or authorization.
//!
//! The legacy default keeps every existing storage name byte-for-byte. A new
//! wallet profile is selected before any persistence access; changing it needs
//! a new document/WASM instance. Background writes therefore cannot cross into
//! another wallet after a context switch. Encryption and snapshot formats stay
//! unchanged.

use std::cell::RefCell;

const PREFIX: &str = "asm-profile-v1:";

#[derive(Default)]
struct Context {
    profile: Option<String>,
    frozen: bool,
}

impl Context {
    fn configure(&mut self, profile: &str) -> Result<(), String> {
        let next = if profile == "default" {
            None
        } else {
            if !uuid_v4(profile) {
                return Err("Invalid browser wallet profile".into());
            }
            Some(profile.to_owned())
        };
        if self.frozen && self.profile != next {
            return Err("Wallet storage context is immutable; reopen the selected wallet".into());
        }
        self.profile = next;
        self.frozen = true;
        Ok(())
    }

    fn prefix(&mut self) -> String {
        self.frozen = true;
        self.profile
            .as_ref()
            .map(|id| format!("{PREFIX}{id}:"))
            .unwrap_or_default()
    }
}

fn uuid_v4(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            14 => *b == b'4',
            19 => matches!(b, b'8' | b'9' | b'a' | b'b'),
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(b),
        })
}

thread_local! {
    static CONTEXT: RefCell<Context> = RefCell::new(Context::default());
}

/// Set once, before opening storage or configuring the wallet's encryption key.
/// `default` selects the unmodified legacy layout; new IDs must be UUIDv4.
pub fn configure_browser_storage_context(profile: &str) -> Result<(), String> {
    CONTEXT.with(|context| context.borrow_mut().configure(profile))
}

/// Physical IndexedDB name. Use before awaiting the open, capturing the context.
pub fn browser_storage_database(base: &str) -> String {
    CONTEXT.with(|context| format!("{}{base}", context.borrow_mut().prefix()))
}

/// Physical localStorage key. Does not change the logical encryption domain.
pub fn browser_storage_key(logical: &str) -> String {
    CONTEXT.with(|context| format!("{}{logical}", context.borrow_mut().prefix()))
}

/// Decode only keys in this context when enumerating the shared localStorage.
/// Legacy readers deliberately exclude every new-profile key.
pub fn browser_storage_logical_key(physical: &str) -> Option<String> {
    CONTEXT.with(|context| {
        let prefix = context.borrow_mut().prefix();
        if prefix.is_empty() {
            (!physical.starts_with(PREFIX)).then(|| physical.to_owned())
        } else {
            physical.strip_prefix(&prefix).map(str::to_owned)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: &str = "12345678-1234-4234-8234-123456789abc";
    const B: &str = "87654321-4321-4321-a321-cba987654321";

    #[test]
    fn legacy_names_are_exact_and_freeze_before_any_new_context() {
        let mut context = Context::default();
        assert_eq!(context.prefix(), "");
        assert!(context.configure(A).is_err());
        assert!(context.configure("default").is_ok());
        assert_eq!(context.prefix(), "");
    }

    #[test]
    fn explicit_context_is_immutable_and_idempotent() {
        let mut context = Context::default();
        context.configure(A).unwrap();
        assert_eq!(context.prefix(), format!("{PREFIX}{A}:"));
        assert!(context.configure(B).is_err());
        assert!(context.configure("default").is_err());
        assert!(context.configure(A).is_ok());
        assert_eq!(context.prefix(), format!("{PREFIX}{A}:"));
    }

    #[test]
    fn malformed_context_never_mutates_or_locks_the_previous_context() {
        for value in [
            "",
            "../wallet",
            "Default",
            "default/",
            "default\0",
            A.to_uppercase().as_str(),
            "12345678-1234-1234-8234-123456789abc",
            "12345678-1234-4234-c234-123456789abc",
            "12345678-1234-4234-8234-123456789abc:other",
        ] {
            let mut context = Context::default();
            assert!(context.configure(value).is_err(), "{value:?}");
            assert!(!context.frozen);
            context.configure(A).unwrap();
        }
    }

    #[test]
    fn new_context_never_reads_legacy_or_another_profiles_local_keys() {
        configure_browser_storage_context(A).unwrap();
        assert_eq!(
            browser_storage_database("rgb_lib_wallet"),
            format!("{PREFIX}{A}:rgb_lib_wallet")
        );
        assert_eq!(
            browser_storage_database("rln_wasm_sdk_runtime"),
            format!("{PREFIX}{A}:rln_wasm_sdk_runtime")
        );
        let key = browser_storage_key("rln:owned:record");
        assert_eq!(
            browser_storage_logical_key(&key).as_deref(),
            Some("rln:owned:record")
        );
        assert_eq!(browser_storage_logical_key("rln:owned:record"), None);
        assert_eq!(
            browser_storage_logical_key(&format!("{PREFIX}{B}:rln:owned:record")),
            None
        );
    }

    #[test]
    fn legacy_enumeration_excludes_new_profile_keys() {
        assert_eq!(browser_storage_key("rln:owned:record"), "rln:owned:record");
        assert_eq!(
            browser_storage_logical_key("rln:owned:record").as_deref(),
            Some("rln:owned:record")
        );
        assert_eq!(
            browser_storage_logical_key(&format!("{PREFIX}{A}:rln:owned:record")),
            None
        );
    }
}
