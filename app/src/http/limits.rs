//! In-process sliding-window limits (ADR-006 values; ADR-007 D7.5 and D7.18:
//! counters live in the single task's memory, a recorded limitation, not a
//! table). A check either admits every named key or records nothing, so a
//! refused request consumes no budget. Memory is bounded: past `MAX_KEYS`
//! live keys a new key is refused (fail closed) until old windows lapse. A
//! key is retained exactly as long as its own longest window (APP-009 review
//! m4), so the fail-closed refusal never outlives the budgets it protects.
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

const MAX_KEYS: usize = 20_000;

#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub limit: usize,
    pub period: Duration,
}
pub const fn per(limit: usize, seconds: u64) -> Window {
    Window {
        limit,
        period: Duration::from_secs(seconds),
    }
}

/// ADR-006 `POST /signin/{handle}`: 10/minute and 60/hour per address class.
pub const SIGNIN_ADDRESS: &[Window] = &[per(10, 60), per(60, 3600)];
/// ADR-006 `POST /signin/{handle}`: 600/hour across the service.
pub const SIGNIN_GLOBAL: &[Window] = &[per(600, 3600)];
/// ADR-006 `POST /invite/validate`: 5/minute and 20/day per address class
/// (under fAuth's registration burst of 5).
pub const INVITE_ADDRESS: &[Window] = &[per(5, 60), per(20, 86_400)];
/// ADR-006 `POST /invite/validate`: 200/day across the service.
pub const INVITE_GLOBAL: &[Window] = &[per(200, 86_400)];
/// ADR-006 resend: a 60-second per-account cooldown mirrored ahead of fAuth's
/// own, and at most 5 per pending identity per 24 hours (D7.17).
pub const RESEND_ACCOUNT: &[Window] = &[per(1, 60), per(5, 86_400)];
/// APP-074 review M2 (ADR-006 amendment, callback row): `GET /auth/callback`
/// per address class, sized at the sum of the starts that precede a
/// legitimate return (sign-in 10/minute + invitation 5/minute; sign-in
/// 60/hour + invitation 20/day), so no real return is refused.
pub const CALLBACK_ADDRESS: &[Window] = &[per(15, 60), per(80, 3600)];
/// The callback across the service: sign-in 600/hour + invitation 200/day.
pub const CALLBACK_GLOBAL: &[Window] = &[per(800, 3600)];
/// Failed cookie lookups (no record, an unregistered organization) on
/// `GET /workspace` and `GET /auth/pending` per address class, at the
/// callback's size: a legitimate browser fails a lookup at most once per
/// arrival before its cookie is replaced.
pub const LOOKUP_FAILURE_ADDRESS: &[Window] = &[per(15, 60), per(80, 3600)];

/// Which check refused (APP-074 review m4): the index of its key in the
/// checks, the blocking window's period, and when that window frees a slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Blocked {
    pub retry_after: u64,
    pub key: usize,
    pub period: Duration,
}

/// One key's recorded events and its retention (its longest window).
struct Log {
    keep: Duration,
    times: VecDeque<Instant>,
}

