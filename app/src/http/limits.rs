//! In-process sliding-window limits (ADR-006 values; ADR-007 D7.5 and D7.18:
//! counters live in the single task's memory, a recorded limitation, not a
//! table). A check either admits every named key or records nothing, so a
//! refused request consumes no budget. Memory is bounded: past `MAX_KEYS`
//! live keys a new key is refused (fail closed) until old windows lapse. A
//! key is retained exactly as long as its own longest window (APP-009 review
//! m4), so the fail-closed refusal never outlives the budgets it protects.
//!
//! APP-222 part A (follow-up A13, the abuse and infrastructure tier): the
//! MCP and agent-token windows are deployment settings ([`Deployment`],
//! `GENUI_LIMIT_*`), read once at startup and validated, with today's
//! numbers as defaults; every other window here is a compiled constant.
use std::{
    collections::{HashMap, VecDeque},
    ops::RangeInclusive,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const MAX_KEYS: usize = 20_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

/// ADR-006 `POST /signin/{handle}` per address class: 40/minute and 150/hour
/// (ADR-006 said 10 and 60; amendment of 2026-11-14). APP-189 classroom
/// network risk: a class of 25 signs in at once from one address behind NAT.
pub const SIGNIN_ADDRESS: &[Window] = &[per(40, 60), per(150, 3600)];
/// ADR-006 `POST /signin/{handle}`: 6,000/hour across the service (ADR-006
/// said 600; amendment of 2026-11-15, APP-215 identity H1). The limiter
/// records every admitted attempt, judged or not, so a few addresses each
/// under their own window could fill a 600 window and refuse the whole
/// class's sign-in for an hour. The per-address windows are unchanged: they
/// are what protects fAuth's bucket and SES.
pub const SIGNIN_GLOBAL: &[Window] = &[per(6000, 3600)];
/// ADR-006 `POST /invite/validate`: 5/minute per address class (under fAuth's
/// registration burst of 5; codes are redeemed at home), and 100/day
/// (ADR-006 said 20; amendment of 2026-11-14). APP-189 classroom network
/// risk: 25 invitations may still come from one shared address.
pub const INVITE_ADDRESS: &[Window] = &[per(5, 60), per(100, 86_400)];
/// ADR-006 `POST /invite/validate`: 2,000/day across the service (ADR-006
/// said 200; amendment of 2026-11-15, APP-215 identity H1: Nov 28 is signup
/// day, and a filled day window would refuse every code until the next day).
pub const INVITE_GLOBAL: &[Window] = &[per(2000, 86_400)];
/// ADR-006 resend: a 60-second per-account cooldown mirrored ahead of fAuth's
/// own, and at most 5 per pending identity per 24 hours (D7.17).
pub const RESEND_ACCOUNT: &[Window] = &[per(1, 60), per(5, 86_400)];
/// APP-074 review M2 (ADR-006 amendment, callback row): `GET /auth/callback`
/// per address class, sized at the sum of the starts that precede a
/// legitimate return (sign-in 40/minute + invitation 5/minute; sign-in
/// 150/hour + invitation 100/day, APP-189), so no real return is refused.
pub const CALLBACK_ADDRESS: &[Window] = &[per(45, 60), per(250, 3600)];
/// The callback across the service: sign-in 6,000/hour + invitation
/// 2,000/day (the 2026-10-30 sum rule over the 2026-11-15 values).
pub const CALLBACK_GLOBAL: &[Window] = &[per(8000, 3600)];
/// Failed cookie lookups (no record, an unregistered organization) on
/// `GET /workspace` and `GET /auth/pending` per address class, at the
/// callback's ADR-006 size (15/minute, 80/hour): a legitimate browser fails a
/// lookup at most once per arrival before its cookie is replaced. A room of
/// 25 arriving in the same minute with stale cookies could pass 15; an ended
/// session is not counted here and `/canvas` does not use this window.
pub const LOOKUP_FAILURE_ADDRESS: &[Window] = &[per(15, 60), per(80, 3600)];

/// APP-113 review: per member, the routes that create or change rows with
/// no model call and no window of their own (folder create, the uploads
/// folder, folder rename and share, work share, the notes work, a
/// slash-command name) share this window.
pub const MEMBER_WRITES: &[Window] = &[per(120, 3600)];

/// APP-215 recheck (review of PR #137, MAJOR 1): the audited command route
/// (`POST /v1/work/{app}/{work}/commands/{command}`) has its own window per
/// member, separate from `MEMBER_WRITES`, so a teacher's dense class hour
/// (posting and closing two dozen items, grading two dozen entries, editing
/// the app) never shares a budget with composer saves, uploads and shares.
pub const COMMAND_WRITES: &[Window] = &[per(300, 3600)];

/// APP-099: per agent token, MCP messages an hour, unless
/// [`MCP_TOKEN_ENV`] says otherwise.
pub const MCP_TOKEN_PER_HOUR: usize = 120;
pub const MCP_TOKEN_ENV: &str = "GENUI_LIMIT_MCP_TOKEN_PER_HOUR";
/// APP-099 review: per member, across all their agent tokens, MCP messages
/// an hour (minting more tokens does not raise a member's rate), unless
/// [`MCP_MEMBER_ENV`] says otherwise.
pub const MCP_MEMBER_PER_HOUR: usize = 240;
pub const MCP_MEMBER_ENV: &str = "GENUI_LIMIT_MCP_MEMBER_PER_HOUR";
/// Per member, agent-token mints an hour, unless [`AGENT_TOKEN_MINTS_ENV`]
/// says otherwise.
pub const AGENT_TOKEN_MINTS_PER_HOUR: usize = 10;
pub const AGENT_TOKEN_MINTS_ENV: &str = "GENUI_LIMIT_AGENT_TOKEN_MINTS_PER_HOUR";
/// The values a `GENUI_LIMIT_*` setting may hold; anything else stops the
/// process at startup.
pub const SETTING_RANGE: RangeInclusive<usize> = 1..=100_000;

/// APP-222 (A13): the hourly windows a deployment sets. Every [`Limiter`]
/// carries one: the process's, which `main` reads from the environment
/// once and [`install`]s before any limiter exists, else the defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Deployment {
    mcp_token: [Window; 1],
    mcp_member: [Window; 1],
    agent_token_mints: [Window; 1],
}
impl Default for Deployment {
    fn default() -> Self {
        Self::DEFAULT
    }
}
impl Deployment {
    /// Today's numbers: 120 and 240 MCP messages an hour, 10 mints an hour.
    pub const DEFAULT: Self = Self {
        mcp_token: [per(MCP_TOKEN_PER_HOUR, 3600)],
        mcp_member: [per(MCP_MEMBER_PER_HOUR, 3600)],
        agent_token_mints: [per(AGENT_TOKEN_MINTS_PER_HOUR, 3600)],
    };
    /// The settings `lookup` names (an unset or empty one keeps its
    /// default); a value that is not a whole number in [`SETTING_RANGE`] is
    /// refused, naming the setting.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let hourly = |name: &str, default: usize| -> Result<[Window; 1], String> {
            let value = lookup(name);
            let value = value.as_deref().map(str::trim).filter(|v| !v.is_empty());
            let limit = match value {
                None => default,
                Some(value) => value
                    .parse::<usize>()
                    .ok()
                    .filter(|limit| SETTING_RANGE.contains(limit))
                    .ok_or_else(|| {
                        format!(
                            "{name} must be a whole number from {} to {}",
                            SETTING_RANGE.start(),
                            SETTING_RANGE.end()
                        )
                    })?,
            };
            Ok([per(limit, 3600)])
        };
        Ok(Self {
            mcp_token: hourly(MCP_TOKEN_ENV, MCP_TOKEN_PER_HOUR)?,
            mcp_member: hourly(MCP_MEMBER_ENV, MCP_MEMBER_PER_HOUR)?,
            agent_token_mints: hourly(AGENT_TOKEN_MINTS_ENV, AGENT_TOKEN_MINTS_PER_HOUR)?,
        })
    }
    /// [`Self::from_lookup`] over the process environment (a value that is
    /// not text is refused like an out-of-range one).
    pub fn from_env() -> Result<Self, String> {
        for name in [MCP_TOKEN_ENV, MCP_MEMBER_ENV, AGENT_TOKEN_MINTS_ENV] {
            if let Err(std::env::VarError::NotUnicode(_)) = std::env::var(name) {
                return Err(format!("{name} must be text"));
            }
        }
        Self::from_lookup(|name| std::env::var(name).ok())
    }
    /// Per agent token: MCP messages an hour.
    pub fn mcp_token(&self) -> &[Window] {
        &self.mcp_token
    }
    /// Per member, across all their agent tokens: MCP messages an hour.
    pub fn mcp_member(&self) -> &[Window] {
        &self.mcp_member
    }
    /// Per member: agent-token mints an hour.
    pub fn agent_token_mints(&self) -> &[Window] {
        &self.agent_token_mints
    }
}

