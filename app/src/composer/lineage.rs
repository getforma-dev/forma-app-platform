//! APP-101: which standard work an adapted work came from, so a teammate who
//! is viewing the shared standard work sees the reshape live.
//!
//! A composition that will be tailored registers its standard (retrieved)
//! work here; the transform records the adapted work it saved; the share
//! route records each share the composing member makes of the standard work.
//! Whichever of the two happens second sees the other (one lock covers both
//! records): an adaptation finishing after a share is shared with the same
//! teammates, and a share made after the adaptation also shares the adapted
//! work, each through the existing share path under the composing member's
//! own authority (`Data::share_work`: the sharer's subset, nothing more), and
//! the teammates' streams receive `app.adapted`. Only the composing member's
//! own shares count; nothing here grants anything by itself.
//!
//! Recorded limitation (APP-097 debt, as the in-process event hub, ADR-011
//! debt item 4): the record is in this process's memory, bounded to
//! [`CAPACITY`] compositions (oldest dropped first); after a restart, or on
//! another task, a share of the standard work no longer carries the adapted
//! one (the member can share the adapted card itself).
use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, OnceLock},
};

/// Compositions remembered.
pub const CAPACITY: usize = 4096;

/// A share to repeat: the grantee and whether `record_write` was asked for.
pub type Share = (String, bool);

#[derive(Default)]
struct Entry {
    owner: String,
    adapted: Option<(String, String)>,
    shares: Vec<Share>,
}

#[derive(Default)]
struct State {
    entries: HashMap<(String, String), Entry>,
    order: VecDeque<(String, String)>,
}

/// The process's lineage record.
#[derive(Default)]
pub struct Lineage {
    state: Mutex<State>,
}

pub fn lineage() -> &'static Lineage {
    static LINEAGE: OnceLock<Lineage> = OnceLock::new();
    LINEAGE.get_or_init(Lineage::default)
}

impl Lineage {
    /// A composition of `owner` placed `work` and will tailor it.
    pub fn begin(&self, organization: &str, owner: &str, work: &str) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let key = (organization.to_owned(), work.to_owned());
        if state.entries.contains_key(&key) {
            return;
        }
        while state.order.len() >= CAPACITY {
            if let Some(oldest) = state.order.pop_front() {
                state.entries.remove(&oldest);
            }
        }
        state.order.push_back(key.clone());
        state.entries.insert(
            key,
            Entry {
                owner: owner.to_owned(),
                ..Entry::default()
            },
        );
    }

    /// The transform saved `(app, adapted)` for the standard `work`; returns
    /// the shares the owner already made of the standard work.
    pub fn adapted(&self, organization: &str, work: &str, app: &str, adapted: &str) -> Vec<Share> {
        let Ok(mut state) = self.state.lock() else {
            return Vec::new();
        };
        match state
            .entries
            .get_mut(&(organization.to_owned(), work.to_owned()))
        {
            Some(entry) => {
                entry.adapted = Some((app.to_owned(), adapted.to_owned()));
                entry.shares.clone()
            }
            None => Vec::new(),
        }
    }

    /// The transform kept the defaults: forget the composition.
    pub fn kept(&self, organization: &str, work: &str) {
        if let Ok(mut state) = self.state.lock() {
            let key = (organization.to_owned(), work.to_owned());
            state.entries.remove(&key);
            state.order.retain(|k| k != &key);
        }
    }

    /// `sharer` shared the standard `work` with `grantee`; returns the
    /// adapted `(app, work)` to share too, when the sharer is the composing
    /// member and the adaptation already exists.
    pub fn shared(
        &self,
        organization: &str,
        sharer: &str,
        work: &str,
        grantee: &str,
        record_write: bool,
    ) -> Option<(String, String)> {
        let mut state = self.state.lock().ok()?;
        let entry = state
            .entries
            .get_mut(&(organization.to_owned(), work.to_owned()))?;
        if entry.owner != sharer {
            return None;
        }
        if !entry
            .shares
            .iter()
            .any(|(g, w)| g == grantee && *w == record_write)
        {
            entry.shares.push((grantee.to_owned(), record_write));
        }
        entry.adapted.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whichever_comes_second_sees_the_other_and_only_the_owner_counts() {
        let lineage = Lineage::default();
        lineage.begin("o", "maya", "w1");
        // A share before the adaptation is remembered...
        assert_eq!(lineage.shared("o", "maya", "w1", "sarah", true), None);
        // ...and handed to the adaptation.
        assert_eq!(
            lineage.adapted("o", "w1", "a2", "w2"),
            vec![("sarah".to_owned(), true)]
        );
        // A share after it names the adapted work.
        assert_eq!(
            lineage.shared("o", "maya", "w1", "diego", false),
            Some(("a2".into(), "w2".into()))
        );
        // Someone else's share of the same work carries nothing.
        assert_eq!(lineage.shared("o", "sarah", "w1", "diego", false), None);
        // Another organization's key is another composition.
        assert_eq!(lineage.shared("p", "maya", "w1", "sarah", false), None);
        lineage.kept("o", "w1");
        assert_eq!(lineage.shared("o", "maya", "w1", "sarah", true), None);
    }

    #[test]
    fn the_record_is_bounded() {
        let lineage = Lineage::default();
        for index in 0..CAPACITY + 10 {
            lineage.begin("o", "maya", &index.to_string());
        }
        let state = lineage.state.lock().unwrap();
        assert_eq!(state.entries.len(), CAPACITY);
        assert!(!state
            .entries
            .contains_key(&("o".to_owned(), "0".to_owned())));
    }
}
