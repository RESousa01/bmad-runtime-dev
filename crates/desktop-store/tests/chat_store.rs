//! Durable chat transcripts: the store must preserve a conversation across a
//! restart and refuse to reconstruct a corrupt one.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use desktop_store::{
    ChatTurnAppend, ChatTurnMessage, KeyProtector, LocalStore, StoreError,
    MAX_DURABLE_CHAT_MESSAGES,
};

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

/// One past the `chat_messages.created_at_ms` ceiling the schema itself
/// enforces. A turn carrying it satisfies every rule the store checks in Rust —
/// known role, non-empty text, legal transition, non-decreasing clock — and is
/// then refused by `SQLite` at the `INSERT`. That is the point: it forces a
/// failure *after* the turn's earlier rows have already been written, which is
/// the only kind of mid-turn failure an atomicity claim has to survive.
const BEYOND_SCHEMA_TIME_CEILING: u64 = 9_007_199_254_740_992;

fn roles(session: &desktop_store::StoredChatSession) -> Vec<&str> {
    session
        .messages
        .iter()
        .map(|message| message.role.as_str())
        .collect()
}

fn turn<'a>(session_id: &'a str, messages: &'a [ChatTurnMessage<'a>]) -> ChatTurnAppend<'a> {
    ChatTurnAppend {
        session_id,
        workspace_id: "ws_1",
        agent_code: None,
        messages,
    }
}

/// A turn is the unit of durability. Committing per message let a crash — or
/// one failed append — leave a durable `user` or `tool_call` row with no turn
/// behind it, which the grammar then lets nothing follow and no delete could
/// clear: the conversation was wedged for good.
#[test]
fn a_turn_that_fails_midway_leaves_none_of_itself_behind() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;

    let doomed = [
        ChatTurnMessage {
            role: "user",
            text: "what is in src?",
            created_at_ms: 1_000,
        },
        ChatTurnMessage {
            role: "tool_call",
            text: "glob src/**",
            created_at_ms: 1_001,
        },
        ChatTurnMessage {
            role: "tool_result",
            text: "src/main.rs",
            created_at_ms: 1_002,
        },
        // Legal by every rule the store applies itself; refused by the table.
        // The three rows above are already inserted when this one fails.
        ChatTurnMessage {
            role: "assistant",
            text: "one file.",
            created_at_ms: BEYOND_SCHEMA_TIME_CEILING,
        },
    ];
    assert!(opened
        .append_chat_turn(turn("chat_atomic", &doomed))
        .is_err());

    // Not "the last message is missing" — *nothing* from that turn is durable.
    let after = opened.chat_session("chat_atomic")?;
    assert!(
        after
            .as_ref()
            .is_none_or(|session| session.messages.is_empty()),
        "a failed turn left rows behind: {:?}",
        after.as_ref().map(roles)
    );

    // And the conversation is not wedged: the next turn still opens.
    opened.append_chat_turn(turn(
        "chat_atomic",
        &[
            ChatTurnMessage {
                role: "user",
                text: "what is in src?",
                created_at_ms: 2_000,
            },
            ChatTurnMessage {
                role: "assistant",
                text: "one file.",
                created_at_ms: 2_001,
            },
        ],
    ))?;
    let session = opened.chat_session("chat_atomic")?.expect("session");
    assert_eq!(roles(&session), vec!["user", "assistant"]);
    Ok(())
}

/// The same guarantee against an existing transcript: a turn that fails partway
/// must not extend what is already durable by even one row.
#[test]
fn a_failed_turn_does_not_extend_an_existing_transcript() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;
    opened.append_chat_turn(turn(
        "chat_keep",
        &[
            ChatTurnMessage {
                role: "user",
                text: "first",
                created_at_ms: 1_000,
            },
            ChatTurnMessage {
                role: "assistant",
                text: "answer",
                created_at_ms: 1_001,
            },
        ],
    ))?;

    assert!(opened
        .append_chat_turn(turn(
            "chat_keep",
            &[
                ChatTurnMessage {
                    role: "user",
                    text: "second",
                    created_at_ms: 2_000,
                },
                ChatTurnMessage {
                    role: "tool_call",
                    text: "glob src/**",
                    created_at_ms: 2_001,
                },
                ChatTurnMessage {
                    role: "tool_result",
                    text: "src/main.rs",
                    created_at_ms: 2_002,
                },
                ChatTurnMessage {
                    role: "assistant",
                    text: "one file.",
                    created_at_ms: BEYOND_SCHEMA_TIME_CEILING,
                },
            ],
        ))
        .is_err());

    let session = opened.chat_session("chat_keep")?.expect("session");
    assert_eq!(roles(&session), vec!["user", "assistant"]);
    assert_eq!(session.messages[1].text, "answer");
    Ok(())
}

