//! Demo shortcuts (ADR-011 pitch; Victor, 2026-10-30: "where something is
//! limited, make the demo look polished with a clearly-scoped shortcut").
//!
//! Every shortcut in this service is keyed off two environment switches and
//! an organization check, so that nothing here can reach a real beta
//! organization: `GENUI_DEMO_MODE` (`1`/`true`; off by default) turns the
//! shortcuts on, and `GENUI_DEMO_ORGANIZATIONS` (comma-separated organization
//! ids) names the synthetic demo organizations they apply to. A request whose
//! organization is not listed behaves exactly as without demo mode. Each
//! shortcut is registered in ticket APP-098 ("Demo shortcuts register") with
//! what it replaces and its proper fix; the audited commit, the permission
//! model, the receipts and the live updates are never shortcut.
//!
//! DS-05: this module is the switch. Every call site of [`is_demo_org`] (and
//! of `Composer::demo_for` and `query::canvas::is_demo`, which require it)
//! carries a `DS-xx` comment naming its register entry; `tests::switch_off`
//! shows that with the switch off none of them answers.
use std::sync::{OnceLock, RwLock};

pub const MODE_ENV: &str = "GENUI_DEMO_MODE";
pub const ORGANIZATIONS_ENV: &str = "GENUI_DEMO_ORGANIZATIONS";

struct Settings {
    enabled: bool,
    organizations: Vec<String>,
}

static SETTINGS: OnceLock<RwLock<Settings>> = OnceLock::new();

fn settings() -> &'static RwLock<Settings> {
    SETTINGS.get_or_init(|| RwLock::new(from_env()))
}

fn from_env() -> Settings {
    from_values(
        std::env::var(MODE_ENV).ok().as_deref(),
        std::env::var(ORGANIZATIONS_ENV).ok().as_deref(),
    )
}

/// The switches from their two values: absent or anything but `1`, `true`,
/// `on` or `yes` is off (the default in code).
fn from_values(mode: Option<&str>, organizations: Option<&str>) -> Settings {
    let enabled = mode
        .map(|value| matches!(value.trim(), "1" | "true" | "on" | "yes"))
        .unwrap_or(false);
    let organizations = organizations
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|id| crate::storage::valid_identifier(id))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Settings {
        enabled,
        organizations,
    }
}

/// Whether `GENUI_DEMO_MODE` is on, for any organization. Only the demo's
/// process-wide data uses this: whether the persona display names load at
/// all (APP-098 DS-02, DS-44). Showing one, like every per-organization
/// shortcut, also needs `is_demo_org`.
pub fn is_on() -> bool {
    settings().read().map(|s| s.enabled).unwrap_or(false)
}

/// Whether demo mode is on and `organization` is one of the listed demo
/// organizations. False for every real organization, always.
pub fn is_demo_org(organization: &str) -> bool {
    settings()
        .read()
        .map(|s| s.enabled && s.organizations.iter().any(|id| id == organization))
        .unwrap_or(false)
}

/// Tests and campaigns: set the switches without the environment.
#[cfg(test)]
pub fn configure_for_tests(enabled: bool, organizations: &[&str]) {
    if let Ok(mut current) = settings().write() {
        current.enabled = enabled;
        current.organizations = organizations.iter().map(|id| id.to_string()).collect();
    }
}

