//! Converts raw grammers [`Update`] into blazegram [`IncomingUpdate`] + [`PeerRef`].
//!
//! Isolates all raw TL pattern matching from the main event loop.

use grammers_client::{tl, update::Update};
use grammers_session::types::PeerRef;

use crate::adapter::helpers::PackedFileId;
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

            // Check media / service types via raw TL
            let raw_msg = &msg.raw;
            if let tl::enums::Update::NewMessage(tl::types::UpdateNewMessage { message, .. })
            | tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage {
                message,
                ..
            }) = raw_msg
            {
                // ── Service messages (member join/left, payments) ──
                if let tl::enums::Message::Service(svc) = message {
                    return convert_service_message(svc, chat_id, user, message_id, peer_ref);
                }

                if let tl::enums::Message::Message(m) = message {
                    if let Some(kind) = convert_media(m, &user) {
                        return Some((
                            IncomingUpdate {
                                chat_id,
                                user,
                                message_id: Some(message_id),
                                kind,
                            },
                            peer_ref,
                        ));
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

        // ── Raw updates: pre-checkout queries ──
        Update::Raw(raw) => convert_raw_update(raw).await,

        _ => None,
    }
}

// ─── Media detection from NewMessage ───

/// Try to detect a specific media/content type from a TL message.
/// Returns `None` if the message should be treated as a plain text message.
fn convert_media(m: &tl::types::Message, _user: &UserInfo) -> Option<UpdateKind> {
    let media = m.media.as_ref()?;

    match media {
        // ── Photo ──
        tl::enums::MessageMedia::Photo(photo) => {
            if let Some(tl::enums::Photo::Photo(p)) = &photo.photo {
                let packed = PackedFileId {
                    id: p.id,
                    access_hash: p.access_hash,
                    file_reference: p.file_reference.clone(),
                };
                return Some(UpdateKind::Photo {
                    file_id: packed.encode(),
                    file_unique_id: p.id.to_string(),
                    caption: non_empty(&m.message),
                });
            }
            None
        }

        // ── Document (voice, video, video_note, sticker, generic file) ──
        tl::enums::MessageMedia::Document(doc) => {
            let d = match &doc.document {
                Some(tl::enums::Document::Document(d)) => d,
                _ => return None,
            };

            let attrs = &d.attributes;
            let packed = PackedFileId {
                id: d.id,
                access_hash: d.access_hash,
                file_reference: d.file_reference.clone(),
            };
            let file_id = packed.encode();
            let file_unique_id = d.id.to_string();

            // Check attributes to determine the exact type
            for attr in attrs {
                match attr {
                    // Voice message (Audio with voice=true)
                    tl::enums::DocumentAttribute::Audio(audio) if audio.voice => {
                        return Some(UpdateKind::Voice {
                            file_id,
                            file_unique_id,
                            duration: audio.duration,
                            caption: non_empty(&m.message),
                        });
                    }

                    // Sticker
                    tl::enums::DocumentAttribute::Sticker(_) => {
                        return Some(UpdateKind::Sticker {
                            file_id,
                            file_unique_id,
                        });
                    }

                    // Video or VideoNote (round video)
                    tl::enums::DocumentAttribute::Video(video) => {
                        if video.round_message {
                            return Some(UpdateKind::VideoNote {
                                file_id,
                                file_unique_id,
                                duration: video.duration as i32,
                            });
                        }
                        return Some(UpdateKind::Video {
                            file_id,
                            file_unique_id,
                            caption: non_empty(&m.message),
                        });
                    }

                    _ => {}
                }
            }

            // Generic document (no special attribute matched)
            let filename = attrs.iter().find_map(|a| {
                if let tl::enums::DocumentAttribute::Filename(f) = a {
                    Some(f.file_name.clone())
                } else {
                    None
                }
            });
            Some(UpdateKind::Document {
                file_id,
                file_unique_id,
                filename,
                caption: non_empty(&m.message),
            })
        }

        // ── Contact ──
        tl::enums::MessageMedia::Contact(c) => Some(UpdateKind::ContactReceived {
            contact: Contact {
                phone_number: c.phone_number.clone(),
                first_name: c.first_name.clone(),
                last_name: if c.last_name.is_empty() {
                    None
                } else {
                    Some(c.last_name.clone())
                },
                user_id: if c.user_id == 0 {
                    None
                } else {
                    Some(c.user_id as u64)
                },
                vcard: if c.vcard.is_empty() {
                    None
                } else {
                    Some(c.vcard.clone())
                },
            },
        }),

        // ── Location (static geo point) ──
        tl::enums::MessageMedia::Geo(geo) => {
            if let tl::enums::GeoPoint::Point(p) = &geo.geo {
                return Some(UpdateKind::LocationReceived {
                    latitude: p.lat,
                    longitude: p.long,
                });
            }
            None
        }

        // ── Live location ──
        tl::enums::MessageMedia::GeoLive(geo_live) => {
            if let tl::enums::GeoPoint::Point(p) = &geo_live.geo {
                return Some(UpdateKind::LocationReceived {
                    latitude: p.lat,
                    longitude: p.long,
                });
            }
            None
        }

        _ => None,
    }
}

// ─── Service message handling (member events, payments) ───

fn convert_service_message(
    svc: &tl::types::MessageService,
    chat_id: ChatId,
    user: UserInfo,
    message_id: MessageId,
    peer_ref: PeerRef,
) -> Option<(IncomingUpdate, PeerRef)> {
    let kind = match &svc.action {
        // ── Member joined ──
        tl::enums::MessageAction::ChatAddUser(_)
        | tl::enums::MessageAction::ChatJoinedByLink(_)
        | tl::enums::MessageAction::ChatJoinedByRequest => Some(UpdateKind::ChatMemberJoined),

        // ── Member left ──
        tl::enums::MessageAction::ChatDeleteUser(_) => Some(UpdateKind::ChatMemberLeft),

        // ── Successful payment (received by the bot) ──
        tl::enums::MessageAction::PaymentSentMe(p) => {
            let payload = String::from_utf8(p.payload.clone()).unwrap_or_default();
            Some(UpdateKind::SuccessfulPayment {
                currency: p.currency.clone(),
                total_amount: p.total_amount,
                payload,
            })
        }

        _ => None,
    }?;

    Some((
        IncomingUpdate {
            chat_id,
            user,
            message_id: Some(message_id),
            kind,
        },
        peer_ref,
    ))
}

// ─── Raw update handling (pre-checkout queries, etc.) ───

async fn convert_raw_update(
    raw: &grammers_client::update::Raw,
) -> Option<(IncomingUpdate, PeerRef)> {
    match &raw.raw {
        tl::enums::Update::BotPrecheckoutQuery(pq) => {
            let user_id = UserId(pq.user_id as u64);
            let payload = String::from_utf8(pq.payload.clone()).unwrap_or_default();

            // Build a dummy PeerRef — pre-checkout queries don't have a chat context,
            // we use user_id as the chat_id for routing.
            let chat_id = ChatId(pq.user_id);
            let peer_ref = PeerRef {
                id: grammers_session::types::PeerId::user_unchecked(pq.user_id),
                auth: grammers_session::types::PeerAuth::from_hash(0),
            };

            Some((
                IncomingUpdate {
                    chat_id,
                    user: UserInfo {
                        id: user_id,
                        first_name: String::new(),
                        last_name: None,
                        username: None,
                        language_code: None,
                    },
                    message_id: None,
                    kind: UpdateKind::PreCheckoutQuery {
                        id: pq.query_id.to_string(),
                        currency: pq.currency.clone(),
                        total_amount: pq.total_amount,
                        payload,
                    },
                },
                peer_ref,
            ))
        }
        _ => None,
    }
}

// ─── Helpers ───

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

/// Return `Some(text)` if the string is non-empty, `None` otherwise.
fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

// Testing note: convert_update, convert_media, convert_service_message, and
// user_from_grammers_user all depend on grammers TL types (Update, Message,
// MessageService, User) whose constructors are not public. Building them from
// raw bytes is fragile and ties tests to grammers internals. These paths are
// covered indirectly via TestApp integration tests in testing.rs. The only
// pure function here — `non_empty` — is trivial but tested below for
// completeness.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_returns_none_for_empty() {
        assert_eq!(non_empty(""), None);
    }

    #[test]
    fn non_empty_returns_some_for_text() {
        assert_eq!(non_empty("hello"), Some("hello".to_string()));
    }

    #[test]
    fn non_empty_returns_some_for_whitespace() {
        assert_eq!(non_empty(" "), Some(" ".to_string()));
    }
}
