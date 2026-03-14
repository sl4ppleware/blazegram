//! Free helper functions for the grammers adapter.

use grammers_client::tl;

use crate::error::ApiError;
use crate::types::*;
use super::GrammersAdapter;

// ─── User / peer resolution ───

impl GrammersAdapter {
    /// Resolve a `UserId` to an `InputPeer` for admin operations.
    pub(super) fn resolve_user_peer(&self, user_id: UserId) -> Result<tl::enums::InputPeer, ApiError> {
        let resolved = self.resolve(ChatId(user_id.0 as i64))?;
        Ok(tl::types::InputPeerUser {
            user_id: resolved.id.bare_id(),
            access_hash: resolved.auth.hash(),
        }
        .into())
    }

    /// Resolve a `UserId` to an `InputUser` for admin operations.
    pub(super) fn resolve_input_user(&self, user_id: UserId) -> Result<tl::enums::InputUser, ApiError> {
        let resolved = self.resolve(ChatId(user_id.0 as i64))?;
        Ok(tl::types::InputUser {
            user_id: resolved.id.bare_id(),
            access_hash: resolved.auth.hash(),
        }
        .into())
    }
}

// ─── ChatBannedRights construction ───

/// Build `ChatBannedRights` with every field set to the same value.
///
/// `banned = true` → ban everything; `banned = false` → lift all restrictions.
pub(super) fn all_banned_rights(banned: bool) -> tl::types::ChatBannedRights {
    tl::types::ChatBannedRights {
        view_messages: banned,
        send_messages: banned,
        send_media: banned,
        send_stickers: banned,
        send_gifs: banned,
        send_games: banned,
        send_inline: banned,
        embed_links: banned,
        send_polls: banned,
        change_info: banned,
        invite_users: banned,
        pin_messages: banned,
        manage_topics: banned,
        send_photos: banned,
        send_videos: banned,
        send_roundvideos: banned,
        send_audios: banned,
        send_voices: banned,
        send_docs: banned,
        send_plain: banned,
        until_date: 0,
    }
}

/// Convert our `ChatPermissions` to `ChatBannedRights`.
///
/// `None` permissions fall back to `default_allowed`.
/// In banned rights, `true` means the right is *taken away* — so we invert.
pub(super) fn permissions_to_banned_rights(
    perms: &ChatPermissions,
    default_allowed: bool,
    manage_topics_from_pin: bool,
) -> tl::types::ChatBannedRights {
    let no_send = !perms.can_send_messages.unwrap_or(default_allowed);
    let no_media = !perms.can_send_media_messages.unwrap_or(default_allowed);
    let no_polls = !perms.can_send_polls.unwrap_or(default_allowed);
    let no_other = !perms.can_send_other_messages.unwrap_or(default_allowed);
    let no_links = !perms.can_add_web_page_previews.unwrap_or(default_allowed);
    let no_info = !perms.can_change_info.unwrap_or(default_allowed);
    let no_invite = !perms.can_invite_users.unwrap_or(default_allowed);
    let no_pin = !perms.can_pin_messages.unwrap_or(default_allowed);

    tl::types::ChatBannedRights {
        view_messages: false,
        send_messages: no_send,
        send_media: no_media,
        send_stickers: no_other,
        send_gifs: no_other,
        send_games: no_other,
        send_inline: no_other,
        embed_links: no_links,
        send_polls: no_polls,
        change_info: no_info,
        invite_users: no_invite,
        pin_messages: no_pin,
        manage_topics: if manage_topics_from_pin { no_pin } else { false },
        send_photos: no_media,
        send_videos: no_media,
        send_roundvideos: no_media,
        send_audios: no_media,
        send_voices: no_media,
        send_docs: no_media,
        send_plain: no_send,
        until_date: 0,
    }
}
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