/// Unit tests that change the switches hold this for their whole run, so two
/// of them never interleave (the settings are process-wide).
#[cfg(test)]
pub(crate) fn lock_for_tests() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_is_off_unless_set_to_an_on_value() {
        let org = "00000000-0000-4000-8000-000000000001";
        let listed = Some(org);
        // Absent: off, even with the organization listed.
        let absent = from_values(None, listed);
        assert!(!absent.enabled);
        assert_eq!(absent.organizations, [org]);
        for off in ["", "0", "false", "off", "no", "TRUE", "On", "2", "enabled"] {
            assert!(!from_values(Some(off), listed).enabled, "{off:?} is off");
        }
        for on in ["1", "true", "on", "yes", " 1 "] {
            assert!(from_values(Some(on), listed).enabled, "{on:?} is on");
        }
        // Only identifier-shaped organization ids are kept.
        let parsed = from_values(Some("1"), Some(&format!(" {org}, not-an-id,,")));
        assert_eq!(parsed.organizations, [org]);
        assert!(from_values(Some("1"), None).organizations.is_empty());
    }

    #[test]
    fn demo_shortcuts_apply_to_listed_organizations_only() {
        let _switch = lock_for_tests();
        let org = "00000000-0000-4000-8000-000000000001";
        let other = "00000000-0000-4000-8000-000000000002";
        configure_for_tests(false, &[org]);
        assert!(!is_demo_org(org), "off by default");
        configure_for_tests(true, &[org]);
        assert!(is_demo_org(org));
        assert!(!is_demo_org(other));
        assert!(!is_demo_org("not-an-id"));
        configure_for_tests(true, &[]);
        assert!(
            !is_demo_org(org),
            "no organization listed: no shortcut anywhere"
        );
        // Review: the composer's and the canvas's own lists go through this
        // switch too, so demo mode off turns every shortcut off.
        let composer =
            crate::composer::Composer::new(None, "r", "g").with_demo_organizations([org.into()]);
        let canvas: std::collections::BTreeSet<String> = [org.to_owned()].into();
        configure_for_tests(false, &[org]);
        assert!(!composer.demo_for(org));
        assert!(!crate::query::canvas::is_demo(&canvas, org));
        configure_for_tests(true, &[org]);
        assert!(composer.demo_for(org) && !composer.demo_for(other));
        assert!(crate::query::canvas::is_demo(&canvas, org));
        assert!(!crate::query::canvas::is_demo(&Default::default(), org));
        configure_for_tests(false, &[]);
    }

    mod switch_off {
        //! APP-098 acceptance 1: with `GENUI_DEMO_MODE` off no demo shortcut runs,
        //! even for an organization that `GENUI_DEMO_ORGANIZATIONS` still lists.
        //!
        //! Each entry point is called as its route calls it: the demo flag is
        //! computed by `demo::is_demo_org` (or `Composer::demo_for` /
        //! `query::canvas::is_demo`, which require it), and every rehearsed input is
        //! one a shortcut answers. No model is configured, so a shortcut is the only
        //! way any of these could answer: with the switch off each must refuse as
        //! unavailable. The control turns the switch on and shows that the same
        //! inputs are answered by the shortcut, so the switch is what stands between
        //! them. Inline gates that cannot run without a store (DS-65, DS-73 to DS-75,
        //! DS-77 to DS-80) are covered by `is_demo_org` being false for the listed
        //! organization while their rehearsed predicates still match.
        use super::super::{configure_for_tests, is_demo_org, lock_for_tests};
        use crate::{
            actions::{
                prompt::{self, CatalogApp},
                receipt::{self, Receipt},
                ParseFailure, Parser,
            },
            composer::{Composer, TransformFailure},
            storage::LibraryApp,
            widgets::{
                customize::{Customizer, Failure, Source},
                state::Widget,
            },
        };
        use serde_json::Value;

        /// Used by no other test, so the control never turns a shortcut on for an
        /// organization another test is using.
        const ORG: &str = "00000000-0000-4000-8000-0000000d5098";
        const TODAY: &str = "2026-10-30";
        /// DS-43 and DS-69: rehearsed sentences of `docs/pitch/SCENARIOS.md`.
        const REHEARSED_UTTERANCES: [&str; 3] = [
            "I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses",
            "approve the Nobu dinner",
            "Book me off Nov 24–26 for Thanksgiving, Sarah approves",
        ];
        /// DS-53.
        const REHEARSED_TRANSFORM: &str = "Review expenses over $200";
        /// DS-66.
        const REHEARSED_RESTYLE: &str = "make it a TI-84 in Matrix style";

        /// A library app from its fixture, as the action catalog reads it.
        fn library(name: &str) -> LibraryApp {
            let path = format!(
                "{}/../contracts/fixtures/library/{name}.json",
                env!("CARGO_MANIFEST_DIR")
            );
            let bundle: Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let definition = &bundle["definition"];
            let content = definition["content"].clone();
            let schema = &content["record_schemas"][0];
            LibraryApp {
                app: definition["app_id"].as_str().unwrap().into(),
                publication: bundle["publication"]["publication_id"]
                    .as_str()
                    .unwrap()
                    .into(),
                definition_digest: definition["digest"].as_str().unwrap().into(),
                title: content["title"].as_str().unwrap().into(),
                description: content["description"].as_str().unwrap().into(),
                inputs: Vec::new(),
                record_schemas: vec![(
                    schema["id"].as_str().unwrap().into(),
                    schema["fields"].clone(),
                )],
                views: Vec::new(),
                content,
            }
        }

        /// The rehearsed receipt (DS-57), when this build context carries it.
        fn rehearsed_receipt() -> Option<Receipt> {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../docs/pitch/receipt-fixture.png");
            let bytes = std::fs::read(path).ok()?;
            Some(Receipt {
                media_type: receipt::sniff(&bytes)?,
                bytes,
            })
        }

        /// What each shortcut entry point answers for its rehearsed input, with the
        /// demo flag computed as its route computes it.
        struct Answers {
            /// DS-43, DS-69: `Ok(true)` is a rehearsed parse.
            parses: Vec<Result<bool, ParseFailure>>,
            /// DS-57; `None` when the fixture is not in this build context.
            receipt: Option<Result<bool, ParseFailure>>,
            /// DS-53: the transform's source.
            transform: Result<&'static str, TransformFailure>,
            /// DS-53: the compose route may answer without a model.
            compose_available: bool,
            /// DS-66.
            restyle: Result<Source, Failure>,
            /// DS-40 (and DS-80, which reads `is_demo_org` directly).
            canvas_demo: bool,
            /// The inline gates of DS-65, DS-73 to DS-75 and DS-77 to DS-79.
            inline_gate: bool,
        }

        fn answers(catalog: &[CatalogApp], expenses: &LibraryApp) -> Answers {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            // No model anywhere: only a shortcut can answer.
            let parser = Parser::new(None, "none");
            let composer = Composer::new(None, "none", "none")
                .with_matcher(false)
                .with_demo_organizations([ORG.to_owned()]);
            let customizer = Customizer::new(None, "none");
            let listed: std::collections::BTreeSet<String> = [ORG.to_owned()].into();
            runtime.block_on(async {
                let mut parses = Vec::new();
                for utterance in REHEARSED_UTTERANCES {
                    // actions/routes.rs and slack/flows.rs: `demo::is_demo_org`.
                    let demo = is_demo_org(ORG);
                    parses.push(
                        parser
                            .parse(utterance, catalog, demo, TODAY)
                            .await
                            .map(|parse| parse.rehearsed),
                    );
                }
                let receipt = match rehearsed_receipt() {
                    Some(image) => Some(
                        parser
                            .parse_receipt("", &image, catalog, is_demo_org(ORG), TODAY)
                            .await
                            .map(|parse| parse.rehearsed),
                    ),
                    None => None,
                };
                // composer/routes.rs: `Composer::demo_for`.
                let transform = composer
                    .transform(REHEARSED_TRANSFORM, expenses, composer.demo_for(ORG))
                    .await
                    .map(|(_, source)| source);
                // widgets/routes.rs and composer/talk.rs: `demo::is_demo_org`.
                let restyle = customizer
                    .customize(
                        Widget::Calculator,
                        &Widget::Calculator.default_state(),
                        REHEARSED_RESTYLE,
                        is_demo_org(ORG),
                    )
                    .await
                    .map(|outcome| outcome.source);
                Answers {
                    parses,
                    receipt,
                    transform,
                    compose_available: composer.available_for(ORG),
                    restyle,
                    canvas_demo: crate::query::canvas::is_demo(&listed, ORG),
                    inline_gate: is_demo_org(ORG),
                }
            })
        }

        #[test]
        fn with_the_switch_off_no_shortcut_answers_even_for_a_listed_organization() {
            let _switch = lock_for_tests();
            let catalog = prompt::catalog(&[library("expense-claims"), library("time-off")]);
            let expenses = library("expense-claims");
            // The inline gates' rehearsed inputs do match: only the switch keeps
            // them from the canned answer.
            assert!(crate::boxes::derive::rehearsed(
                "Build an app that shows the key metrics and explain them"
            ));
            assert!(crate::composer::shape::rehearsed_theme("dark green, compact").is_some());
            assert_eq!(
                crate::widgets::weather::fixture("Austin", false)["source"],
                "fixture"
            );

            // Off (the default in code), with the organization still listed.
            configure_for_tests(false, &[ORG]);
            let off = answers(&catalog, &expenses);
            for parse in &off.parses {
                assert_eq!(*parse, Err(ParseFailure::Unavailable), "DS-43/DS-69 off");
            }
            if let Some(receipt) = off.receipt {
                assert_eq!(receipt, Err(ParseFailure::Unavailable), "DS-57 off");
            }
            assert!(
                matches!(off.transform, Err(TransformFailure::Model(_))),
                "DS-53 off: {:?}",
                off.transform
            );
            assert!(!off.compose_available, "DS-53 off: no model, no compose");
            assert_eq!(off.restyle, Err(Failure::Unavailable), "DS-66 off");
            assert!(!off.canvas_demo, "DS-40 off");
            assert!(!off.inline_gate, "DS-65, DS-73..75, DS-77..80 off");

            // Control: on, the same inputs are answered by the shortcut.
            configure_for_tests(true, &[ORG]);
            let on = answers(&catalog, &expenses);
            configure_for_tests(false, &[]);
            for parse in &on.parses {
                assert_eq!(*parse, Ok(true), "DS-43/DS-69 on");
            }
            if let Some(receipt) = on.receipt {
                assert_eq!(receipt, Ok(true), "DS-57 on");
            }
            assert_eq!(on.transform, Ok("rehearsed"), "DS-53 on");
            assert!(on.compose_available, "DS-53 on");
            assert_eq!(on.restyle, Ok(Source::Rehearsed), "DS-66 on");
            assert!(on.canvas_demo && on.inline_gate);
        }
    }
}
