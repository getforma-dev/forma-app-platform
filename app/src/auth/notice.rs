//! The beta notice (ADR-008 amendment 2026-11-15, "Continuation after the
//! class", which records "Beta notice, version 2" beside version 1).
//! The invitation page shows it, and the required `consent` checkbox records
//! the member's agreement to exactly this version: `/invite/validate`
//! refuses without `consent=1`, the reservation carries `NOTICE_VERSION`,
//! `admit_redemption` stores it in `genui.member_consents` and appends
//! `admission.consented` on the admission chain (migration 0023).
//!
//! The text is general platform text, never a cohort's or a person's. Change
//! it only together with `NOTICE_VERSION`: a new version is a new consent.
//! Nothing re-asks an existing member: sign-in never reads `member_consents`,
//! so a bump changes only what new redemptions record.

/// The version the checkbox consents to. Bump it with every change to the text.
pub const NOTICE_VERSION: i32 = 2;

/// The notice's heading, as shown above the paragraphs.
pub const NOTICE_TITLE: &str = "Before you join this Forma workspace";

/// The notice body, one paragraph per entry, in order. A leading
/// `**Label:** ` names the paragraph's lead-in, rendered as a bold span.
pub const NOTICE_PARAGRAPHS: [&str; 7] = [
    "Forma (getforma.dev) is in early beta. It may change, break or go offline without notice, and there is no service guarantee or support commitment. You must be 18 or older to take part.",
    "You are joining an organization's workspace. Its owner and admins can see who has joined, your agreement to this notice, your role, when you were last active, and how much you have created. Apps shared with you may collect what you enter. The app's owner and its reviewers can see what you enter; unless the app keeps entries private, so can the other people it is shared with. Anything you build is private unless you share it. Forma's operators can access stored data to run and support the service.",
    "**What we keep:** your email address for sign-in, the name you choose, what you create and enter, the files you upload, and basic activity logs. Data is stored in the United States.",
    "**AI:** Forma includes a small daily allowance of AI use per member, to build apps and answer questions; when it is used up, building and Ask pause until the next day (UTC). Your prompts, and the files and entries you ask the AI to work with, are sent to Anthropic, our AI provider. If you connect your own AI agent through MCP, it runs on your own AI account and at your own cost, and Forma receives only what the agent sends it. Please don't upload sensitive personal, health or payment information.",
    "**After a trial period:** your account, your place in this workspace, your apps and your data continue on Forma's free plan; nothing is deleted when a course or test ends. Paid plans may be offered later, and we will not move you to one without asking. Allowances may change; we announce changes in the app.",
    "**Your data:** you can ask at any time, at privacy@getforma.dev, to delete your account and your data. We confirm within 7 days and complete the deletion within 30 days; backup copies expire about a week later. Deletion removes your sign-in account, your name, your apps, your uploads and what you entered in your own apps. Entries you made in apps that other people own stay with those apps, without your name. A minimal security log of account events (identifiers and times, no content) is kept. Your sign-in account (email and password) is held by our identity provider and is deleted with the rest.",
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
    fn the_notice_is_version_two_and_general() {
        // Version 2 (ADR-008 amendment 2026-11-15): accounts continue on the
        // free plan; deletion is on request. The title and the checkbox label
        // are version 1's, unchanged.
        assert_eq!(NOTICE_VERSION, 2);
        assert_eq!(NOTICE_TITLE, "Before you join this Forma workspace");
        assert_eq!(
            CONSENT_LABEL,
            "I have read the notice and agree to take part on these terms."
        );
        let text = NOTICE_PARAGRAPHS.join("\n");
        assert!(text.contains("the name you choose"), "D6 names");
        assert!(
            text.contains("privacy@getforma.dev"),
            "the data-request address"
        );
        // Version 1's promises, withdrawn by version 2.
        for gone in [
            "December 14",
            "feedback@getforma.dev",
            "deleted after the test",
            "end of the test",
            "Copy out",
        ] {
            assert!(!text.contains(gone), "version 1 text: {gone}");
        }
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
            "no service guarantee or support commitment",
            "Its owner and admins can see",
            "your role",
            "operators can access",
            "United States",
            "small daily allowance of AI use per member",
            "pause until the next day (UTC)",
            "connect your own AI agent through MCP",
            "at your own cost",
            "continue on Forma's free plan",
            "nothing is deleted when a course or test ends",
            "we will not move you to one without asking",
            "confirm within 7 days and complete the deletion within 30 days",
            "backup copies expire about a week later",
            "stay with those apps, without your name",
            "minimal security log of account events (identifiers and times, no content)",
            "deleted with the rest",
        ] {
            assert!(text.contains(phrase), "version 2 text: {phrase}");
        }
        // The two paragraphs that carry the continuation decision, word for
        // word (the plan's "Notice v2 draft", approved by Victor 2026-11-15).
        assert_eq!(
            lead_in(NOTICE_PARAGRAPHS[4]),
            (
                Some("After a trial period:"),
                "your account, your place in this workspace, your apps and your data continue on Forma's free plan; nothing is deleted when a course or test ends. Paid plans may be offered later, and we will not move you to one without asking. Allowances may change; we announce changes in the app."
            )
        );
        assert_eq!(
            lead_in(NOTICE_PARAGRAPHS[5]),
            (
                Some("Your data:"),
                "you can ask at any time, at privacy@getforma.dev, to delete your account and your data. We confirm within 7 days and complete the deletion within 30 days; backup copies expire about a week later. Deletion removes your sign-in account, your name, your apps, your uploads and what you entered in your own apps. Entries you made in apps that other people own stay with those apps, without your name. A minimal security log of account events (identifiers and times, no content) is kept. Your sign-in account (email and password) is held by our identity provider and is deleted with the rest."
            )
        );
        assert_eq!(lead_in(NOTICE_PARAGRAPHS[0]).0, None);
        assert_eq!(
            lead_in(NOTICE_PARAGRAPHS[NOTICE_PARAGRAPHS.len() - 1]),
            (
                None,
                "By checking the box you agree to take part on these terms."
            )
        );
        assert_eq!(lead_in("**unterminated"), (None, "**unterminated"));
    }
}
