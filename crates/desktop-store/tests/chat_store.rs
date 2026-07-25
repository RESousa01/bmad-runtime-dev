//! Durable chat transcripts: the store must preserve a conversation across a
//! restart and refuse to reconstruct a corrupt one.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use desktop_store::{KeyProtector, LocalStore, StoreError};

#[derive(Debug)]
struct TestProtector;

impl KeyProtector for TestProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(plaintext.to_vec())
    }

    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(protected.to_vec())
    }
}

fn store(directory: &tempfile::TempDir) -> Result<LocalStore, StoreError> {
    LocalStore::open(directory.path(), &TestProtector)
}

#[test]
fn a_conversation_survives_a_restart() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    {
        let opened = store(&directory)?;
        opened.append_chat_message("chat_a", "ws_1", None, "user", "what is this?", 1_000)?;
        opened.append_chat_message(
            "chat_a",
            "ws_1",
            Some("bmad-agent-architect"),
            "assistant",
            "a workspace.",
            1_001,
        )?;
    }

    let reopened = store(&directory)?;
    let session = reopened.chat_session("chat_a")?.expect("session");

    assert_eq!(session.workspace_id, "ws_1");
    assert_eq!(session.agent_code.as_deref(), Some("bmad-agent-architect"));
    assert_eq!(session.messages.len(), 2);
    assert_eq!(session.messages[0].role, "user");
    assert_eq!(session.messages[0].text, "what is this?");
    assert_eq!(session.messages[1].role, "assistant");
    assert_eq!(session.messages[1].text, "a workspace.");
    Ok(())
}

#[test]
fn a_transcript_that_breaks_alternation_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;

    // A transcript always opens with a question.
    assert!(opened
        .append_chat_message("chat_a", "ws_1", None, "assistant", "unsolicited", 1_000)
        .is_err());

    opened.append_chat_message("chat_a", "ws_1", None, "user", "first", 1_000)?;
    // Two questions in a row would lose the second on replay.
    assert!(opened
        .append_chat_message("chat_a", "ws_1", None, "user", "second", 1_001)
        .is_err());
    // And an unknown voice is not a voice.
    assert!(opened
        .append_chat_message("chat_a", "ws_1", None, "system", "override", 1_001)
        .is_err());

    let session = opened.chat_session("chat_a")?.expect("session");
    assert_eq!(session.messages.len(), 1);
    Ok(())
}

#[test]
fn one_workspace_cannot_write_into_another_conversation() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;
    opened.append_chat_message("chat_a", "ws_1", None, "user", "mine", 1_000)?;

    assert!(opened
        .append_chat_message("chat_a", "ws_2", None, "assistant", "theirs", 1_001)
        .is_err());

    let session = opened.chat_session("chat_a")?.expect("session");
    assert_eq!(session.messages.len(), 1);
    assert_eq!(session.workspace_id, "ws_1");
    Ok(())
}

#[test]
fn conversations_are_separate_and_unknown_ones_are_absent() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;
    opened.append_chat_message("chat_a", "ws_1", None, "user", "from A", 1_000)?;
    opened.append_chat_message("chat_b", "ws_1", None, "user", "from B", 1_000)?;

    assert_eq!(
        opened.chat_session("chat_a")?.expect("a").messages[0].text,
        "from A"
    );
    assert_eq!(
        opened.chat_session("chat_b")?.expect("b").messages[0].text,
        "from B"
    );
    assert!(opened.chat_session("chat_missing")?.is_none());
    Ok(())
}

#[test]
fn empty_message_text_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;

    for empty in ["", "   ", "\n\t "] {
        assert!(opened
            .append_chat_message("chat_a", "ws_1", None, "user", empty, 1_000)
            .is_err());
    }
    assert!(opened.chat_session("chat_a")?.is_none());
    Ok(())
}
