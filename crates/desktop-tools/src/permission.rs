//! The consent gate for agent-initiated tool calls (ADR-0009).
//!
//! ADR-0001 bought safety by having no execution primitive at all. ADR-0009
//! withdraws that exclusion, so the safety has to be bought again — here, by
//! asking. Every tool call is evaluated against a rule set before it runs, and
//! anything not explicitly allowed suspends the call and puts the decision in
//! front of the user.
//!
//! Three properties are deliberate and load-bearing:
//!
//! * **Unmatched means ask.** Never allow. A tool or argument nobody wrote a
//!   rule for is exactly the case where a human should look.
//! * **Last match wins.** Rule sets are merged in precedence order and scanned
//!   from the end, so a later source overrides an earlier one without having to
//!   remove anything.
//! * **A denial is an answer, not a crash.** The model is told it was refused
//!   and may reason about it, which is what stops it from silently retrying the
//!   same thing a different way.

use std::collections::BTreeMap;

use serde::Serialize;

/// What a rule says to do with a matching call.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAction {
    Allow,
    Ask,
    Deny,
}

/// One rule: a tool name pattern, an argument pattern, and what to do.
///
/// Both patterns use the same restricted glob — `*` matches any run of
/// characters, everything else is literal. Deliberately not a regex: a rule the
/// user cannot read is a rule they cannot consent to.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PermissionRule {
    pub tool: String,
    pub pattern: String,
    pub action: PermissionAction,
}

impl PermissionRule {
    #[must_use]
    pub fn new(tool: &str, pattern: &str, action: PermissionAction) -> Self {
        Self {
            tool: tool.to_owned(),
            pattern: pattern.to_owned(),
            action,
        }
    }
}

/// Matches a restricted glob: `*` is any run of characters, all else literal.
///
/// Anchored at both ends, so `git status` does not match the rule `git`. A rule
/// that means "and anything after" says so with a trailing `*`.
#[must_use]
pub fn glob_match(pattern: &str, value: &str) -> bool {
    fn walk(pattern: &[u8], value: &[u8]) -> bool {
        match pattern.first() {
            None => value.is_empty(),
            Some(b'*') => {
                // Try every split, shortest first; `*` is greedy only in the
                // sense that some split must work.
                (0..=value.len()).any(|index| walk(&pattern[1..], &value[index..]))
            }
            Some(expected) => match value.first() {
                Some(actual) if actual == expected => walk(&pattern[1..], &value[1..]),
                _ => false,
            },
        }
    }
    walk(pattern.as_bytes(), value.as_bytes())
}

/// Evaluates one call against merged rule sets, last match winning.
///
/// `rulesets` are supplied in precedence order — least specific first — and are
/// scanned as one flattened sequence from the end.
#[must_use]
pub fn evaluate(tool: &str, subject: &str, rulesets: &[&[PermissionRule]]) -> PermissionAction {
    rulesets
        .iter()
        .flat_map(|rules| rules.iter())
        .rfind(|rule| glob_match(&rule.tool, tool) && glob_match(&rule.pattern, subject))
        .map_or(PermissionAction::Ask, |rule| rule.action)
}

/// What the user is being asked about, in terms they can act on.
///
/// `subject` is the thing the rule matched — a path, a command line, a URL. It
/// is what a grant would be *about*, so it is what the prompt must show.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub request_id: String,
    pub tool: String,
    pub subject: String,
    /// Short human sentence naming the effect, e.g. "Run a command in the
    /// workspace". Written by the tool, not the model.
    pub title: String,
    /// The pattern an "always" answer would grant. Deliberately shown, because
    /// the durable half of the decision is the half that outlives the context
    /// the user made it in.
    pub always_pattern: String,
    /// Extra facts the prompt should render — the command, the diff, the path.
    /// Never model prose.
    pub detail: BTreeMap<String, String>,
}

/// The user's answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionReply {
    /// Allow this call only.
    Once,
    /// Allow this and anything matching `always_pattern` for the rest of the
    /// process lifetime. ADR-0009 keeps this in memory on purpose: a grant that
    /// survives a restart is standing authority the user may not remember
    /// giving.
    Always,
    /// Refuse. The model is told, and may reason about it.
    Deny,
}

/// The outcome of asking, from the tool's point of view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PermissionOutcome {
    Allowed,
    /// Refused, carrying the sentence handed back to the model.
    Refused(String),
}

/// Session-scoped grants added by "allow always", newest last.
#[derive(Debug, Default)]
pub struct GrantedRules {
    rules: Vec<PermissionRule>,
}

impl GrantedRules {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an "always" answer. Idempotent: repeating an answer does not
    /// grow the rule set.
    pub fn grant(&mut self, tool: &str, pattern: &str) {
        let rule = PermissionRule::new(tool, pattern, PermissionAction::Allow);
        if !self.rules.contains(&rule) {
            self.rules.push(rule);
        }
    }

    #[must_use]
    pub fn rules(&self) -> &[PermissionRule] {
        &self.rules
    }

    /// Drops every session grant. Called on sign-out and workspace revocation,
    /// so authority never outlives the context it was granted in.
    pub fn clear(&mut self) {
        self.rules.clear();
    }
}

