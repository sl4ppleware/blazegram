//! Converts raw grammers [`Update`] into blazegram [`IncomingUpdate`] + [`PeerRef`].
//!
//! Isolates all raw TL pattern matching from the main event loop.

use grammers_client::{tl, update::Update};
use grammers_session::types::PeerRef;

use crate::types::*;

/// Convert a raw grammers Update into a blazegram IncomingUpdate + PeerRef.
pub(crate) async fn convert_update(update: &Update) -> Option<(IncomingUpdate, PeerRef)> {
    match update {
        Update::NewMessage(msg) => {
            if msg.outgoing() {
                return None;
            }
            let sender = msg.sender()?;
            let peer_ref = msg.peer_ref().await?;
            let user = user_from_peer(sender);
            let chat_id = ChatId(peer_ref.id.bot_api_dialog_id());
            let message_id = MessageId(msg.id());

            // Check media types via raw TL
            let raw_msg = &msg.raw;
            if let tl::enums::Update::NewMessage(tl::types::UpdateNewMessage { message, .. })
            | tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage {
                message,
                ..
            }) = raw_msg
            {
                if let tl::enums::Message::Message(m) = message {
                    // Photo
                    if let Some(tl::enums::MessageMedia::Photo(photo)) = &m.media {
                        if let Some(tl::enums::Photo::Photo(p)) = &photo.photo {
                            return Some((
                                IncomingUpdate {
                                    chat_id,
                                    user,
                                    message_id: Some(message_id),
                                    kind: UpdateKind::Photo {
                                        file_id: p.id.to_string(),
                                        file_unique_id: p.id.to_string(),
                                        caption: if m.message.is_empty() {
                                            None
                                        } else {
                                            Some(m.message.clone())
                                        },
                                    },
                                },
                                peer_ref,
                            ));
                        }
                    }
                    // Document
                    if let Some(tl::enums::MessageMedia::Document(doc)) = &m.media {
                        if let Some(tl::enums::Document::Document(d)) = &doc.document {
                            let filename = d.attributes.iter().find_map(|a| {
                                if let tl::enums::DocumentAttribute::Filename(f) = a {
                                    Some(f.file_name.clone())
                                } else {
                                    None
                                }
                            });
                            return Some((
                                IncomingUpdate {
                                    chat_id,
                                    user,
                                    message_id: Some(message_id),
                                    kind: UpdateKind::Document {
                                        file_id: d.id.to_string(),
                                        file_unique_id: d.id.to_string(),
                                        filename,
                                        caption: if m.message.is_empty() {
                                            None
                                        } else {
                                            Some(m.message.clone())
                                        },
                                    },
                                },
                                peer_ref,
                            ));
                        }
                    }
                }
            }

            // Text message (default)
            let text = {
                let t = msg.text().to_string();
                if t.is_empty() { None } else { Some(t) }
            };
            Some((
                IncomingUpdate {
                    chat_id,
                    user,
                    message_id: Some(message_id),
                    kind: UpdateKind::Message { text },
                },
                peer_ref,
            ))
        }

        Update::CallbackQuery(query) => {
            let peer_ref = query.peer_ref().await?;
            let sender = query.sender()?;
            let user = user_from_peer(sender);
            let chat_id = ChatId(peer_ref.id.bot_api_dialog_id());

            let query_id = match &query.raw {
                tl::enums::Update::BotCallbackQuery(u) => u.query_id.to_string(),
                tl::enums::Update::InlineBotCallbackQuery(u) => u.query_id.to_string(),
                _ => return None,
            };

            let msg_id = match &query.raw {
                tl::enums::Update::BotCallbackQuery(u) => Some(MessageId(u.msg_id)),
                _ => None,
            };

            let inline_msg_id = match &query.raw {
                tl::enums::Update::InlineBotCallbackQuery(u) => {
                    use grammers_tl_types::Serializable;
                    let mut buf = Vec::new();
                    u.msg_id.serialize(&mut buf);
                    Some(data_encoding::BASE64URL_NOPAD.encode(&buf))
                }
                _ => None,
            };

            let data = {
                let bytes = query.data();
                if bytes.is_empty() {
                    None
                } else {
                    String::from_utf8(bytes.to_vec()).ok()
                }
            };

            Some((
                IncomingUpdate {
                    chat_id,
                    user,
                    message_id: msg_id,
                    kind: UpdateKind::CallbackQuery {
                        id: query_id,
                        data,
                        inline_message_id: inline_msg_id,
                    },
                },
                peer_ref,
            ))
        }

        Update::InlineQuery(query) => {
            tracing::debug!(query_text = %query.text(), "received inline query from grammers");
            let user = match query.sender() {
                Some(s) => user_from_grammers_user(s),
                None => {
                    tracing::debug!("inline query: sender not cached, using raw user_id");
                    UserInfo {
                        id: UserId(query.sender_id().bare_id() as u64),
                        first_name: String::new(),
                        last_name: None,
                        username: None,
                        language_code: None,
                    }
                }
            };
            let (id, q, offset) = match &query.raw {
                tl::enums::Update::BotInlineQuery(u) => {
                    (u.query_id.to_string(), u.query.clone(), u.offset.clone())
                }
                _ => return None,
            };
            let chat_id = ChatId(user.id.0 as i64);
            let mk_update = |user: UserInfo| IncomingUpdate {
                chat_id,
                user,
                message_id: None,
                kind: UpdateKind::InlineQuery {
                    id: id.clone(),
                    query: q.clone(),
                    offset: offset.clone(),
                },
            };
            let peer_ref = match query.sender_ref().await {
                Some(pr) => pr,
                None => {
                    tracing::debug!("inline query: no peer_ref, using dummy");
                    return Some((
                        mk_update(user),
                        PeerRef {
                            id: query.sender_id(),
                            auth: grammers_session::types::PeerAuth::from_hash(0),
                        },
                    ));
                }
            };
            Some((mk_update(user), peer_ref))
        }

        Update::MessageEdited(msg) => {
            if msg.outgoing() {
                return None;
            }
            let sender = msg.sender()?;
            let peer_ref = msg.peer_ref().await?;
            let user = user_from_peer(sender);
            let chat_id = ChatId(peer_ref.id.bot_api_dialog_id());
            let message_id = MessageId(msg.id());
            let text = {
                let t = msg.text().to_string();
                if t.is_empty() { None } else { Some(t) }
            };
            Some((
                IncomingUpdate {
                    chat_id,
                    user,
                    message_id: Some(message_id),
                    kind: UpdateKind::MessageEdited { text },
                },
                peer_ref,
            ))
        }

        Update::InlineSend(inline_send) => {
            let user = match inline_send.sender() {
                Some(s) => user_from_grammers_user(s),
                None => UserInfo {
                    id: UserId(inline_send.sender_id().bare_id() as u64),
                    first_name: String::new(),
                    last_name: None,
                    username: None,
                    language_code: None,
                },
            };
            let result_id = inline_send.result_id().to_string();
            let query = inline_send.text().to_string();
            let inline_message_id = inline_send.message_id().map(|id| {
                use grammers_client::tl;
                match id {
                    tl::enums::InputBotInlineMessageId::Id64(id64) => {
                        let mut bytes = Vec::with_capacity(24);
                        bytes.extend_from_slice(&id64.dc_id.to_le_bytes());
                        bytes.extend_from_slice(&id64.owner_id.to_le_bytes());
                        bytes.extend_from_slice(&id64.id.to_le_bytes());
                        bytes.extend_from_slice(&id64.access_hash.to_le_bytes());
                        data_encoding::BASE64URL_NOPAD.encode(&bytes)
                    }
                    tl::enums::InputBotInlineMessageId::Id(id) => {
                        let mut bytes = Vec::with_capacity(20);
                        bytes.extend_from_slice(&id.dc_id.to_le_bytes());
                        bytes.extend_from_slice(&id.id.to_le_bytes());
                        bytes.extend_from_slice(&id.access_hash.to_le_bytes());
                        data_encoding::BASE64URL_NOPAD.encode(&bytes)
                    }
                }
            });
            let chat_id = ChatId(user.id.0 as i64);
            let peer_ref = match inline_send.sender_ref().await {
                Some(pr) => pr,
                None => PeerRef {
                    id: inline_send.sender_id(),
                    auth: grammers_session::types::PeerAuth::from_hash(0),
                },
            };
            Some((
                IncomingUpdate {
                    chat_id,
                    user,
                    message_id: None,
                    kind: UpdateKind::ChosenInlineResult {
                        result_id,
                        inline_message_id,
                        query,
                    },
                },
                peer_ref,
            ))
        }

        _ => None,
    }
}

pub(crate) fn user_from_peer(peer: &grammers_client::peer::Peer) -> UserInfo {
    use grammers_client::peer::Peer;
    match peer {
        Peer::User(u) => user_from_grammers_user(u),
        _ => UserInfo {
            id: UserId(peer.id().bare_id() as u64),
            first_name: peer.name().unwrap_or_default().to_string(),
            last_name: None,
            username: peer.username().map(String::from),
            language_code: None,
        },
    }
}

pub(crate) fn user_from_grammers_user(u: &grammers_client::peer::User) -> UserInfo {
    UserInfo {
        id: UserId(u.id().bare_id() as u64),
        first_name: u.first_name().unwrap_or_default().to_string(),
        last_name: u.last_name().map(String::from),
        username: u.username().map(String::from),
        language_code: None, // MTProto doesn't provide lang in updates
    }
}