static DEPLOYMENT: OnceLock<Deployment> = OnceLock::new();
/// The process's [`Deployment`], set once by `main` at startup (a second
/// call changes nothing).
pub fn install(deployment: Deployment) {
    let _ = DEPLOYMENT.set(deployment);
}

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

pub struct Limiter {
    events: Mutex<HashMap<String, Log>>,
    deployment: Deployment,
}
/// A limiter with the process's installed [`Deployment`] (else the defaults).
impl Default for Limiter {
    fn default() -> Self {
        Self::with_deployment(DEPLOYMENT.get().copied().unwrap_or_default())
    }
}
impl Limiter {
    /// A limiter whose deployment windows are `deployment`'s (tests set
    /// lowered values here).
    pub fn with_deployment(deployment: Deployment) -> Self {
        Self {
            events: Mutex::default(),
            deployment,
        }
    }
    /// The deployment windows this limiter applies.
    pub fn deployment(&self) -> &Deployment {
        &self.deployment
    }
    /// `Ok(())` admits and records one event under every key; `Err(seconds)`
    /// refuses, naming when the earliest blocking window frees a slot.
    pub fn admit(&self, checks: &[(&str, &[Window])]) -> Result<(), u64> {
        self.admit_at(checks, Instant::now())
    }
    /// One row-creating or row-changing request of a member (`MEMBER_WRITES`).
    pub fn admit_member_write(&self, organization: &str, principal: &str) -> Result<(), u64> {
        self.admit(&[(&format!("writes:{organization}:{principal}"), MEMBER_WRITES)])
    }

