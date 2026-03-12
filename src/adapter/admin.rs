//! Chat administration: ban, unban, promote, restrict, member info, invite links, join requests.

use grammers_client::tl;

use super::GrammersAdapter;
use crate::error::ApiError;
use crate::types::*;

impl GrammersAdapter {
    pub(crate) async fn impl_ban_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_peer_user: tl::enums::InputPeer = tl::types::InputPeerUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }
        .into();
        self.client
            .invoke(&tl::functions::channels::EditBanned {
                channel: peer.into(),
                participant: input_peer_user,
                banned_rights: tl::types::ChatBannedRights {
                    view_messages: true,
                    send_messages: true,
                    send_media: true,
                    send_stickers: true,
                    send_gifs: true,
                    send_games: true,
                    send_inline: true,
                    embed_links: true,
                    send_polls: true,
                    change_info: true,
                    invite_users: true,
                    pin_messages: true,
                    manage_topics: true,
                    send_photos: true,
                    send_videos: true,
                    send_roundvideos: true,
                    send_audios: true,
                    send_voices: true,
                    send_docs: true,
                    send_plain: true,
                    until_date: 0,
                }
                .into(),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_unban_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_peer_user: tl::enums::InputPeer = tl::types::InputPeerUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }
        .into();
        self.client
            .invoke(&tl::functions::channels::EditBanned {
                channel: peer.into(),
                participant: input_peer_user,
                banned_rights: tl::types::ChatBannedRights {
                    view_messages: false,
                    send_messages: false,
                    send_media: false,
                    send_stickers: false,
                    send_gifs: false,
                    send_games: false,
                    send_inline: false,
                    embed_links: false,
                    send_polls: false,
                    change_info: false,
                    invite_users: false,
                    pin_messages: false,
                    manage_topics: false,
                    send_photos: false,
                    send_videos: false,
                    send_roundvideos: false,
                    send_audios: false,
                    send_voices: false,
                    send_docs: false,
                    send_plain: false,
                    until_date: 0,
                }
                .into(),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_get_chat_member_count(
        &self,
        chat_id: ChatId,
    ) -> Result<i32, ApiError> {
        let peer = self.resolve(chat_id)?;
        let full = self
            .client
            .invoke(&tl::functions::messages::GetFullChat {
                chat_id: peer.id.bare_id(),
            })
            .await;
        match full {
            Ok(tl::enums::messages::ChatFull::Full(f)) => match f.full_chat {
                tl::enums::ChatFull::Full(cf) => match cf.participants {
                    tl::enums::ChatParticipants::Participants(p) => Ok(p.participants.len() as i32),
                    tl::enums::ChatParticipants::Forbidden(_) => Ok(0),
                },
                tl::enums::ChatFull::ChannelFull(cf) => Ok(cf.participants_count.unwrap_or(0)),
            },
            Err(e) => Err(Self::convert_error(e)),
        }
    }

    pub(crate) async fn impl_leave_chat(&self, chat_id: ChatId) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        if chat_id.0 < 0 {
            self.client
                .invoke(&tl::functions::channels::LeaveChannel {
                    channel: peer.into(),
                })
                .await
                .map_err(Self::convert_error)?;
        } else {
            self.client
                .invoke(&tl::functions::messages::DeleteChatUser {
                    revoke_history: false,
                    chat_id: peer.id.bare_id(),
                    user_id: tl::types::InputUserSelf {}.into(),
                })
                .await
                .map_err(Self::convert_error)?;
        }
        Ok(())
    }

    pub(crate) async fn impl_pin_chat_message(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        silent: bool,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client
            .invoke(&tl::functions::messages::UpdatePinnedMessage {
                silent,
                unpin: false,
                pm_oneside: false,
                peer: peer.into(),
                id: message_id.0,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_unpin_chat_message(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client
            .invoke(&tl::functions::messages::UpdatePinnedMessage {
                silent: true,
                unpin: true,
                pm_oneside: false,
                peer: peer.into(),
                id: message_id.0,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_set_message_reaction(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        emoji: &str,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let reaction: tl::enums::Reaction = if emoji.is_empty() {
            tl::types::ReactionEmpty {}.into()
        } else {
            tl::types::ReactionEmoji {
                emoticon: emoji.to_string(),
            }
            .into()
        };
        self.client
            .invoke(&tl::functions::messages::SendReaction {
                big: false,
                add_to_recent: true,
                peer: peer.into(),
                msg_id: message_id.0,
                reaction: Some(vec![reaction]),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_export_chat_invite_link(
        &self,
        chat_id: ChatId,
    ) -> Result<String, ApiError> {
        let peer = self.resolve(chat_id)?;
        let result = self
            .client
            .invoke(&tl::functions::messages::ExportChatInvite {
                legacy_revoke_permanent: false,
                request_needed: false,
                peer: peer.into(),
                expire_date: None,
                usage_limit: None,
                title: None,
                subscription_pricing: None,
            })
            .await
            .map_err(Self::convert_error)?;
        match result {
            tl::enums::ExportedChatInvite::ChatInviteExported(inv) => Ok(inv.link),
            tl::enums::ExportedChatInvite::ChatInvitePublicJoinRequests => Err(ApiError::Unknown(
                "public join request, no direct link".into(),
            )),
        }
    }

    pub(crate) async fn impl_set_chat_title(
        &self,
        chat_id: ChatId,
        title: &str,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        if chat_id.0 < -1_000_000_000 {
            self.client
                .invoke(&tl::functions::channels::EditTitle {
                    channel: peer.into(),
                    title: title.to_string(),
                })
                .await
                .map_err(Self::convert_error)?;
        } else {
            self.client
                .invoke(&tl::functions::messages::EditChatTitle {
                    chat_id: peer.id.bare_id(),
                    title: title.to_string(),
                })
                .await
                .map_err(Self::convert_error)?;
        }
        Ok(())
    }

    pub(crate) async fn impl_set_chat_description(
        &self,
        chat_id: ChatId,
        description: Option<&str>,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client
            .invoke(&tl::functions::messages::EditChatAbout {
                peer: peer.into(),
                about: description.unwrap_or("").to_string(),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_delete_chat_photo(&self, chat_id: ChatId) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        if chat_id.0 < -1_000_000_000 {
            self.client
                .invoke(&tl::functions::channels::EditPhoto {
                    channel: peer.into(),
                    photo: tl::types::InputChatPhotoEmpty {}.into(),
                })
                .await
                .map_err(Self::convert_error)?;
        } else {
            self.client
                .invoke(&tl::functions::messages::EditChatPhoto {
                    chat_id: peer.id.bare_id(),
                    photo: tl::types::InputChatPhotoEmpty {}.into(),
                })
                .await
                .map_err(Self::convert_error)?;
        }
        Ok(())
    }

    pub(crate) async fn impl_get_chat_administrators(
        &self,
        chat_id: ChatId,
    ) -> Result<Vec<ChatMember>, ApiError> {
        let peer = self.resolve(chat_id)?;
        let result = self
            .client
            .invoke(&tl::functions::channels::GetParticipants {
                channel: peer.into(),
                filter: tl::types::ChannelParticipantsAdmins {}.into(),
                offset: 0,
                limit: 200,
                hash: 0,
            })
            .await
            .map_err(Self::convert_error)?;

        let mut admins = Vec::new();
        if let tl::enums::channels::ChannelParticipants::Participants(p) = result {
            for user in &p.users {
                if let tl::enums::User::User(u) = user {
                    admins.push(ChatMember {
                        user: UserInfo {
                            id: UserId(u.id as u64),
                            first_name: u.first_name.clone().unwrap_or_default(),
                            last_name: u.last_name.clone(),
                            username: u.username.clone(),
                            language_code: None,
                        },
                        status: ChatMemberStatus::Administrator,
                    });
                }
            }
        }
        Ok(admins)
    }

    pub(crate) async fn impl_set_chat_administrator_custom_title(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        custom_title: &str,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }
        .into();
        self.client
            .invoke(&tl::functions::channels::EditAdmin {
                channel: peer.into(),
                user_id: input_user,
                admin_rights: tl::types::ChatAdminRights {
                    change_info: false,
                    post_messages: false,
                    edit_messages: false,
                    delete_messages: false,
                    ban_users: false,
                    invite_users: false,
                    pin_messages: false,
                    add_admins: false,
                    anonymous: false,
                    manage_call: false,
                    other: true,
                    manage_topics: false,
                    post_stories: false,
                    edit_stories: false,
                    delete_stories: false,
                    manage_direct_messages: false,
                }
                .into(),
                rank: custom_title.to_string(),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_approve_chat_join_request(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }
        .into();
        self.client
            .invoke(&tl::functions::messages::HideChatJoinRequest {
                approved: true,
                peer: peer.into(),
                user_id: input_user,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_decline_chat_join_request(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }
        .into();
        self.client
            .invoke(&tl::functions::messages::HideChatJoinRequest {
                approved: false,
                peer: peer.into(),
                user_id: input_user,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_get_user_profile_photos(
        &self,
        user_id: UserId,
        offset: Option<i32>,
        limit: Option<i32>,
    ) -> Result<UserProfilePhotos, ApiError> {
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }
        .into();
        let result = self
            .client
            .invoke(&tl::functions::photos::GetUserPhotos {
                user_id: input_user,
                offset: offset.unwrap_or(0),
                max_id: 0,
                limit: limit.unwrap_or(100),
            })
            .await
            .map_err(Self::convert_error)?;

        match result {
            tl::enums::photos::Photos::Photos(p) => {
                let photos: Vec<String> = p
                    .photos
                    .iter()
                    .filter_map(|photo| {
                        if let tl::enums::Photo::Photo(ph) = photo {
                            Some(ph.id.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(UserProfilePhotos {
                    total_count: photos.len() as i32,
                    photos,
                })
            }
            tl::enums::photos::Photos::Slice(p) => {
                let photos: Vec<String> = p
                    .photos
                    .iter()
                    .filter_map(|photo| {
                        if let tl::enums::Photo::Photo(ph) = photo {
                            Some(ph.id.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(UserProfilePhotos {
                    total_count: p.count,
                    photos,
                })
            }
        }
    }

    pub(crate) async fn impl_answer_pre_checkout_query(
        &self,
        id: String,
        ok: bool,
        error_message: Option<String>,
    ) -> Result<(), ApiError> {
        let query_id: i64 = id
            .parse()
            .map_err(|_| ApiError::Unknown(format!("invalid pre_checkout_query id: {id}")))?;
        self.client
            .invoke(&tl::functions::messages::SetBotPrecheckoutResults {
                success: ok,
                query_id,
                error: error_message,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_restrict_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        permissions: &ChatPermissions,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_peer_user: tl::enums::InputPeer = tl::types::InputPeerUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }
        .into();

        // In ChatBannedRights, `true` means BANNED (i.e. the right is taken away).
        // In our ChatPermissions, `true` means ALLOWED.
        // So we invert: banned = !allowed.
        let no_send = !permissions.can_send_messages.unwrap_or(true);
        let no_media = !permissions.can_send_media_messages.unwrap_or(true);
        let no_polls = !permissions.can_send_polls.unwrap_or(true);
        let no_other = !permissions.can_send_other_messages.unwrap_or(true);
        let no_links = !permissions.can_add_web_page_previews.unwrap_or(true);
        let no_info = !permissions.can_change_info.unwrap_or(true);
        let no_invite = !permissions.can_invite_users.unwrap_or(true);
        let no_pin = !permissions.can_pin_messages.unwrap_or(true);

        self.client
            .invoke(&tl::functions::channels::EditBanned {
                channel: peer.into(),
                participant: input_peer_user,
                banned_rights: tl::types::ChatBannedRights {
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
                    manage_topics: no_pin,
                    send_photos: no_media,
                    send_videos: no_media,
                    send_roundvideos: no_media,
                    send_audios: no_media,
                    send_voices: no_media,
                    send_docs: no_media,
                    send_plain: no_send,
                    until_date: 0,
                }
                .into(),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_promote_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        permissions: &ChatPermissions,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }
        .into();

        self.client
            .invoke(&tl::functions::channels::EditAdmin {
                channel: peer.into(),
                user_id: input_user,
                admin_rights: tl::types::ChatAdminRights {
                    change_info: permissions.can_change_info.unwrap_or(false),
                    post_messages: false,
                    edit_messages: false,
                    delete_messages: false,
                    ban_users: false,
                    invite_users: permissions.can_invite_users.unwrap_or(false),
                    pin_messages: permissions.can_pin_messages.unwrap_or(false),
                    add_admins: false,
                    anonymous: false,
                    manage_call: false,
                    other: false,
                    manage_topics: false,
                    post_stories: false,
                    edit_stories: false,
                    delete_stories: false,
                    manage_direct_messages: false,
                }
                .into(),
                rank: String::new(),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }
}