/// The store and `ChatSession` must agree on what a transcript is. A backwards
/// `created_at_ms` was admitted here and refused there, so the record could be
/// written but never rebuilt: the conversation rendered empty on relaunch while
/// the durable transcript kept growing behind it.
#[test]
fn a_backwards_clock_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;
    opened.append_chat_message("chat_c", "ws_1", None, "user", "asked at 5000", 5_000)?;

    assert!(opened
        .append_chat_message("chat_c", "ws_1", None, "assistant", "answered at 1", 1)
        .is_err());
    // Standing still is not moving backwards; a turn may share a millisecond.
    opened.append_chat_message("chat_c", "ws_1", None, "assistant", "same ms", 5_000)?;

    // And inside one turn, where nothing durable is there to compare against.
    assert!(opened
        .append_chat_turn(turn(
            "chat_c",
            &[
                ChatTurnMessage {
                    role: "user",
                    text: "asked at 9000",
                    created_at_ms: 9_000,
                },
                ChatTurnMessage {
                    role: "assistant",
                    text: "answered at 8999",
                    created_at_ms: 8_999,
                },
            ],
        ))
        .is_err());

    let session = opened.chat_session("chat_c")?.expect("session");
    assert_eq!(roles(&session), vec!["user", "assistant"]);
    assert_eq!(
        session
            .messages
            .iter()
            .map(|message| message.created_at_ms)
            .collect::<Vec<_>>(),
        vec![5_000, 5_000]
    );
    Ok(())
}

/// The store's only delete, and deliberately the narrowest one that frees a
/// conversation an older build wedged: everything after the newest `assistant`,
/// and nothing at or before it.
#[test]
fn repairing_a_dangling_tail_removes_only_the_partial_turn(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;

    // One completed turn, then the wreckage of a turn that never closed.
    opened.append_chat_message("chat_r", "ws_1", None, "user", "first", 1_000)?;
    opened.append_chat_message("chat_r", "ws_1", None, "assistant", "answer", 1_001)?;
    opened.append_chat_message("chat_r", "ws_1", None, "user", "second", 2_000)?;
    opened.append_chat_message("chat_r", "ws_1", None, "tool_call", "glob src/**", 2_001)?;
    opened.append_chat_message("chat_r", "ws_1", None, "tool_result", "src/main.rs", 2_002)?;

    assert_eq!(opened.repair_dangling_chat_tail("chat_r", "ws_1")?, 3);
    let session = opened.chat_session("chat_r")?.expect("session");
    assert_eq!(roles(&session), vec!["user", "assistant"]);
    assert_eq!(session.messages[0].text, "first");
    assert_eq!(session.messages[1].text, "answer");

    // A closed tail is left alone, so the repair is safe to run on every load.
    assert_eq!(opened.repair_dangling_chat_tail("chat_r", "ws_1")?, 0);
    assert_eq!(
        opened
            .chat_session("chat_r")?
            .expect("session")
            .messages
            .len(),
        2
    );

    // And the conversation accepts a new turn again, which is the whole point.
    opened.append_chat_turn(turn(
        "chat_r",
        &[
            ChatTurnMessage {
                role: "user",
                text: "third",
                created_at_ms: 3_000,
            },
            ChatTurnMessage {
                role: "assistant",
                text: "answer again",
                created_at_ms: 3_001,
            },
        ],
    ))?;
    assert_eq!(
        roles(&opened.chat_session("chat_r")?.expect("session")),
        vec!["user", "assistant", "user", "assistant"]
    );
    Ok(())
}