    /// One audited command of a member (`COMMAND_WRITES`).
    pub fn admit_command_write(&self, organization: &str, principal: &str) -> Result<(), u64> {
        self.admit(&[(
            &format!("commands:{organization}:{principal}"),
            COMMAND_WRITES,
        )])
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

    /// APP-222 (A13): the three deployment settings default to today's
    /// numbers, follow a value from 1 to 100,000, and refuse anything else
    /// by name (`main` then refuses to start).
    #[test]
    fn deployment_settings_default_to_todays_numbers_and_refuse_out_of_range() {
        let none = Deployment::from_lookup(|_| None).unwrap();
        assert_eq!(none, Deployment::DEFAULT);
        assert_eq!(none.mcp_token()[0].limit, 120);
        assert_eq!(none.mcp_member()[0].limit, 240);
        assert_eq!(none.agent_token_mints()[0].limit, 10);
        for window in [
            none.mcp_token(),
            none.mcp_member(),
            none.agent_token_mints(),
        ] {
            assert_eq!(window.len(), 1);
            assert_eq!(window[0].period, Duration::from_secs(3600));
        }
        assert_eq!(Limiter::with_deployment(none).deployment(), &none);
        let set = |name: &'static str, value: &'static str| {
            Deployment::from_lookup(move |asked| (asked == name).then(|| value.to_owned()))
        };
        assert_eq!(set(MCP_TOKEN_ENV, "7").unwrap().mcp_token()[0].limit, 7);
        assert_eq!(set(MCP_MEMBER_ENV, " 3 ").unwrap().mcp_member()[0].limit, 3);
        assert_eq!(
            set(AGENT_TOKEN_MINTS_ENV, "100000")
                .unwrap()
                .agent_token_mints()[0]
                .limit,
            100_000
        );
        assert_eq!(set(MCP_MEMBER_ENV, "1").unwrap().mcp_token()[0].limit, 120);
        assert_eq!(set(MCP_MEMBER_ENV, "").unwrap(), Deployment::DEFAULT);
        for name in [MCP_TOKEN_ENV, MCP_MEMBER_ENV, AGENT_TOKEN_MINTS_ENV] {
            for value in [
                "0",
                "100001",
                "-1",
                "12.5",
                "many",
                "1e3",
                "99999999999999999999",
            ] {
                let refused = set(name, value).unwrap_err();
                assert!(refused.contains(name), "{refused}");
                assert!(refused.contains("1 to 100000"), "{refused}");
            }
        }
    }

    /// APP-113 review: the row-creating routes had no window; they share
    /// one per member (another member is unaffected).
    /// APP-215 recheck: commands have their own, larger window, separate
    /// from the member's other writes and from other members.
    #[test]
    fn command_writes_are_limited_per_member_and_apart_from_other_writes() {
        let limiter = Limiter::default();
        let org = "00000000-0000-4000-8000-000000000001";
        assert_eq!(COMMAND_WRITES[0].limit, 300);
        for _ in 0..COMMAND_WRITES[0].limit {
            assert!(limiter.admit_command_write(org, "a").is_ok());
        }
        assert!(limiter.admit_command_write(org, "a").is_err());
        assert!(
            limiter.admit_member_write(org, "a").is_ok(),
            "other writes keep their own window"
        );
        assert!(limiter.admit_command_write(org, "b").is_ok());
    }

    #[test]
    fn member_writes_are_limited_per_member() {
        let limiter = Limiter::default();
        let org = "00000000-0000-4000-8000-000000000001";
        for _ in 0..MEMBER_WRITES[0].limit {
            assert!(limiter.admit_member_write(org, "a").is_ok());
        }
        assert!(limiter.admit_member_write(org, "a").is_err());
        assert!(limiter.admit_member_write(org, "b").is_ok());
    }

    #[test]
    fn windows_admit_up_to_their_limit_and_refusals_record_nothing() {
        let limiter = Limiter::default();
        let start = Instant::now();
        // APP-189: the sign-in minute window is 40 per address class.
        let per_minute = SIGNIN_ADDRESS[0].limit;
        for _ in 0..per_minute {
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
        // of the global window (one minute's worth recorded per class, never
        // the refused attempt).
        for _ in 0..per_minute {
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
        assert_eq!(
            limiter.events.lock().unwrap()["global"].times.len(),
            2 * per_minute
        );
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
        // APP-189 classroom network risk: the day window is 100 (ADR-006: 20),
        // reached here a minute's five at a time.
        let per_minute = INVITE_ADDRESS[0].limit;
        let mut at = start;
        for _ in 1..INVITE_ADDRESS[1].limit / per_minute {
            at += Duration::from_secs(61);
            for _ in 0..per_minute {
                assert!(limiter.admit_blocked_at(&checks("a"), at).is_ok());
            }
        }
        let day = limiter
            .admit_blocked_at(&checks("a"), at + Duration::from_secs(61))
            .unwrap_err();
        assert_eq!((day.key, day.period), (0, Duration::from_secs(86_400)));
        assert!(day.retry_after > 80_000);
        // APP-189 classroom network risk (ADR-006 amendment, 2026-11-14): a
        // room of 25 signs in at once from one address; codes are redeemed at
        // home.
        assert_eq!(
            (
                SIGNIN_ADDRESS[0].limit,
                SIGNIN_ADDRESS[1].limit,
                INVITE_ADDRESS[0].limit,
                INVITE_ADDRESS[1].limit
            ),
            (40, 150, 5, 100)
        );
        // The service window: 2,000 a day across classes (APP-215 identity
        // H1; ADR-006 said 200), filled here five at a time from distinct
        // classes so no class window refuses first.
        assert_eq!(
            (
                SIGNIN_GLOBAL[0].limit,
                INVITE_GLOBAL[0].limit,
                CALLBACK_GLOBAL[0].limit
            ),
            (6000, 2000, 8000)
        );
        let global = Limiter::default();
        for i in 0..INVITE_GLOBAL[0].limit {
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
