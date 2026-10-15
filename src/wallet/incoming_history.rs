//! History policy shared by incoming confirmation preparation and application.
//! This predicate supplies no chain proof or settlement authority.
use rgbstd::validation::{Status, Warning};

pub(super) fn safe_to_accept(status: &Status) -> bool {
    !status
        .warnings
        .iter()
        .any(|warning| matches!(warning, Warning::UnsafeHistory(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_history_refuses_in_every_warning_position() {
        for position in 0..=3 {
            let mut status = Status::new();
            for _ in 0..3 {
                status.add_warning(Warning::Custom("diagnostic".into()));
            }
            status
                .warnings
                .insert(position, Warning::UnsafeHistory(Default::default()));
            assert!(!safe_to_accept(&status), "warning position {position}");
        }
    }

    #[test]
    fn safe_history_preserves_existing_custom_warning_policy() {
        let mut status = Status::new();
        assert!(safe_to_accept(&status));
        status.add_warning(Warning::Custom("diagnostic".into()));
        assert!(safe_to_accept(&status));
    }
}
