//! Free helper functions for the grammers adapter.

use grammers_client::tl;

/// Extract a single message ID from a forwarded message response.
pub(super) fn extract_forwarded_msg_id(updates: &tl::enums::Updates) -> Option<i32> {
    match updates {
        tl::enums::Updates::Updates(u) => {
            for update in &u.updates {
                if let tl::enums::Update::NewMessage(tl::types::UpdateNewMessage {
                    message: tl::enums::Message::Message(m),
                    ..
                })
                | tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage {
                    message: tl::enums::Message::Message(m),
                    ..
                }) = update
                {
                    return Some(m.id);
                }
            }
            None
        }
        tl::enums::Updates::Combined(u) => {
            for update in &u.updates {
                if let tl::enums::Update::NewMessage(tl::types::UpdateNewMessage {
                    message: tl::enums::Message::Message(m),
                    ..
                }) = update
                {
                    return Some(m.id);
                }
            }
            None
        }
        tl::enums::Updates::UpdateShortSentMessage(m) => Some(m.id),
        _ => None,
    }
}

/// Extract forum topic ID from a topic creation response.
pub(super) fn extract_forum_topic_id(updates: &tl::enums::Updates) -> Option<i32> {
    match updates {
        tl::enums::Updates::Updates(u) => {
            for update in &u.updates {
                if let tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage {
                    message,
                    ..
                }) = update
                {
                    if let tl::enums::Message::Service(m) = message {
                        return Some(m.id);
                    }
                    if let tl::enums::Message::Message(m) = message {
                        return Some(m.id);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract all message IDs from a batch response.
pub(super) fn extract_all_msg_ids(updates: &tl::enums::Updates) -> Vec<i32> {
    let mut ids = Vec::new();
    match updates {
        tl::enums::Updates::Updates(u) => {
            for update in &u.updates {
                match update {
                    tl::enums::Update::NewMessage(tl::types::UpdateNewMessage {
                        message: tl::enums::Message::Message(m),
                        ..
                    })
                    | tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage {
                        message: tl::enums::Message::Message(m),
                        ..
                    }) => {
                        ids.push(m.id);
                    }
                    _ => {}
                }
            }
        }
        tl::enums::Updates::Combined(u) => {
            for update in &u.updates {
                if let tl::enums::Update::NewMessage(tl::types::UpdateNewMessage {
                    message: tl::enums::Message::Message(m),
                    ..
                }) = update
                {
                    ids.push(m.id);
                }
            }
        }
        tl::enums::Updates::UpdateShortSentMessage(m) => {
            ids.push(m.id);
        }
        _ => {}
    }
    ids
}

/// Generate a pseudo-random i64 for MTProto random_id fields.
pub(super) fn rand_i64() -> i64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::SystemTime;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock before UNIX epoch");
    let cnt = COUNTER.fetch_add(1, Ordering::Relaxed);
    (d.as_nanos() as i64) ^ (cnt as i64 * 6_364_136_223_846_793_005 + 1)
}