/// A transcript that is nothing but a partial turn empties completely, and an
/// unknown conversation is not an error.
#[test]
fn repairing_an_all_partial_transcript_empties_it() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;
    opened.append_chat_message("chat_p", "ws_1", None, "user", "only question", 1_000)?;

    assert_eq!(opened.repair_dangling_chat_tail("chat_p", "ws_1")?, 1);
    assert!(opened
        .chat_session("chat_p")?
        .expect("the session header survives")
        .messages
        .is_empty());
    assert_eq!(opened.repair_dangling_chat_tail("chat_missing", "ws_1")?, 0);

    opened.append_chat_message("chat_p", "ws_1", None, "user", "asked again", 2_000)?;

    // The store's only delete carries the same ownership guard its writes do.
    assert!(opened
        .repair_dangling_chat_tail("chat_p", "ws_someone_else")
        .is_err());
    assert_eq!(
        opened
            .chat_session("chat_p")?
            .expect("session")
            .messages
            .len(),
        1
    );
    Ok(())
}

/// `put_payload` cannot run inside the write transaction, so a turn's payloads
/// are written before it opens. A rejected turn must not leave one behind for
/// every message it carried.
#[test]
fn a_rejected_turn_writes_no_payload() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;
    let count = || -> Result<u64, StoreError> {
        let connection = rusqlite::Connection::open(opened.database_path())?;
        Ok(connection.query_row(
            "SELECT COUNT(*) FROM payloads WHERE kind = 'chat_message'",
            [],
            |row| row.get(0),
        )?)
    };
    opened.append_chat_message("chat_o", "ws_1", None, "user", "first", 1_000)?;
    let before = count()?;

    // Rejected on the grammar — the exact wedge case: a fresh turn arriving
    // on top of a `user` row an earlier turn left behind.
    assert!(opened
        .append_chat_turn(turn(
            "chat_o",
            &[
                ChatTurnMessage {
                    role: "user",
                    text: "orphan one",
                    created_at_ms: 2_000,
                },
                ChatTurnMessage {
                    role: "assistant",
                    text: "orphan two",
                    created_at_ms: 2_001,
                },
            ],
        ))
        .is_err());

    assert_eq!(
        count()?,
        before,
        "a turn refused on its own grammar must not write payloads first"
    );
    Ok(())
}

/// Writes one turn — `user -> (tool_call -> tool_result)* -> assistant` —
/// and returns how many rows it wrote.
///
/// The number of tool exchanges varies with the index on purpose. Fixed-length
/// turns let a retention that cut purely on row count land on a turn boundary by
/// arithmetic coincidence, and a test that passes by coincidence is not a test:
/// varying the length means a cut that ignores the grammar lands mid-turn and
/// gets caught.
fn varied_turn(
    opened: &LocalStore,
    session_id: &str,
    index: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    let at = 1_000 + (index as u64) * 100;
    let exchanges = index % 3;
    let question = format!("question {index}");
    let answer = format!("answer {index}");
    let bodies: Vec<(String, String)> = (0..exchanges)
        .map(|step| {
            (
                format!("glob src{index}/{step}/**"),
                format!("src{index}/{step}/main.rs"),
            )
        })
        .collect();

    let mut messages = vec![ChatTurnMessage {
        role: "user",
        text: &question,
        created_at_ms: at,
    }];
    for (step, (call, result)) in bodies.iter().enumerate() {
        let step_at = at + 1 + (step as u64) * 2;
        messages.push(ChatTurnMessage {
            role: "tool_call",
            text: call,
            created_at_ms: step_at,
        });
        messages.push(ChatTurnMessage {
            role: "tool_result",
            text: result,
            created_at_ms: step_at + 1,
        });
    }
    messages.push(ChatTurnMessage {
        role: "assistant",
        text: &answer,
        created_at_ms: at + 90,
    });

    opened.append_chat_turn(turn(session_id, &messages))?;
    Ok(messages.len())
}

