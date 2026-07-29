#![allow(clippy::expect_used, clippy::panic)]

//! Characterization for ENG-18: ordinary chat and the BMAD capability closure
//! must negotiate readiness from disjoint prerequisite sets.
//!
//! An earlier finding (SRC-05) recorded that `crates/desktop-runtime/src/chat.rs`
//! already holds no dependency on the sealed `bmad` domain module: `mod bmad;`
//! and `mod chat;` are declared side by side in `lib.rs`, and `chat.rs` never
//! writes `use crate::bmad` or names a `bmad::` type. These tests lock that
//! shape in so a later change cannot reintroduce the coupling silently — the
//! source scan fails loudly the moment a real (non-prose) reference to `bmad`
//! appears in `chat.rs`, and the behavioural tests fail if `ChatSession` ever
//! grows a required BMAD input.

use std::{fs, path::Path};

use desktop_runtime::{ChatRole, ChatSession, ChatTurn, ContractId, UnixMillis};

fn chat_source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chat.rs");
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("could not read {}: {error}", path.display());
    })
}

/// Strips `//`, `///` and `//!` line comments before scanning for a `bmad`
/// reference, so the module's own prose -- which explains BMAD is an
/// *optional* overlay -- cannot itself trip the check. What must stay absent
/// is a real code path: an import, a type path, or a call.
fn code_only_lines(source: &str) -> Vec<&str> {
    source
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                ""
            } else {
                line
            }
        })
        .collect()
}

#[test]
// Break caught: a later edit wiring chat's send path to a BMAD helper (a
// persona lookup helper living in `bmad`, a library read, a capability
// closure check) for anything beyond the opaque `agent_code: Option<String>`
// it already carries. `chat.rs` has never imported `crate::bmad`; this test
// fails the moment code does, rather than leaving it to be noticed in review.
fn chat_bmad_capability_separation_source_has_no_bmad_module_dependency() {
    let source = chat_source();
    let code_lines = code_only_lines(&source);
    for (index, line) in code_lines.iter().enumerate() {
        assert!(
            !line.contains("crate::bmad") && !line.contains("super::bmad"),
            "chat.rs line {} references the bmad module in code, not prose: {line:?}",
            index + 1,
        );
        // A `bmad::` type path (as opposed to the string literal
        // `"bmad-agent-architect"`, which chat.rs treats as an opaque code) is
        // the same defect through the glob-import door.
        assert!(
            !line.contains(" bmad::") && !line.trim_start().starts_with("bmad::"),
            "chat.rs line {} names a bmad:: type: {line:?}",
            index + 1,
        );
    }
}

#[test]
// Break caught: `ChatSession` requiring a BMAD-sourced argument (a capability
// closure, a library snapshot, a help-run handle) to open, send, or read a
// turn -- which would mean an ordinary conversation could not proceed while
// BMAD Help or the method library was unavailable, exactly the FE-12 defect
// this program already fixed once.
fn chat_bmad_capability_separation_ordinary_send_needs_no_bmad_state() {
    let mut session = ChatSession::open(
        ContractId::new("chat_sep_test").expect("session id"),
        ContractId::new("ws_sep_test").expect("workspace id"),
    );
    assert_eq!(session.agent_code(), None, "no BMAD concept is bound yet");

    session
        .append_user("hello".to_owned(), UnixMillis(1_000))
        .expect("a plain question needs no BMAD state to be accepted");
    let turn = ChatTurn::new("hi there".to_owned()).expect("a plain reply needs no BMAD state");
    session
        .append_assistant(&turn, UnixMillis(1_001))
        .expect("a plain reply needs no BMAD state to close the turn");

    assert_eq!(session.messages().len(), 2);
    assert_eq!(session.messages()[0].role, ChatRole::User);
}

#[test]
// Break caught: the *asymmetry* collapsing into equivalence -- i.e. someone
// "fixing" the previous test by making BMAD's agent binding a prerequisite
// for ordinary send instead of an optional add-on. Binding an agent code must
// remain something that happens to an already-functioning conversation, never
// something a conversation needs in order to function.
fn chat_bmad_capability_separation_bmad_persona_is_additive_not_a_prerequisite() {
    let mut session = ChatSession::open(
        ContractId::new("chat_sep_asym").expect("session id"),
        ContractId::new("ws_sep_asym").expect("workspace id"),
    );

    session
        .append_user("q1".to_owned(), UnixMillis(2_000))
        .expect("user message");
    let reply = ChatTurn::new("a1".to_owned()).expect("turn");
    session
        .append_assistant(&reply, UnixMillis(2_001))
        .expect("assistant message");
    let messages_before = session.messages().len();

    // Binding a BMAD persona is a later, optional step: it changes what a
    // *future* turn wears, not whether the conversation so far was valid.
    session.set_agent_code(Some("bmad-agent-architect".to_owned()));
    assert_eq!(session.agent_code(), Some("bmad-agent-architect"));
    assert_eq!(
        session.messages().len(),
        messages_before,
        "binding a persona must not rewrite or require replaying the transcript"
    );

    session.set_agent_code(None);
    assert_eq!(
        session.agent_code(),
        None,
        "clearing the persona must leave the transcript, and the ability to keep chatting, intact"
    );
}
