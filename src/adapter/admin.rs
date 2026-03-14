//! Chat administration: ban, unban, promote, restrict, member info, invite links, join requests.

use grammers_client::tl;

use super::GrammersAdapter;
use super::helpers::{all_banned_rights, permissions_to_banned_rights};
use crate::error::ApiError;
use crate::types::*;

impl GrammersAdapter {
    pub(crate) async fn impl_ban_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let input_peer_user = self.resolve_user_peer(user_id)?;
        self.client
            .invoke(&tl::functions::channels::EditBanned {
                channel: peer.into(),
                participant: input_peer_user,
                banned_rights: all_banned_rights(true).into(),
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
        let input_peer_user = self.resolve_user_peer(user_id)?;
        self.client
            .invoke(&tl::functions::channels::EditBanned {
                channel: peer.into(),
                participant: input_peer_user,
                banned_rights: all_banned_rights(false).into(),
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
        let input_peer_user = self.resolve_user_peer(user_id)?;

        self.client
            .invoke(&tl::functions::channels::EditBanned {
                channel: peer.into(),
                participant: input_peer_user,
                banned_rights: permissions_to_banned_rights(permissions, true, true).into(),
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
        let input_user = self.resolve_input_user(user_id)?;

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

    pub(crate) async fn impl_get_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<ChatMember, ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_peer_user: tl::enums::InputPeer = tl::types::InputPeerUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }
        .into();
        let result = self
            .client
            .invoke(&tl::functions::channels::GetParticipant {
                channel: peer.into(),
                participant: input_peer_user,
            })
            .await
            .map_err(Self::convert_error)?;

        let tl::enums::channels::ChannelParticipant::Participant(p) = result;
        let status = match &p.participant {
            tl::enums::ChannelParticipant::Participant(_)
            | tl::enums::ChannelParticipant::ParticipantSelf(_) => ChatMemberStatus::Member,
            tl::enums::ChannelParticipant::Creator(_) => ChatMemberStatus::Creator,
            tl::enums::ChannelParticipant::Admin(_) => ChatMemberStatus::Administrator,
            tl::enums::ChannelParticipant::Banned(cp) => {
                let tl::enums::ChatBannedRights::Rights(rights) = &cp.banned_rights;
                if rights.view_messages {
                    ChatMemberStatus::Banned
                } else {
                    ChatMemberStatus::Restricted
                }
            }
            tl::enums::ChannelParticipant::Left(_) => ChatMemberStatus::Left,
        };
        // Find user in participants response
        let user_info = Self::extract_user_from_list(&p.users, user_id.0 as i64);
        Ok(ChatMember {
            user: user_info,
            status,
        })
    }

    fn extract_user_from_list(users: &[tl::enums::User], user_id: i64) -> UserInfo {
        for u in users {
            if let tl::enums::User::User(user) = u {
                if user.id == user_id {
                    return UserInfo {
                        id: UserId(user.id as u64),
                        first_name: user.first_name.clone().unwrap_or_default(),
                        last_name: user.last_name.clone(),
                        username: user.username.clone(),
                        language_code: user.lang_code.clone(),
                    };
                }
            }
        }
        UserInfo {
            id: UserId(user_id as u64),
            first_name: String::new(),
            last_name: None,
            username: None,
            language_code: None,
        }
    }

    pub(crate) async fn impl_get_chat(&self, chat_id: ChatId) -> Result<ChatInfo, ApiError> {
        let peer = self.resolve(chat_id)?;
        let result = self
            .client
            .invoke(&tl::functions::messages::GetFullChat {
                chat_id: peer.id.bare_id(),
            })
            .await;
        match result {
            Ok(tl::enums::messages::ChatFull::Full(full)) => {
                let chat_info = match &full.full_chat {
                    tl::enums::ChatFull::Full(cf) => {
                        let (title, member_count) = full
                            .chats
                            .iter()
                            .find_map(|c| match c {
                                tl::enums::Chat::Chat(ch) if ch.id == cf.id => {
                                    Some((Some(ch.title.clone()), Some(ch.participants_count)))
                                }
                                _ => None,
                            })
                            .unwrap_or((None, None));
                        ChatInfo {
                            id: chat_id,
                            chat_type: ChatType::Group,
                            title,
                            username: None,
                            first_name: None,
                            last_name: None,
                            member_count,
                        }
                    }
                    tl::enums::ChatFull::ChannelFull(cf) => {
                        let (title, username) = full
                            .chats
                            .iter()
                            .find_map(|c| match c {
                                tl::enums::Chat::Channel(ch) if ch.id == cf.id => {
                                    Some((ch.title.clone(), ch.username.clone()))
                                }
                                _ => None,
                            })
                            .unwrap_or_default();
                        ChatInfo {
                            id: chat_id,
                            chat_type: ChatType::Supergroup,
                            title: Some(title),
                            username,
                            first_name: None,
                            last_name: None,
                            member_count: cf.participants_count,
                        }
                    }
                };
                Ok(chat_info)
            }
            Err(e) => Err(Self::convert_error(e)),
        }
    }

    pub(crate) async fn impl_set_chat_permissions(
        &self,
        chat_id: ChatId,
        permissions: &ChatPermissions,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let banned_rights = permissions_to_banned_rights(permissions, true, false);
        let input_peer: tl::enums::InputPeer = if chat_id.0 < 0 {
            tl::types::InputPeerChannel {
                channel_id: peer.id.bare_id(),
                access_hash: peer.auth.hash(),
            }
            .into()
        } else {
            tl::types::InputPeerChat {
                chat_id: peer.id.bare_id(),
            }
            .into()
        };
        self.client
            .invoke(&tl::functions::messages::EditChatDefaultBannedRights {
                peer: input_peer,
                banned_rights: banned_rights.into(),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_set_chat_photo(
        &self,
        chat_id: ChatId,
        photo: FileSource,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let uploaded = match &photo {
            FileSource::LocalPath(path) => self
                .client
                .upload_file(path)
                .await
                .map_err(|e| ApiError::Unknown(format!("upload: {e}")))?,
            FileSource::Bytes { data, filename } => {
                let mut cursor = std::io::Cursor::new(data.clone());
                self.client
                    .upload_stream(&mut cursor, data.len(), filename.clone())
                    .await
                    .map_err(|e| ApiError::Unknown(format!("upload: {e}")))?
            }
            _ => {
                return Err(ApiError::Unknown(
                    "set_chat_photo requires LocalPath or Bytes".into(),
                ));
            }
        };
        let input_photo: tl::enums::InputChatPhoto = tl::types::InputChatUploadedPhoto {
            file: Some(uploaded.raw),
            video: None,
            video_start_ts: None,
            video_emoji_markup: None,
        }
        .into();
        self.client
            .invoke(&tl::functions::channels::EditPhoto {
                channel: peer.into(),
                photo: input_photo,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_unpin_all_chat_messages(
        &self,
        chat_id: ChatId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client
            .invoke(&tl::functions::messages::UnpinAllMessages {
                peer: peer.into(),
                top_msg_id: None,
                saved_peer_id: None,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_create_chat_invite_link(
        &self,
        chat_id: ChatId,
        _name: Option<&str>,
        expire_date: Option<i64>,
        member_limit: Option<i32>,
    ) -> Result<String, ApiError> {
        let peer = self.resolve(chat_id)?;
        let result = self
            .client
            .invoke(&tl::functions::messages::ExportChatInvite {
                legacy_revoke_permanent: false,
                request_needed: false,
                peer: peer.into(),
                expire_date: expire_date.map(|d| d as i32),
                usage_limit: member_limit,
                title: None,
                subscription_pricing: None,
            })
            .await
            .map_err(Self::convert_error)?;
        match result {
            tl::enums::ExportedChatInvite::ChatInviteExported(inv) => Ok(inv.link),
            tl::enums::ExportedChatInvite::ChatInvitePublicJoinRequests => {
                Err(ApiError::Unknown("public join requests — no link".into()))
            }
        }
    }

    pub(crate) async fn impl_revoke_chat_invite_link(
        &self,
        chat_id: ChatId,
        invite_link: &str,
    ) -> Result<ChatInviteLink, ApiError> {
        let peer = self.resolve(chat_id)?;
        let result = self
            .client
            .invoke(&tl::functions::messages::EditExportedChatInvite {
                revoked: true,
                peer: peer.into(),
                link: invite_link.to_string(),
                expire_date: None,
                usage_limit: None,
                request_needed: None,
                title: None,
            })
            .await
            .map_err(Self::convert_error)?;

        let inv = match result {
            tl::enums::messages::ExportedChatInvite::Invite(i) => i.invite,
            tl::enums::messages::ExportedChatInvite::Replaced(r) => r.invite,
        };
        match inv {
            tl::enums::ExportedChatInvite::ChatInviteExported(i) => Ok(ChatInviteLink {
                invite_link: i.link,
                creator: None,
                creates_join_request: i.request_needed,
                is_primary: i.permanent,
                is_revoked: i.revoked,
                name: i.title,
                expire_date: i.expire_date.map(|d| d as i64),
                member_limit: i.usage_limit,
                pending_join_request_count: i.requested,
            }),
            tl::enums::ExportedChatInvite::ChatInvitePublicJoinRequests => {
                Err(ApiError::Unknown("public join requests — no link".into()))
            }
        }
    }

    pub(crate) async fn impl_answer_shipping_query(
        &self,
        shipping_query_id: String,
        ok: bool,
        shipping_options: Option<Vec<ShippingOption>>,
        error_message: Option<String>,
    ) -> Result<(), ApiError> {
        let query_id: i64 = shipping_query_id.parse().map_err(|_| {
            ApiError::Unknown(format!("invalid shipping query id: {shipping_query_id}"))
        })?;
        let options: Vec<tl::enums::ShippingOption> = shipping_options
            .unwrap_or_default()
            .into_iter()
            .map(|opt| {
                tl::types::ShippingOption {
                    id: opt.id,
                    title: opt.title,
                    prices: opt
                        .prices
                        .into_iter()
                        .map(|(label, amount)| tl::types::LabeledPrice { label, amount }.into())
                        .collect(),
                }
                .into()
            })
            .collect();
        self.client
            .invoke(&tl::functions::messages::SetBotShippingResults {
                query_id,
                error: if ok { None } else { error_message },
                shipping_options: if ok { Some(options) } else { None },
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_create_invoice_link(
        &self,
        invoice: Invoice,
    ) -> Result<String, ApiError> {
        let prices: Vec<tl::enums::LabeledPrice> = invoice
            .prices
            .iter()
            .map(|(label, amount)| {
                tl::types::LabeledPrice {
                    label: label.clone(),
                    amount: *amount,
                }
                .into()
            })
            .collect();

        let tl_invoice = tl::types::Invoice {
            test: false,
            name_requested: invoice.need_name,
            phone_requested: invoice.need_phone_number,
            email_requested: invoice.need_email,
            shipping_address_requested: invoice.need_shipping_address,
            flexible: invoice.is_flexible,
            phone_to_provider: false,
            email_to_provider: false,
            recurring: false,
            currency: invoice.currency,
            prices,
            max_tip_amount: None,
            suggested_tip_amounts: None,
            terms_url: None,
            subscription_period: None,
        };

        let input_media: tl::enums::InputMedia = tl::types::InputMediaInvoice {
            title: invoice.title,
            description: invoice.description,
            photo: invoice.photo_url.map(|url| {
                tl::types::InputWebDocument {
                    url,
                    size: 0,
                    mime_type: "image/jpeg".into(),
                    attributes: vec![],
                }
                .into()
            }),
            invoice: tl_invoice.into(),
            payload: invoice.payload.into_bytes(),
            provider: Some(invoice.provider_token.unwrap_or_default()),
            provider_data: tl::types::DataJson { data: "{}".into() }.into(),
            start_param: invoice.start_parameter,
            extended_media: None,
        }
        .into();

        let result = self
            .client
            .invoke(&tl::functions::payments::ExportInvoice {
                invoice_media: input_media,
            })
            .await
            .map_err(Self::convert_error)?;

        let tl::enums::payments::ExportedInvoice::Invoice(exported) = result;
        Ok(exported.url)
    }
}