/// Whether a role sequence is a whole number of complete turns and nothing
/// else: `(user -> (tool_call -> tool_result)* -> assistant)+`.
///
/// This is the property retention must never break. A leading fragment — a
/// `tool_result` with no call, an `assistant` with no question — is exactly what
/// wedges a conversation, because the grammar then permits only a continuation
/// that no fresh turn ever sends.
fn is_whole_turns(roles: &[&str]) -> bool {
    let mut index = 0;
    while index < roles.len() {
        if roles[index] != "user" {
            return false;
        }
        index += 1;
        while roles.get(index) == Some(&"tool_call") {
            if roles.get(index + 1) != Some(&"tool_result") {
                return false;
            }
            index += 2;
        }
        if roles.get(index) != Some(&"assistant") {
            return false;
        }
        index += 1;
    }
    !roles.is_empty()
}

/// Nothing pruned the durable transcript: `MAX_SESSION_MESSAGES` trims a `Vec`
/// in memory, so `chat_messages` and the encrypted payloads behind it grew for
/// the life of the install — for rows the runtime's own prune makes unreachable
/// the moment it rebuilds the conversation.
///
/// A turn is the unit of durability here, so it has to be the unit of deletion:
/// a durable `user` or `tool_call` row with nothing behind it wedges a
/// conversation permanently. This is the guard that retention never leaves one.
#[test]
fn retention_bounds_a_transcript_and_only_ever_drops_whole_turns(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;

    // Enough turns that a comfortable margin of history is past the ceiling,
    // and enough of them that the retained window cannot line up with a turn
    // boundary by accident.
    let mut written = 0_usize;
    let mut turns = 0_usize;
    while written <= MAX_DURABLE_CHAT_MESSAGES + 60 {
        written += varied_turn(&opened, "chat_retained", turns)?;
        turns += 1;
    }

    let session = opened
        .chat_session("chat_retained")?
        .expect("the conversation survives");
    assert!(
        session.messages.len() <= MAX_DURABLE_CHAT_MESSAGES,
        "the durable transcript is unbounded: {} of {written} rows retained",
        session.messages.len()
    );
    assert!(!session.messages.is_empty());

    let seen = roles(&session);
    assert!(
        is_whole_turns(&seen),
        "retention left a turn fragment: {seen:?}"
    );

    // The newest turns are the ones kept, not an arbitrary window.
    assert_eq!(
        session.messages.last().expect("newest").text,
        format!("answer {}", turns - 1)
    );

    // And the conversation is not wedged: it still accepts the next turn.
    varied_turn(&opened, "chat_retained", turns)?;
    let after = opened.chat_session("chat_retained")?.expect("session");
    assert_eq!(
        after.messages.last().expect("newest").text,
        format!("answer {turns}")
    );
    assert!(is_whole_turns(&roles(&after)));
    Ok(())
}

/// The ceiling yields, the turn does not. Half a turn costs the user the whole
/// conversation, so a single turn longer than the entire budget is kept intact
/// and the bound is allowed to be exceeded instead.
#[test]
fn a_turn_longer_than_the_whole_budget_is_kept_whole() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;

    let exchanges = MAX_DURABLE_CHAT_MESSAGES / 2 + 1;
    let bodies: Vec<(String, String)> = (0..exchanges)
        .map(|index| (format!("glob src{index}/**"), format!("src{index}/main.rs")))
        .collect();
    let mut messages = vec![ChatTurnMessage {
        role: "user",
        text: "trace every file",
        created_at_ms: 1_000,
    }];
    for (index, (call, result)) in bodies.iter().enumerate() {
        let at = 1_001 + (index as u64) * 2;
        messages.push(ChatTurnMessage {
            role: "tool_call",
            text: call,
            created_at_ms: at,
        });
        messages.push(ChatTurnMessage {
            role: "tool_result",
            text: result,
            created_at_ms: at + 1,
        });
    }
    messages.push(ChatTurnMessage {
        role: "assistant",
        text: "traced.",
        created_at_ms: 9_000_000,
    });
    let expected = messages.len();
    assert!(expected > MAX_DURABLE_CHAT_MESSAGES);
    opened.append_chat_turn(turn("chat_long", &messages))?;

    let session = opened.chat_session("chat_long")?.expect("session");
    assert_eq!(
        session.messages.len(),
        expected,
        "an oversized turn was cut instead of kept whole"
    );
    assert_eq!(roles(&session).first().copied(), Some("user"));
    assert_eq!(roles(&session).last().copied(), Some("assistant"));
    Ok(())
}