/// The rules in force before the user has answered anything.
///
/// Read-only inspection is allowed outright because it has no effect and asking
/// about it trains the user to click through prompts. Anything that writes,
/// runs, or leaves the machine asks.
#[must_use]
pub fn default_rules() -> Vec<PermissionRule> {
    vec![
        // Everything asks unless a later rule says otherwise.
        PermissionRule::new("*", "*", PermissionAction::Ask),
        // Inspection inside the granted workspace: no effect, no egress.
        PermissionRule::new("read", "*", PermissionAction::Allow),
        PermissionRule::new("list", "*", PermissionAction::Allow),
        PermissionRule::new("glob", "*", PermissionAction::Allow),
        PermissionRule::new("grep", "*", PermissionAction::Allow),
        // …except the files whose whole purpose is to hold a secret. These are
        // readable by a human in the workspace, but a model reading them is a
        // question worth asking.
        PermissionRule::new("read", "*.env", PermissionAction::Ask),
        PermissionRule::new("read", "*.env.*", PermissionAction::Ask),
        PermissionRule::new("read", "*.pem", PermissionAction::Ask),
        PermissionRule::new("read", "*id_rsa*", PermissionAction::Ask),
        PermissionRule::new("read", "*.npmrc", PermissionAction::Ask),
        PermissionRule::new("read", "*.netrc", PermissionAction::Ask),
        // An example file is a template, not a secret.
        PermissionRule::new("read", "*.env.example", PermissionAction::Allow),
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        default_rules, evaluate, glob_match, GrantedRules, PermissionAction, PermissionRule,
    };

    #[test]
    fn a_glob_is_anchored_at_both_ends() {
        assert!(glob_match("git status", "git status"));
        assert!(!glob_match("git", "git status"));
        assert!(glob_match("git *", "git status --short"));
        assert!(glob_match("*", "anything at all"));
        assert!(glob_match("*.env", "packages/app/.env"));
        assert!(!glob_match("*.env", "packages/app/.environment"));
        assert!(glob_match("src/*/mod.rs", "src/thing/mod.rs"));
        assert!(!glob_match("", "x"));
        assert!(glob_match("", ""));
    }

    /// The single most important property in this module: a call nobody wrote a
    /// rule for must reach a human, never run silently.
    #[test]
    fn an_unmatched_call_asks() {
        assert_eq!(
            evaluate("shell", "rm -rf /", &[]),
            PermissionAction::Ask,
            "an empty rule set must not allow anything",
        );
        let unrelated = [PermissionRule::new("read", "*", PermissionAction::Allow)];
        assert_eq!(
            evaluate("shell", "curl example.com", &[&unrelated]),
            PermissionAction::Ask,
        );
    }

    #[test]
    fn the_last_matching_rule_wins() {
        let base = [PermissionRule::new("shell", "*", PermissionAction::Deny)];
        let later = [PermissionRule::new(
            "shell",
            "cargo test*",
            PermissionAction::Allow,
        )];
        assert_eq!(
            evaluate("shell", "cargo test --all", &[&base, &later]),
            PermissionAction::Allow,
        );
        // …and the earlier rule still governs everything the later one misses.
        assert_eq!(
            evaluate("shell", "cargo publish", &[&base, &later]),
            PermissionAction::Deny,
        );
    }

    #[test]
    fn reading_is_allowed_but_secret_bearing_files_still_ask() {
        let defaults = default_rules();
        let rules: &[&[PermissionRule]] = &[&defaults];
        assert_eq!(
            evaluate("read", "src/main.rs", rules),
            PermissionAction::Allow
        );
        assert_eq!(evaluate("read", ".env", rules), PermissionAction::Ask);
        assert_eq!(
            evaluate("read", "apps/web/.env.production", rules),
            PermissionAction::Ask
        );
        assert_eq!(
            evaluate("read", "certs/server.pem", rules),
            PermissionAction::Ask
        );
        // An example file is a template, and the rule ordering has to let that
        // more specific allow win over the `*.env.*` ask above it.
        assert_eq!(
            evaluate("read", ".env.example", rules),
            PermissionAction::Allow
        );
        // Nothing that has an effect is allowed by default.
        for tool in ["shell", "write", "edit", "webfetch"] {
            assert_eq!(
                evaluate(tool, "anything", rules),
                PermissionAction::Ask,
                "{tool} must ask by default",
            );
        }
    }

    #[test]
    fn a_session_grant_applies_and_can_be_withdrawn() {
        let defaults = default_rules();
        let mut granted = GrantedRules::new();
        assert_eq!(
            evaluate("shell", "cargo test --all", &[&defaults, granted.rules()]),
            PermissionAction::Ask,
        );

        granted.grant("shell", "cargo test*");
        assert_eq!(
            evaluate("shell", "cargo test --all", &[&defaults, granted.rules()]),
            PermissionAction::Allow,
        );
        // The grant is scoped to what was asked, not to the tool.
        assert_eq!(
            evaluate("shell", "cargo publish", &[&defaults, granted.rules()]),
            PermissionAction::Ask,
        );

        // Repeating the answer does not grow the set.
        granted.grant("shell", "cargo test*");
        assert_eq!(granted.rules().len(), 1);

        // Sign-out and revocation drop every grant.
        granted.clear();
        assert_eq!(
            evaluate("shell", "cargo test --all", &[&defaults, granted.rules()]),
            PermissionAction::Ask,
        );
    }
}
