//! APP-100 (ADR-011 pitch; demo shortcut DS-59 in APP-098): where a record
//! version was written from, for the canvas's origin chip ("via Slack").
//!
//! The audited command path records the actor principal on the hash chain;
//! it has no column for the channel the member acted through, and widening
//! the audit event body would change every chain's hash input. For the pitch
//! the channel is kept here, in process memory, keyed by the exact record
//! version a channel committed: the records listing adds `"via": "Slack"` to
//! a record whose current version carries a mark. A mark is a display label
//! only, never authority, and a restart forgets it (the chip disappears; the
//! audited record is unchanged). Proper fix (APP-097 revisit): an
//! actor-detail field on the command outcome or the audit event.
use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, OnceLock},
};

/// Marks kept at most (oldest forgotten first).
const CAPACITY: usize = 4096;

#[derive(Default)]
struct Marks {
    labels: HashMap<(String, i64), &'static str>,
    order: VecDeque<(String, i64)>,
}

fn marks() -> &'static Mutex<Marks> {
    static MARKS: OnceLock<Mutex<Marks>> = OnceLock::new();
    MARKS.get_or_init(|| Mutex::new(Marks::default()))
}

/// Records that `version` of `record` was committed through `label`'s channel.
pub fn mark(record: &str, version: i64, label: &'static str) {
    let Ok(mut marks) = marks().lock() else {
        return;
    };
    let key = (record.to_owned(), version);
    if marks.labels.insert(key.clone(), label).is_none() {
        marks.order.push_back(key);
    }
    while marks.order.len() > CAPACITY {
        if let Some(oldest) = marks.order.pop_front() {
            marks.labels.remove(&oldest);
        }
    }
}

/// The channel that committed `version` of `record`, when one was marked.
pub fn label(record: &str, version: i64) -> Option<&'static str> {
    marks()
        .lock()
        .ok()?
        .labels
        .get(&(record.to_owned(), version))
        .copied()
}

#[cfg(test)]
mod tests {
    #[test]
    fn marks_name_one_exact_version() {
        let record = "00000000-0000-4000-8000-0000000c0de1";
        super::mark(record, 2, "Slack");
        assert_eq!(super::label(record, 2), Some("Slack"));
        assert_eq!(super::label(record, 1), None);
        assert_eq!(
            super::label("00000000-0000-4000-8000-0000000c0de2", 2),
            None
        );
    }
}
