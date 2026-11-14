//! The beta notice (ADR-008 amendment 2026-11-14, "Beta notice, version 1").
//! The invitation page shows it, and the required `consent` checkbox records
//! the member's agreement to exactly this version: `/invite/validate`
//! refuses without `consent=1`, the reservation carries `NOTICE_VERSION`,
//! `admit_redemption` stores it in `genui.member_consents` and appends
//! `admission.consented` on the admission chain (migration 0023).
//!
//! The text is general platform text, never a cohort's or a person's. Change
//! it only together with `NOTICE_VERSION`: a new version is a new consent.

/// The version the checkbox consents to. Bump it with every change to the text.
pub const NOTICE_VERSION: i32 = 1;

/// The notice's heading, as shown above the paragraphs.
pub const NOTICE_TITLE: &str = "Before you join this Forma workspace";

/// The notice body, one paragraph per entry, in order. A leading
/// `**Label:** ` names the paragraph's lead-in, rendered as a bold span.
pub const NOTICE_PARAGRAPHS: [&str; 6] = [
    "Forma (getforma.dev) is in early beta. It may change, break or go offline without notice, and there is no service guarantee. You must be 18 or older to take part.",
    "You are joining an organization's workspace. Its admins can see who has joined, your agreement to this notice, when you were last active, and how much you have created. Apps shared with you may collect what you enter. The app's owner and its reviewers can see what you enter; unless the app keeps entries private, so can the other people it is shared with. Anything you build is private unless you share it. Forma's operators can access stored data to run and support the service.",
    "**What we keep:** your email address for sign-in, the name you choose, what you create and enter, the files you upload, and basic activity logs. Data is stored in the United States.",
    "**AI:** to build apps and answer questions, your prompts, and the files and entries you ask it to work with, are sent to Anthropic, our AI provider. Please don't upload sensitive personal, health or payment information.",
    "**Your data:** you can ask at any time, at feedback@getforma.dev, to remove your access and delete the files you uploaded; entries you made in shared apps stay until the end of the test. This workspace's data is deleted after the test, by December 14, 2026; backup copies expire about a week later. A minimal security log of account events (identifiers and times, no content) is kept. Your sign-in account (email and password) is held by our identity provider; we ask it to delete the account after the test. Copy out anything you want to keep before then.",
    "By checking the box you agree to take part on these terms.",
];

/// The checkbox's label.
pub const CONSENT_LABEL: &str = "I have read the notice and agree to take part on these terms.";

/// One paragraph split into its optional bold lead-in and the rest.
pub fn lead_in(paragraph: &str) -> (Option<&str>, &str) {
    match paragraph.strip_prefix("**") {
        Some(rest) => match rest.split_once("** ") {
            Some((lead, body)) => (Some(lead), body),
            None => (None, paragraph),
        },
        None => (None, paragraph),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_is_version_one_and_general() {
        assert_eq!(NOTICE_VERSION, 1);
        let text = NOTICE_PARAGRAPHS.join("\n");
        assert!(text.contains("the name you choose"), "D6 names");
        assert!(text.contains("December 14, 2026"), "the purge date");
        assert!(text.contains("feedback@getforma.dev"));
        for word in ["class", "student", "teacher", "Victor"] {
            assert!(!text.contains(word), "cohort text: {word}");
        }
        assert_eq!(
            lead_in(NOTICE_PARAGRAPHS[2]),
            (
                Some("What we keep:"),
                "your email address for sign-in, the name you choose, what you create and enter, the files you upload, and basic activity logs. Data is stored in the United States."
            )
        );
        for phrase in [
            "18 or older",
            "operators can access",
            "United States",
            "backup copies expire about a week later",
            "minimal security log of account events (identifiers and times, no content)",
            "Copy out",
        ] {
            assert!(text.contains(phrase), "revised text: {phrase}");
        }
        // The "Your data" paragraph, word for word (the charter's PR #117 text).
        assert_eq!(
            lead_in(NOTICE_PARAGRAPHS[4]),
            (
                Some("Your data:"),
                "you can ask at any time, at feedback@getforma.dev, to remove your access and delete the files you uploaded; entries you made in shared apps stay until the end of the test. This workspace's data is deleted after the test, by December 14, 2026; backup copies expire about a week later. A minimal security log of account events (identifiers and times, no content) is kept. Your sign-in account (email and password) is held by our identity provider; we ask it to delete the account after the test. Copy out anything you want to keep before then."
            )
        );
        assert_eq!(lead_in(NOTICE_PARAGRAPHS[0]).0, None);
        assert_eq!(lead_in("**unterminated"), (None, "**unterminated"));
    }
}