#[derive(Default)]
pub struct Limiter {
    events: Mutex<HashMap<String, Log>>,
}
impl Limiter {
    /// `Ok(())` admits and records one event under every key; `Err(seconds)`
    /// refuses, naming when the earliest blocking window frees a slot.
    pub fn admit(&self, checks: &[(&str, &[Window])]) -> Result<(), u64> {
        self.admit_at(checks, Instant::now())
    }
    /// `admit`, naming which key and window refused.
    pub fn admit_blocked(&self, checks: &[(&str, &[Window])]) -> Result<(), Blocked> {
        self.admit_blocked_at(checks, Instant::now())
    }
    /// Test-only: forget every window (a campaign drives many outcomes for
    /// one account inside one cooldown period).
    #[cfg(test)]
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
    /// Whether every key would admit now, without recording anything.
    pub fn would_admit(&self, checks: &[(&str, &[Window])]) -> Result<(), u64> {
        let now = Instant::now();
        let events = self.events.lock().map_err(|_| 60u64)?;
        Self::judge(&events, checks, now).map_err(|blocked| blocked.retry_after)
    }
    /// The refusal whose window frees last (the one the caller must wait for).
    fn judge(
        events: &HashMap<String, Log>,
        checks: &[(&str, &[Window])],
        now: Instant,
    ) -> Result<(), Blocked> {
        let mut wait: Option<(Duration, usize, Duration)> = None;
        for (index, (key, windows)) in checks.iter().enumerate() {
            let Some(log) = events.get(*key) else {
                continue;
            };
            for window in *windows {
                let recent: Vec<&Instant> = log
                    .times
                    .iter()
                    .filter(|t| now.saturating_duration_since(**t) < window.period)
                    .collect();
                if recent.len() >= window.limit {
                    // The oldest counted event leaves the window first.
                    let free = window.period - now.saturating_duration_since(*recent[0]);
                    if wait.is_none_or(|(longest, _, _)| free > longest) {
                        wait = Some((free, index, window.period));
                    }
                }
            }
        }
        match wait {
            Some((wait, key, period)) => Err(Blocked {
                retry_after: wait.as_secs().max(1),
                key,
                period,
            }),
            None => Ok(()),
        }
    }
    fn admit_at(&self, checks: &[(&str, &[Window])], now: Instant) -> Result<(), u64> {
        self.admit_blocked_at(checks, now)
            .map_err(|blocked| blocked.retry_after)
    }
    fn admit_blocked_at(&self, checks: &[(&str, &[Window])], now: Instant) -> Result<(), Blocked> {
        let full = |key: usize| Blocked {
            retry_after: 60,
            key,
            period: Duration::from_secs(60),
        };
        let mut events = self.events.lock().map_err(|_| full(0))?;
        Self::judge(&events, checks, now)?;
        let longest =
            |windows: &[Window]| windows.iter().map(|w| w.period).max().unwrap_or_default();
        if events.len() >= MAX_KEYS {
            events.retain(|_, log| {
                log.times
                    .back()
                    .is_some_and(|t| now.saturating_duration_since(*t) < log.keep)
            });
        }
        for (index, (key, _)) in checks.iter().enumerate() {
            if !events.contains_key(*key) && events.len() >= MAX_KEYS {
                return Err(full(index));
            }
        }
        for (key, windows) in checks {
            let keep = longest(windows);
            let cap = windows.iter().map(|w| w.limit).max().unwrap_or(1);
            let log = events.entry((*key).to_owned()).or_insert_with(|| Log {
                keep,
                times: VecDeque::new(),
            });
            log.keep = log.keep.max(keep);
            while log
                .times
                .front()
                .is_some_and(|t| now.saturating_duration_since(*t) >= log.keep)
                || log.times.len() >= cap
            {
                log.times.pop_front();
            }
            log.times.push_back(now);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_admit_up_to_their_limit_and_refusals_record_nothing() {
        let limiter = Limiter::default();
        let start = Instant::now();
        for _ in 0..10 {
            assert!(limiter
                .admit_at(
                    &[
                        ("v4:203.0.113.7", SIGNIN_ADDRESS),
                        ("global", SIGNIN_GLOBAL)
                    ],
                    start
                )
                .is_ok());
        }
        let refused = limiter
            .admit_at(
                &[
                    ("v4:203.0.113.7", SIGNIN_ADDRESS),
                    ("global", SIGNIN_GLOBAL),
                ],
                start,
            )
            .unwrap_err();
        assert!((1..=60).contains(&refused));
        // Another class still has its own budget; the refusal consumed none
        // of the global window (10 recorded, not 11).
        for _ in 0..10 {
            assert!(limiter
                .admit_at(
                    &[
                        ("v4:198.51.100.1", SIGNIN_ADDRESS),
                        ("global", SIGNIN_GLOBAL)
                    ],
                    start
                )
                .is_ok());
        }
        assert_eq!(limiter.events.lock().unwrap()["global"].times.len(), 20);
        // A minute later the per-minute window frees; the hour still counts.
        let later = start + Duration::from_secs(61);
        assert!(limiter
            .admit_at(&[("v4:203.0.113.7", SIGNIN_ADDRESS)], later)
            .is_ok());
        let cooldown = Limiter::default();
        assert!(cooldown
            .admit_at(&[("acct", RESEND_ACCOUNT)], start)
            .is_ok());
        assert_eq!(
            cooldown.admit_at(&[("acct", RESEND_ACCOUNT)], start),
            Err(60)
        );
        for minute in 1..=4u64 {
            assert!(cooldown
                .admit_at(
                    &[("acct", RESEND_ACCOUNT)],
                    start + Duration::from_secs(61 * minute)
                )
                .is_ok());
        }
        assert!(cooldown
            .admit_at(
                &[("acct", RESEND_ACCOUNT)],
                start + Duration::from_secs(61 * 5)
            )
            .is_err());
    }

    /// APP-074 review m4: a refusal names its key and window, the one that
    /// frees last.
    #[test]
    fn a_refusal_names_the_window_that_blocked() {
        let limiter = Limiter::default();
        let start = Instant::now();
        let checks = |class: &'static str| [(class, INVITE_ADDRESS), ("global", INVITE_GLOBAL)];
        for _ in 0..5 {
            assert!(limiter.admit_blocked_at(&checks("a"), start).is_ok());
        }
        let minute = limiter.admit_blocked_at(&checks("a"), start).unwrap_err();
        assert_eq!((minute.key, minute.period), (0, Duration::from_secs(60)));
        let mut at = start;
        for _ in 0..3 {
            at += Duration::from_secs(61);
            for _ in 0..5 {
                assert!(limiter.admit_blocked_at(&checks("a"), at).is_ok());
            }
        }
        let day = limiter
            .admit_blocked_at(&checks("a"), at + Duration::from_secs(61))
            .unwrap_err();
        assert_eq!((day.key, day.period), (0, Duration::from_secs(86_400)));
        assert!(day.retry_after > 80_000);
        // The service window: 200 a day across classes.
        let global = Limiter::default();
        for i in 0..200 {
            let class = format!("c{}", i / 5);
            assert!(global
                .admit_blocked_at(
                    &[(&class, INVITE_ADDRESS), ("global", INVITE_GLOBAL)],
                    start
                )
                .is_ok());
        }
        let service = global
            .admit_blocked_at(
                &[("fresh", INVITE_ADDRESS), ("global", INVITE_GLOBAL)],
                start,
            )
            .unwrap_err();
        assert_eq!(
            (service.key, service.period),
            (1, Duration::from_secs(86_400))
        );
        // The callback windows hold the sum of the starts before them.
        assert_eq!(
            (
                CALLBACK_ADDRESS[0].limit,
                CALLBACK_ADDRESS[1].limit,
                CALLBACK_GLOBAL[0].limit
            ),
            (
                SIGNIN_ADDRESS[0].limit + INVITE_ADDRESS[0].limit,
                SIGNIN_ADDRESS[1].limit + INVITE_ADDRESS[1].limit,
                SIGNIN_GLOBAL[0].limit + INVITE_GLOBAL[0].limit
            )
        );
    }

    /// APP-009 review m4: with `MAX_KEYS` sign-in classes recorded, a new
    /// class is refused only while those keys' own windows (at most an hour)
    /// are live, not for a day.
    #[test]
    fn full_key_table_frees_with_each_keys_longest_window() {
        let limiter = Limiter::default();
        let start = Instant::now();
        for i in 0..MAX_KEYS {
            assert!(limiter
                .admit_at(&[(&format!("signin:c{i}"), SIGNIN_ADDRESS)], start)
                .is_ok());
        }
        assert_eq!(
            limiter.admit_at(
                &[("signin:new", SIGNIN_ADDRESS)],
                start + Duration::from_secs(60)
            ),
            Err(60),
            "fail closed while the hour windows are live"
        );
        assert!(limiter
            .admit_at(
                &[("signin:new", SIGNIN_ADDRESS)],
                start + Duration::from_secs(3_601)
            )
            .is_ok());
        // A resend key keeps its 24-hour window.
        let resend = Limiter::default();
        assert!(resend.admit_at(&[("acct", RESEND_ACCOUNT)], start).is_ok());
        assert_eq!(
            resend.events.lock().unwrap()["acct"].keep,
            Duration::from_secs(86_400)
        );
    }
}