/// Dropping the rows without dropping what they point at would leave the
/// encrypted bodies — the actual bulk — behind, and boot verification would keep
/// paying to decrypt them.
#[test]
fn retention_releases_the_payloads_of_the_turns_it_drops() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;
    let registered = || -> Result<u64, StoreError> {
        let connection = rusqlite::Connection::open(opened.database_path())?;
        Ok(connection.query_row(
            "SELECT COUNT(*) FROM payloads WHERE kind = 'chat_message'",
            [],
            |row| row.get(0),
        )?)
    };

    let mut written = 0_usize;
    let mut turns = 0_usize;
    while written <= MAX_DURABLE_CHAT_MESSAGES + 60 {
        written += varied_turn(&opened, "chat_payloads", turns)?;
        turns += 1;
    }

    let retained = opened
        .chat_session("chat_payloads")?
        .expect("session")
        .messages
        .len();
    let written = u64::try_from(written)?;
    let held = registered()?;
    assert!(
        held < written,
        "every message body is still registered: {held} of {written}"
    );
    // One registration per retained message body. Every body in this transcript
    // is distinct, so content addressing does not fold any of them together.
    assert_eq!(held, u64::try_from(retained)?);
    Ok(())
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
fn a_tool_using_turn_is_admitted_but_its_grammar_is_still_strict(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let opened = store(&directory)?;

    // user -> (tool_call -> tool_result)* -> assistant
    opened.append_chat_message("chat_t", "ws_1", None, "user", "what is in src?", 1_000)?;
    opened.append_chat_message("chat_t", "ws_1", None, "tool_call", "glob src/**", 1_001)?;

    // A second call before the first is answered would strand it.
    assert!(opened
        .append_chat_message("chat_t", "ws_1", None, "tool_call", "glob docs/**", 1_002)
        .is_err());
    // So would closing the turn with a call outstanding.
    assert!(opened
        .append_chat_message("chat_t", "ws_1", None, "assistant", "done", 1_002)
        .is_err());
    // And a question cannot interrupt one.
    assert!(opened
        .append_chat_message("chat_t", "ws_1", None, "user", "actually never mind", 1_002)
        .is_err());

    opened.append_chat_message("chat_t", "ws_1", None, "tool_result", "src/main.rs", 1_003)?;
    // A result with nothing outstanding is unrequested and refused.
    assert!(opened
        .append_chat_message("chat_t", "ws_1", None, "tool_result", "src/other.rs", 1_004)
        .is_err());

    // Another exchange, then the turn closes.
    opened.append_chat_message(
        "chat_t",
        "ws_1",
        None,
        "tool_call",
        "read src/main.rs",
        1_005,
    )?;
    opened.append_chat_message("chat_t", "ws_1", None, "tool_result", "fn main() {}", 1_006)?;
    opened.append_chat_message("chat_t", "ws_1", None, "assistant", "It has a main.", 1_007)?;

    let session = opened.chat_session("chat_t")?.expect("session");
    let roles: Vec<&str> = session
        .messages
        .iter()
        .map(|message| message.role.as_str())
        .collect();
    assert_eq!(
        roles,
        vec![
            "user",
            "tool_call",
            "tool_result",
            "tool_call",
            "tool_result",
            "assistant"
        ]
    );
    Ok(())
}

#[test]
fn an_existing_transcript_survives_the_v16_rebuild() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    {
        let opened = store(&directory)?;
        opened.append_chat_message("chat_m", "ws_1", None, "user", "before", 1_000)?;
        opened.append_chat_message("chat_m", "ws_1", None, "assistant", "after", 1_001)?;
    }
    // v16 rebuilds chat_messages to widen a CHECK constraint. Reopening walks
    // the ladder, so this asserts the copy preserved order and content rather
    // than quietly emptying the table.
    let reopened = store(&directory)?;
    let session = reopened.chat_session("chat_m")?.expect("session");
    let seen: Vec<(&str, &str)> = session
        .messages
        .iter()
        .map(|message| (message.role.as_str(), message.text.as_str()))
        .collect();
    assert_eq!(seen, vec![("user", "before"), ("assistant", "after")]);
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
