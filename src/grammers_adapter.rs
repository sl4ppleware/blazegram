//! BotApi implementation backed by grammers (pure Rust MTProto).
//!
//! Speaks MTProto directly to Telegram DC — no HTTP, no Bot API proxy.
//! Single binary, zero external processes.

use async_trait::async_trait;
use dashmap::DashMap;
use grammers_client::{
    Client, InvocationError,
    message::{Button, InputMessage, Key, ReplyMarkup},
    tl,
};
use grammers_session::types::PeerRef;
use std::sync::Arc;

use crate::bot_api::{BotApi, SendOptions};
use crate::error::ApiError;
use crate::keyboard::{ButtonAction, InlineKeyboard};
use crate::types::*;

/// Default Telegram Desktop API credentials (public, from TDesktop source).
pub const DEFAULT_API_ID: i32 = 2040;
pub const DEFAULT_API_HASH: &str = "b18441a1ff607e10a989891a5462e627";

/// A [`BotApi`] implementation using grammers MTProto client.
#[derive(Clone)]
pub struct GrammersAdapter {
    client: Client,
    /// Cached PeerRefs, keyed by ChatId.0
    peers: Arc<DashMap<i64, PeerRef>>,
}

impl GrammersAdapter {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            peers: Arc::new(DashMap::new()),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Cache a PeerRef for later API calls. Must be called for every incoming update.
    pub fn cache_peer(&self, peer: PeerRef) {
        self.peers.insert(peer.id.bot_api_dialog_id(), peer);
    }

    pub fn get_peer(&self, chat_id: ChatId) -> Option<PeerRef> {
        self.peers.get(&chat_id.0).map(|r| *r)
    }

    /// Serialize the peer cache for persistence.
    pub fn export_peers(&self) -> Vec<(i64, i64, i64)> {
        self.peers.iter().map(|entry| {
            let id = *entry.key();
            let peer = entry.value();
            let bare_id = peer.id.bare_id();
            let access_hash = peer.auth.hash();
            (id, bare_id, access_hash)
        }).collect()
    }

    /// Restore the peer cache from persisted data.
    pub fn import_peers(&self, data: &[(i64, i64, i64)]) {
        use grammers_session::types::{PeerId, PeerAuth};
        for &(dialog_id, bare_id, access_hash) in data {
            let peer_id = if dialog_id > 0 {
                PeerId::user(bare_id)
            } else if dialog_id < -1_000_000_000 {
                PeerId::channel(bare_id)
            } else {
                PeerId::chat(bare_id)
            };
            if let Some(pid) = peer_id {
                let peer_ref = PeerRef {
                    id: pid,
                    auth: PeerAuth::from_hash(access_hash),
                };
                self.peers.insert(dialog_id, peer_ref);
            }
        }
    }

    fn resolve(&self, chat_id: ChatId) -> Result<PeerRef, ApiError> {
        self.get_peer(chat_id).ok_or(ApiError::ChatNotFound)
    }

    // ── Error conversion ──

    fn convert_error(e: InvocationError) -> ApiError {
        match &e {
            InvocationError::Rpc(rpc) => {
                let name = rpc.name.as_str();
                match name {
                    "MESSAGE_ID_INVALID" | "MESSAGE_DELETE_FORBIDDEN" => ApiError::MessageNotFound,
                    "MESSAGE_NOT_MODIFIED" => ApiError::MessageNotModified,
                    "FLOOD_WAIT" | "SLOWMODE_WAIT" => {
                        ApiError::TooManyRequests { retry_after: rpc.value.unwrap_or(1) }
                    }
                    "ENTITY_BOUNDS_INVALID" | "ENTITY_TEXTURL_INVALID" => ApiError::EntityBoundsInvalid,
                    "USER_IS_BLOCKED" | "BOT_BLOCKED" => ApiError::BotBlocked,
                    "PEER_ID_INVALID" | "CHAT_WRITE_FORBIDDEN" | "CHANNEL_PRIVATE"
                    | "INPUT_USER_DEACTIVATED" | "USER_DEACTIVATED" => ApiError::ChatNotFound,
                    _ if rpc.code == 403 => ApiError::Forbidden(name.to_string()),
                    _ => ApiError::Unknown(format!("{name} (code {})", rpc.code)),
                }
            }
            InvocationError::Io(_) | InvocationError::Dropped => {
                ApiError::Network(e.to_string())
            }
            _ => ApiError::Unknown(e.to_string()),
        }
    }

    // ── Keyboard conversion ──

    fn to_inline_markup(kb: &InlineKeyboard) -> ReplyMarkup {
        let rows: Vec<Vec<Button>> = kb.rows.iter().map(|row| {
            row.iter().map(|btn| match &btn.action {
                ButtonAction::Callback(data) => Button::data(&btn.text, data.as_bytes()),
                ButtonAction::Url(url) => Button::url(&btn.text, url),
                ButtonAction::WebApp(url) => Button::webview(&btn.text, url),
                ButtonAction::SwitchInline { query, current_chat } => {
                    if *current_chat { Button::switch(&btn.text, query) }
                    else { Button::switch_elsewhere(&btn.text, query) }
                }
            }).collect()
        }).collect();
        ReplyMarkup::from_buttons(&rows)
    }

    fn to_reply_markup(action: &crate::screen::ReplyKeyboardAction) -> ReplyMarkup {
        match action {
            crate::screen::ReplyKeyboardAction::Show { rows, resize, one_time, .. } => {
                let key_rows: Vec<Vec<Key>> = rows.iter().map(|row| {
                    row.iter().map(|btn| {
                        if btn.request_contact { Key::request_phone(&btn.text) }
                        else if btn.request_location { Key::request_geo(&btn.text) }
                        else { Key::text(&btn.text) }
                    }).collect()
                }).collect();
                let mut m = ReplyMarkup::from_keys(&key_rows);
                if *resize { m = m.fit_size(); }
                if *one_time { m = m.single_use(); }
                m
            }
            crate::screen::ReplyKeyboardAction::Remove => ReplyMarkup::hide(),
        }
    }

    // ── Message building ──

    fn text_msg(text: &str, parse_mode: ParseMode, link_preview: bool) -> InputMessage {
        let msg = match parse_mode {
            ParseMode::Html => InputMessage::new().html(text),
            ParseMode::MarkdownV2 => InputMessage::new().markdown(text),
            ParseMode::None => InputMessage::new().text(text),
        };
        msg.link_preview(link_preview)
    }

    fn with_markup(
        msg: InputMessage,
        inline: Option<&InlineKeyboard>,
        reply: Option<&crate::screen::ReplyKeyboardAction>,
    ) -> InputMessage {
        if let Some(kb) = inline {
            msg.reply_markup(Self::to_inline_markup(kb))
        } else if let Some(rk) = reply {
            msg.reply_markup(Self::to_reply_markup(rk))
        } else {
            msg
        }
    }

    async fn build_input(
        &self,
        content: &MessageContent,
        reply_kb: Option<&crate::screen::ReplyKeyboardAction>,
    ) -> Result<InputMessage, ApiError> {
        match content {
            MessageContent::Text { text, parse_mode, keyboard, link_preview } => {
                let lp = matches!(link_preview, LinkPreview::Enabled);
                let msg = Self::text_msg(text, *parse_mode, lp);
                Ok(Self::with_markup(msg, keyboard.as_ref(), reply_kb))
            }
            MessageContent::Photo { source, caption, parse_mode, keyboard, .. } => {
                let msg = self.media_msg(caption.as_deref().unwrap_or(""), *parse_mode, source, true).await?;
                Ok(Self::with_markup(msg, keyboard.as_ref(), reply_kb))
            }
            MessageContent::Video { source, caption, parse_mode, keyboard, .. }
            | MessageContent::Animation { source, caption, parse_mode, keyboard, .. }
            | MessageContent::Document { source, caption, parse_mode, keyboard, .. } => {
                let msg = self.media_msg(caption.as_deref().unwrap_or(""), *parse_mode, source, false).await?;
                Ok(Self::with_markup(msg, keyboard.as_ref(), reply_kb))
            }
            MessageContent::Sticker { source } => {
                let msg = self.media_msg("", ParseMode::None, source, false).await?;
                Ok(msg)
            }
            MessageContent::Location { latitude, longitude, keyboard } => {
                let media: tl::enums::InputMedia = tl::types::InputMediaGeoPoint {
                    geo_point: tl::types::InputGeoPoint {
                        lat: *latitude, long: *longitude, accuracy_radius: None,
                    }.into(),
                }.into();
                let msg = InputMessage::new().media(media);
                Ok(Self::with_markup(msg, keyboard.as_ref(), reply_kb))
            }
        }
    }

    async fn media_msg(
        &self,
        caption: &str,
        parse_mode: ParseMode,
        source: &FileSource,
        is_photo: bool,
    ) -> Result<InputMessage, ApiError> {
        let base = Self::text_msg(caption, parse_mode, false);
        match source {
            FileSource::Url(url) => Ok(if is_photo { base.photo_url(url) } else { base.document_url(url) }),
            FileSource::LocalPath(path) => {
                let uploaded = self.client.upload_file(path).await
                    .map_err(|e| ApiError::Unknown(format!("upload: {e}")))?;
                Ok(if is_photo { base.photo(uploaded) } else { base.document(uploaded) })
            }
            FileSource::Bytes { data, filename } => {
                let mut cursor = std::io::Cursor::new(data.clone());
                let uploaded = self.client.upload_stream(&mut cursor, data.len(), filename.clone()).await
                    .map_err(|e| ApiError::Unknown(format!("upload: {e}")))?;
                Ok(if is_photo { base.photo(uploaded) } else { base.document(uploaded) })
            }
            FileSource::FileId(file_id) => {
                // MTProto doesn't use Bot API file_ids directly. To re-send cached files,
                // use FileIdCache to map content hashes to file sources, or store the
                // original URL/path instead of the file_id.
                Err(ApiError::Unknown(format!(
                    "FileId '{}...' cannot be sent via MTProto. Use FileSource::Url or \
                     FileSource::LocalPath instead, or cache the Uploaded file.",
                    &file_id[..file_id.len().min(20)]
                )))
            }
        }
    }

    // ── Public helpers for Ctx ──

    /// Public wrapper for keyboard conversion (used by ctx.rs for inline edits).
    pub fn to_inline_markup_pub(kb: &InlineKeyboard) -> ReplyMarkup {
        Self::to_inline_markup(kb)
    }

    /// Public wrapper for error conversion.
    pub fn convert_error_pub(e: InvocationError) -> ApiError {
        Self::convert_error(e)
    }

    /// Resolve a ChatId from the shared peer cache.
    pub fn resolve_from_cache(peers: &DashMap<i64, PeerRef>, chat_id: ChatId) -> Option<PeerRef> {
        peers.get(&chat_id.0).map(|r| *r)
    }

    /// Get a reference to the peer cache (for sharing with Ctx).
    pub fn peer_cache(&self) -> Arc<DashMap<i64, PeerRef>> {
        Arc::clone(&self.peers)
    }

    fn tl_action(action: ChatAction) -> tl::enums::SendMessageAction {
        match action {
            ChatAction::Typing => tl::types::SendMessageTypingAction {}.into(),
            ChatAction::UploadPhoto => tl::types::SendMessageUploadPhotoAction { progress: 0 }.into(),
            ChatAction::UploadVideo => tl::types::SendMessageUploadVideoAction { progress: 0 }.into(),
            ChatAction::UploadDocument => tl::types::SendMessageUploadDocumentAction { progress: 0 }.into(),
            ChatAction::FindLocation => tl::types::SendMessageGeoLocationAction {}.into(),
            ChatAction::RecordVoice => tl::types::SendMessageRecordAudioAction {}.into(),
            ChatAction::RecordVideo => tl::types::SendMessageRecordVideoAction {}.into(),
        }
    }
}

#[async_trait]
impl BotApi for GrammersAdapter {
    async fn send_message(
        &self, chat_id: ChatId, content: MessageContent, opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        let peer = self.resolve(chat_id)?;
        let mut msg = self.build_input(&content, opts.reply_keyboard.as_ref()).await?;
        if let Some(reply_id) = opts.reply_to {
            msg = msg.reply_to(Some(reply_id.0));
        }
        let sent = self.client.send_message(peer, msg).await.map_err(Self::convert_error)?;
        Ok(SentMessage { message_id: MessageId(sent.id()), chat_id })
    }

    async fn edit_message_text(
        &self, chat_id: ChatId, message_id: MessageId, text: String,
        parse_mode: ParseMode, keyboard: Option<InlineKeyboard>, link_preview: bool,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let pm = parse_mode;
        let msg = Self::text_msg(&text, pm, link_preview);
        let msg = if let Some(kb) = &keyboard { msg.reply_markup(Self::to_inline_markup(kb)) } else { msg };
        self.client.edit_message(peer, message_id.0, msg).await.map_err(Self::convert_error)
    }

    async fn edit_message_caption(
        &self, chat_id: ChatId, message_id: MessageId, caption: Option<String>,
        parse_mode: ParseMode, keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let cap = caption.as_deref().unwrap_or("");
        let pm = parse_mode;
        let msg = Self::text_msg(cap, pm, false);
        let msg = if let Some(kb) = &keyboard { msg.reply_markup(Self::to_inline_markup(kb)) } else { msg };
        self.client.edit_message(peer, message_id.0, msg).await.map_err(Self::convert_error)
    }

    async fn edit_message_media(
        &self, chat_id: ChatId, message_id: MessageId, content: MessageContent,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let msg = self.build_input(&content, None).await?;
        let msg = if let Some(kb) = &keyboard { msg.reply_markup(Self::to_inline_markup(kb)) } else { msg };
        self.client.edit_message(peer, message_id.0, msg).await.map_err(Self::convert_error)
    }

    async fn edit_message_keyboard(
        &self, chat_id: ChatId, message_id: MessageId, keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let markup = keyboard.as_ref().map(Self::to_inline_markup);
        self.client.invoke(&tl::functions::messages::EditMessage {
            no_webpage: false, invert_media: false,
            peer: peer.into(), id: message_id.0,
            message: None, media: None,
            reply_markup: markup.map(|m| m.raw),
            entities: None, schedule_date: None,
            schedule_repeat_period: None, quick_reply_shortcut_id: None,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn delete_messages(
        &self, chat_id: ChatId, message_ids: Vec<MessageId>,
    ) -> Result<(), ApiError> {
        if message_ids.is_empty() { return Ok(()); }
        let peer = self.resolve(chat_id)?;
        let ids: Vec<i32> = message_ids.iter().map(|m| m.0).collect();
        for chunk in ids.chunks(100) {
            match self.client.delete_messages(peer, chunk).await {
                Ok(_) => {}
                Err(e) => {
                    let err = Self::convert_error(e);
                    if !matches!(err, ApiError::MessageNotFound) { return Err(err); }
                }
            }
        }
        Ok(())
    }

    async fn answer_callback_query(
        &self, id: String, text: Option<String>, show_alert: bool,
    ) -> Result<(), ApiError> {
        let query_id: i64 = id.parse()
            .map_err(|_| ApiError::Unknown(format!("invalid callback query id: {id}")))?;
        self.client.invoke(&tl::functions::messages::SetBotCallbackAnswer {
            alert: show_alert, query_id, message: text, url: None, cache_time: 0,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn send_chat_action(
        &self, chat_id: ChatId, action: ChatAction,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client.invoke(&tl::functions::messages::SetTyping {
            peer: peer.into(), top_msg_id: None, action: Self::tl_action(action),
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn answer_inline_query(
        &self, query_id: String, results: Vec<InlineQueryResult>,
        next_offset: Option<String>, cache_time: Option<i32>, is_personal: bool,
    ) -> Result<(), ApiError> {
        use crate::types::InlineResultKind;
        let qid: i64 = query_id.parse()
            .map_err(|_| ApiError::Unknown(format!("invalid inline query id: {query_id}")))?;

        let tl_results: Vec<tl::enums::InputBotInlineResult> = results.into_iter().map(|r| {
            let markup = r.keyboard.as_ref().map(|kb| Self::to_inline_markup(kb).raw);
            let (text, entities) = if let Some(txt) = &r.message_text {
                match r.parse_mode {
                    ParseMode::Html => {
                        let (plain, ents) = grammers_client::parsers::parse_html_message(txt);
                        (plain, if ents.is_empty() { None } else { Some(ents) })
                    }
                    _ => (txt.clone(), None),
                }
            } else {
                (String::new(), None)
            };
            let send_message: tl::enums::InputBotInlineMessage =
                tl::types::InputBotInlineMessageText {
                    no_webpage: true, invert_media: false,
                    message: text, entities, reply_markup: markup,
                }.into();

            match r.kind {
                InlineResultKind::Article => {
                    tl::types::InputBotInlineResult {
                        id: r.id,
                        r#type: "article".into(),
                        title: r.title,
                        description: r.description,
                        url: None,
                        thumb: r.thumb_url.map(|u| tl::types::InputWebDocument {
                            url: u, size: 0,
                            mime_type: "image/jpeg".into(),
                            attributes: vec![],
                        }.into()),
                        content: None,
                        send_message,
                    }.into()
                }
                InlineResultKind::Photo { photo_url, .. } => {
                    tl::types::InputBotInlineResult {
                        id: r.id,
                        r#type: "photo".into(),
                        title: r.title,
                        description: r.description,
                        url: Some(photo_url.clone()),
                        thumb: Some(tl::types::InputWebDocument {
                            url: photo_url, size: 0,
                            mime_type: "image/jpeg".into(),
                            attributes: vec![],
                        }.into()),
                        content: None,
                        send_message,
                    }.into()
                }
                InlineResultKind::Gif { gif_url } => {
                    tl::types::InputBotInlineResult {
                        id: r.id,
                        r#type: "gif".into(),
                        title: r.title,
                        description: r.description,
                        url: Some(gif_url.clone()),
                        thumb: Some(tl::types::InputWebDocument {
                            url: gif_url, size: 0,
                            mime_type: "image/gif".into(),
                            attributes: vec![],
                        }.into()),
                        content: None,
                        send_message,
                    }.into()
                }
            }
        }).collect();

        self.client.invoke(&tl::functions::messages::SetInlineBotResults {
            gallery: false,
            private: is_personal,
            query_id: qid,
            results: tl_results,
            cache_time: cache_time.unwrap_or(0),
            next_offset,
            switch_pm: None,
            switch_webview: None,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    // ── Forward / Copy ──

    async fn forward_message(
        &self, chat_id: ChatId, from_chat_id: ChatId, message_id: MessageId,
    ) -> Result<SentMessage, ApiError> {
        let to_peer = self.resolve(chat_id)?;
        let from_peer = self.resolve(from_chat_id)?;
        let result = self.client.invoke(&tl::functions::messages::ForwardMessages {
            silent: false, background: false, with_my_score: false,
            drop_author: false, drop_media_captions: false, noforwards: false,
            allow_paid_floodskip: false,
            from_peer: from_peer.into(),
            id: vec![message_id.0],
            random_id: vec![rand_i64()],
            to_peer: to_peer.into(),
            top_msg_id: None, reply_to: None, schedule_date: None,
            schedule_repeat_period: None, send_as: None,
            quick_reply_shortcut: None, effect: None, video_timestamp: None,
            allow_paid_stars: None, suggested_post: None,
        }).await.map_err(Self::convert_error)?;
        // Extract message ID from the response
        let msg_id = extract_forwarded_msg_id(&result).unwrap_or(0);
        Ok(SentMessage { message_id: MessageId(msg_id), chat_id })
    }

    async fn copy_message(
        &self, chat_id: ChatId, from_chat_id: ChatId, message_id: MessageId,
    ) -> Result<MessageId, ApiError> {
        // MTProto: forward with drop_author=true acts like copy
        let to_peer = self.resolve(chat_id)?;
        let from_peer = self.resolve(from_chat_id)?;
        let result = self.client.invoke(&tl::functions::messages::ForwardMessages {
            silent: false, background: false, with_my_score: false,
            drop_author: true, drop_media_captions: false, noforwards: false,
            allow_paid_floodskip: false,
            from_peer: from_peer.into(),
            id: vec![message_id.0],
            random_id: vec![rand_i64()],
            to_peer: to_peer.into(),
            top_msg_id: None, reply_to: None, schedule_date: None,
            schedule_repeat_period: None, send_as: None,
            quick_reply_shortcut: None, effect: None, video_timestamp: None,
            allow_paid_stars: None, suggested_post: None,
        }).await.map_err(Self::convert_error)?;
        let msg_id = extract_forwarded_msg_id(&result).unwrap_or(0);
        Ok(MessageId(msg_id))
    }

    // ── Download ──

    async fn download_file(&self, file_id: &str) -> Result<DownloadedFile, ApiError> {
        // In MTProto, file_id from incoming updates is the document/photo ID.
        // We need to get the InputFileLocation from the message.
        // For now, use the file_id as a document ID and try to download.
        let id: i64 = file_id.parse()
            .map_err(|_| ApiError::Unknown(format!("invalid file_id: {}", file_id)))?;

        // Try as document first
        let input_location: tl::enums::InputFileLocation = tl::types::InputDocumentFileLocation {
            id,
            access_hash: 0, // We don't store access_hash in file_id currently
            file_reference: Vec::new(),
            thumb_size: String::new(),
        }.into();

        let mut data = Vec::new();
        let mut offset = 0i64;
        let limit = 512 * 1024; // 512KB chunks

        loop {
            let result = self.client.invoke(&tl::functions::upload::GetFile {
                precise: false,
                cdn_supported: false,
                location: input_location.clone(),
                offset,
                limit,
            }).await;

            match result {
                Ok(tl::enums::upload::File::File(file)) => {
                    let bytes = file.bytes;
                    let len = bytes.len();
                    data.extend_from_slice(&bytes);
                    if (len as i32) < limit {
                        break;
                    }
                    offset += len as i64;
                }
                Ok(tl::enums::upload::File::CdnRedirect(_)) => {
                    return Err(ApiError::Unknown("CDN redirect not supported".into()));
                }
                Err(e) => return Err(Self::convert_error(e)),
            }
        }

        Ok(DownloadedFile {
            file_size: Some(data.len()),
            data,
        })
    }

    // ── Polls ──

    async fn send_poll(
        &self, chat_id: ChatId, poll: SendPoll,
    ) -> Result<SentMessage, ApiError> {
        let peer = self.resolve(chat_id)?;
        let answers: Vec<tl::enums::PollAnswer> = poll.options.iter().enumerate().map(|(i, opt)| {
            tl::types::PollAnswer {
                text: tl::types::TextWithEntities {
                    text: opt.clone(),
                    entities: vec![],
                }.into(),
                option: vec![i as u8],
            }.into()
        }).collect();

        let tl_poll = tl::types::Poll {
            id: rand_i64(),
            closed: false,
            public_voters: !poll.is_anonymous,
            multiple_choice: poll.allows_multiple_answers,
            quiz: poll.poll_type == PollType::Quiz,
            question: tl::types::TextWithEntities {
                text: poll.question,
                entities: vec![],
            }.into(),
            answers,
            close_period: poll.open_period,
            close_date: None,
        };

        let media: tl::enums::InputMedia = tl::types::InputMediaPoll {
            poll: tl_poll.into(),
            correct_answers: poll.correct_option_id.map(|i| vec![vec![i as u8]]),
            solution: poll.explanation,
            solution_entities: None,
        }.into();

        let msg = InputMessage::new().media(media);
        let sent = self.client.send_message(peer, msg).await.map_err(Self::convert_error)?;
        Ok(SentMessage { message_id: MessageId(sent.id()), chat_id })
    }

    async fn stop_poll(
        &self, chat_id: ChatId, message_id: MessageId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        // Close the poll by editing its media with closed=true
        // MTProto: messages.editMessage with media containing a closed poll
        // Simpler: just invoke the edit directly
        self.client.invoke(&tl::functions::messages::EditMessage {
            no_webpage: false, invert_media: false,
            peer: peer.into(), id: message_id.0,
            message: None,
            media: Some(tl::types::InputMediaPoll {
                poll: tl::types::Poll {
                    id: 0, // will be filled by server
                    closed: true,
                    public_voters: false,
                    multiple_choice: false,
                    quiz: false,
                    question: tl::types::TextWithEntities { text: String::new(), entities: vec![] }.into(),
                    answers: vec![],
                    close_period: None,
                    close_date: None,
                }.into(),
                correct_answers: None,
                solution: None,
                solution_entities: None,
            }.into()),
            reply_markup: None, entities: None,
            schedule_date: None, schedule_repeat_period: None,
            quick_reply_shortcut_id: None,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    // ── Dice ──

    async fn send_dice(
        &self, chat_id: ChatId, emoji: DiceEmoji,
    ) -> Result<SentMessage, ApiError> {
        let peer = self.resolve(chat_id)?;
        let media: tl::enums::InputMedia = tl::types::InputMediaDice {
            emoticon: emoji.as_str().to_string(),
        }.into();
        let msg = InputMessage::new().media(media);
        let sent = self.client.send_message(peer, msg).await.map_err(Self::convert_error)?;
        Ok(SentMessage { message_id: MessageId(sent.id()), chat_id })
    }

    // ── Contact / Venue ──

    async fn send_contact(
        &self, chat_id: ChatId, contact: Contact,
    ) -> Result<SentMessage, ApiError> {
        let peer = self.resolve(chat_id)?;
        let media: tl::enums::InputMedia = tl::types::InputMediaContact {
            phone_number: contact.phone_number,
            first_name: contact.first_name,
            last_name: contact.last_name.unwrap_or_default(),
            vcard: contact.vcard.unwrap_or_default(),
        }.into();
        let msg = InputMessage::new().media(media);
        let sent = self.client.send_message(peer, msg).await.map_err(Self::convert_error)?;
        Ok(SentMessage { message_id: MessageId(sent.id()), chat_id })
    }

    async fn send_venue(
        &self, chat_id: ChatId, venue: Venue,
    ) -> Result<SentMessage, ApiError> {
        let peer = self.resolve(chat_id)?;
        let media: tl::enums::InputMedia = tl::types::InputMediaVenue {
            geo_point: tl::types::InputGeoPoint {
                lat: venue.latitude, long: venue.longitude, accuracy_radius: None,
            }.into(),
            title: venue.title,
            address: venue.address,
            provider: "foursquare".to_string(),
            venue_id: venue.foursquare_id.unwrap_or_default(),
            venue_type: venue.foursquare_type.unwrap_or_default(),
        }.into();
        let msg = InputMessage::new().media(media);
        let sent = self.client.send_message(peer, msg).await.map_err(Self::convert_error)?;
        Ok(SentMessage { message_id: MessageId(sent.id()), chat_id })
    }

    // ── Chat Administration ──

    async fn ban_chat_member(
        &self, chat_id: ChatId, user_id: UserId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_peer_user: tl::enums::InputPeer = tl::types::InputPeerUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }.into();
        self.client.invoke(&tl::functions::channels::EditBanned {
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
                until_date: 0, // permanent
            }.into(),
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn unban_chat_member(
        &self, chat_id: ChatId, user_id: UserId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_peer_user: tl::enums::InputPeer = tl::types::InputPeerUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }.into();
        // Remove all restrictions
        self.client.invoke(&tl::functions::channels::EditBanned {
            channel: peer.into(),
            participant: input_peer_user,
            banned_rights: tl::types::ChatBannedRights {
                view_messages: false, send_messages: false, send_media: false,
                send_stickers: false, send_gifs: false, send_games: false,
                send_inline: false, embed_links: false, send_polls: false,
                change_info: false, invite_users: false, pin_messages: false,
                manage_topics: false, send_photos: false, send_videos: false,
                send_roundvideos: false, send_audios: false, send_voices: false,
                send_docs: false, send_plain: false, until_date: 0,
            }.into(),
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn get_chat_member_count(
        &self, chat_id: ChatId,
    ) -> Result<i32, ApiError> {
        let peer = self.resolve(chat_id)?;
        let full = self.client.invoke(&tl::functions::messages::GetFullChat {
            chat_id: peer.id.bare_id(),
        }).await;
        match full {
            Ok(tl::enums::messages::ChatFull::Full(f)) => {
                match f.full_chat {
                    tl::enums::ChatFull::Full(cf) => {
                        // ChatFull doesn't have participants_count; count from participants list
                        match cf.participants {
                            tl::enums::ChatParticipants::Participants(p) => Ok(p.participants.len() as i32),
                            tl::enums::ChatParticipants::Forbidden(_) => Ok(0),
                        }
                    }
                    tl::enums::ChatFull::ChannelFull(cf) => Ok(cf.participants_count.unwrap_or(0)),
                }
            }
            Err(e) => Err(Self::convert_error(e)),
        }
    }

    async fn leave_chat(&self, chat_id: ChatId) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        if chat_id.0 < 0 {
            // Channel/supergroup
            self.client.invoke(&tl::functions::channels::LeaveChannel {
                channel: peer.into(),
            }).await.map_err(Self::convert_error)?;
        } else {
            // Regular group
            self.client.invoke(&tl::functions::messages::DeleteChatUser {
                revoke_history: false,
                chat_id: peer.id.bare_id(),
                user_id: tl::types::InputUserSelf {}.into(),
            }).await.map_err(Self::convert_error)?;
        }
        Ok(())
    }

    // ── Bot Settings ──

    async fn set_my_commands(&self, commands: Vec<BotCommand>) -> Result<(), ApiError> {
        let tl_commands: Vec<tl::enums::BotCommand> = commands.into_iter().map(|c| {
            tl::types::BotCommand {
                command: c.command,
                description: c.description,
            }.into()
        }).collect();
        self.client.invoke(&tl::functions::bots::SetBotCommands {
            scope: tl::types::BotCommandScopeDefault {}.into(),
            lang_code: String::new(),
            commands: tl_commands,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn delete_my_commands(&self) -> Result<(), ApiError> {
        self.client.invoke(&tl::functions::bots::ResetBotCommands {
            scope: tl::types::BotCommandScopeDefault {}.into(),
            lang_code: String::new(),
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn get_me(&self) -> Result<BotInfo, ApiError> {
        let me = self.client.get_me().await.map_err(Self::convert_error)?;
        Ok(BotInfo {
            id: UserId(me.id().bare_id() as u64),
            username: me.username().unwrap_or_default().to_string(),
            first_name: me.first_name().unwrap_or_default().to_string(),
            can_join_groups: true,
            can_read_all_group_messages: false,
            supports_inline_queries: false,
        })
    }

    // ── Pinning ──

    async fn pin_chat_message(
        &self, chat_id: ChatId, message_id: MessageId, silent: bool,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client.invoke(&tl::functions::messages::UpdatePinnedMessage {
            silent, unpin: false, pm_oneside: false,
            peer: peer.into(), id: message_id.0,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn unpin_chat_message(
        &self, chat_id: ChatId, message_id: MessageId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client.invoke(&tl::functions::messages::UpdatePinnedMessage {
            silent: true, unpin: true, pm_oneside: false,
            peer: peer.into(), id: message_id.0,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    // ── Reactions ──

    async fn set_message_reaction(
        &self, chat_id: ChatId, message_id: MessageId, emoji: &str,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let reaction: tl::enums::Reaction = if emoji.is_empty() {
            tl::types::ReactionEmpty {}.into()
        } else {
            tl::types::ReactionEmoji { emoticon: emoji.to_string() }.into()
        };
        self.client.invoke(&tl::functions::messages::SendReaction {
            big: false, add_to_recent: true,
            peer: peer.into(),
            msg_id: message_id.0,
            reaction: Some(vec![reaction]),
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    // ── Invite Links ──

    async fn export_chat_invite_link(
        &self, chat_id: ChatId,
    ) -> Result<String, ApiError> {
        let peer = self.resolve(chat_id)?;
        let result = self.client.invoke(&tl::functions::messages::ExportChatInvite {
            legacy_revoke_permanent: false,
            request_needed: false,
            peer: peer.into(),
            expire_date: None,
            usage_limit: None,
            title: None,
            subscription_pricing: None,
        }).await.map_err(Self::convert_error)?;
        match result {
            tl::enums::ExportedChatInvite::ChatInviteExported(inv) => Ok(inv.link),
            tl::enums::ExportedChatInvite::ChatInvitePublicJoinRequests => {
                Err(ApiError::Unknown("public join request, no direct link".into()))
            }
        }
    }

    // ── Payments ──

    async fn answer_pre_checkout_query(
        &self, id: String, ok: bool, error_message: Option<String>,
    ) -> Result<(), ApiError> {
        let query_id: i64 = id.parse()
            .map_err(|_| ApiError::Unknown(format!("invalid pre_checkout_query id: {id}")))?;
        self.client.invoke(&tl::functions::messages::SetBotPrecheckoutResults {
            success: ok,
            query_id,
            error: error_message,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    // ── Chat Management ──

    async fn set_chat_title(
        &self, chat_id: ChatId, title: &str,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        if chat_id.0 < -1_000_000_000 {
            self.client.invoke(&tl::functions::channels::EditTitle {
                channel: peer.into(),
                title: title.to_string(),
            }).await.map_err(Self::convert_error)?;
        } else {
            self.client.invoke(&tl::functions::messages::EditChatTitle {
                chat_id: peer.id.bare_id(),
                title: title.to_string(),
            }).await.map_err(Self::convert_error)?;
        }
        Ok(())
    }

    async fn set_chat_description(
        &self, chat_id: ChatId, description: Option<&str>,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client.invoke(&tl::functions::messages::EditChatAbout {
            peer: peer.into(),
            about: description.unwrap_or("").to_string(),
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn delete_chat_photo(
        &self, chat_id: ChatId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        if chat_id.0 < -1_000_000_000 {
            self.client.invoke(&tl::functions::channels::EditPhoto {
                channel: peer.into(),
                photo: tl::types::InputChatPhotoEmpty {}.into(),
            }).await.map_err(Self::convert_error)?;
        } else {
            self.client.invoke(&tl::functions::messages::EditChatPhoto {
                chat_id: peer.id.bare_id(),
                photo: tl::types::InputChatPhotoEmpty {}.into(),
            }).await.map_err(Self::convert_error)?;
        }
        Ok(())
    }

    async fn get_chat_administrators(
        &self, chat_id: ChatId,
    ) -> Result<Vec<ChatMember>, ApiError> {
        let peer = self.resolve(chat_id)?;
        let result = self.client.invoke(&tl::functions::channels::GetParticipants {
            channel: peer.into(),
            filter: tl::types::ChannelParticipantsAdmins {}.into(),
            offset: 0,
            limit: 200,
            hash: 0,
        }).await.map_err(Self::convert_error)?;

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

    async fn set_chat_administrator_custom_title(
        &self, chat_id: ChatId, user_id: UserId, custom_title: &str,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }.into();
        self.client.invoke(&tl::functions::channels::EditAdmin {
            channel: peer.into(),
            user_id: input_user,
            admin_rights: tl::types::ChatAdminRights {
                change_info: false, post_messages: false, edit_messages: false,
                delete_messages: false, ban_users: false, invite_users: false,
                pin_messages: false, add_admins: false, anonymous: false,
                manage_call: false, other: true, manage_topics: false,
                post_stories: false, edit_stories: false, delete_stories: false,
                manage_direct_messages: false,
            }.into(),
            rank: custom_title.to_string(),
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    // ── Chat Join Requests ──

    async fn approve_chat_join_request(
        &self, chat_id: ChatId, user_id: UserId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }.into();
        self.client.invoke(&tl::functions::messages::HideChatJoinRequest {
            approved: true,
            peer: peer.into(),
            user_id: input_user,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn decline_chat_join_request(
        &self, chat_id: ChatId, user_id: UserId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }.into();
        self.client.invoke(&tl::functions::messages::HideChatJoinRequest {
            approved: false,
            peer: peer.into(),
            user_id: input_user,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    // ── User Info ──

    async fn get_user_profile_photos(
        &self, user_id: UserId, offset: Option<i32>, limit: Option<i32>,
    ) -> Result<UserProfilePhotos, ApiError> {
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }.into();
        let result = self.client.invoke(&tl::functions::photos::GetUserPhotos {
            user_id: input_user,
            offset: offset.unwrap_or(0),
            max_id: 0,
            limit: limit.unwrap_or(100),
        }).await.map_err(Self::convert_error)?;

        match result {
            tl::enums::photos::Photos::Photos(p) => {
                let photos: Vec<String> = p.photos.iter().filter_map(|photo| {
                    if let tl::enums::Photo::Photo(ph) = photo {
                        Some(ph.id.to_string())
                    } else {
                        None
                    }
                }).collect();
                Ok(UserProfilePhotos { total_count: photos.len() as i32, photos })
            }
            tl::enums::photos::Photos::Slice(p) => {
                let photos: Vec<String> = p.photos.iter().filter_map(|photo| {
                    if let tl::enums::Photo::Photo(ph) = photo {
                        Some(ph.id.to_string())
                    } else {
                        None
                    }
                }).collect();
                Ok(UserProfilePhotos { total_count: p.count, photos })
            }
        }
    }

    // ── Bot Settings (extended) ──

    async fn get_my_commands(&self) -> Result<Vec<BotCommand>, ApiError> {
        let result = self.client.invoke(&tl::functions::bots::GetBotCommands {
            scope: tl::types::BotCommandScopeDefault {}.into(),
            lang_code: String::new(),
        }).await.map_err(Self::convert_error)?;
        Ok(result.into_iter().map(|cmd| {
            let tl::enums::BotCommand::Command(c) = cmd;
            BotCommand {
                command: c.command,
                description: c.description,
            }
        }).collect())
    }

    async fn set_my_description(
        &self, description: Option<&str>, language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.client.invoke(&tl::functions::bots::SetBotInfo {
            bot: None,
            lang_code: language_code.unwrap_or("").to_string(),
            name: None,
            about: description.map(|s| s.to_string()),
            description: None,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn get_my_description(
        &self, language_code: Option<&str>,
    ) -> Result<BotDescription, ApiError> {
        let result = self.client.invoke(&tl::functions::bots::GetBotInfo {
            bot: None,
            lang_code: language_code.unwrap_or("").to_string(),
        }).await.map_err(Self::convert_error)?;
        let tl::enums::bots::BotInfo::Info(info) = result;
        Ok(BotDescription {
            description: info.about,
        })
    }

    async fn set_my_short_description(
        &self, short_description: Option<&str>, language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.client.invoke(&tl::functions::bots::SetBotInfo {
            bot: None,
            lang_code: language_code.unwrap_or("").to_string(),
            name: None,
            about: None,
            description: short_description.map(|s| s.to_string()),
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn get_my_short_description(
        &self, language_code: Option<&str>,
    ) -> Result<BotShortDescription, ApiError> {
        let result = self.client.invoke(&tl::functions::bots::GetBotInfo {
            bot: None,
            lang_code: language_code.unwrap_or("").to_string(),
        }).await.map_err(Self::convert_error)?;
        let tl::enums::bots::BotInfo::Info(info) = result;
        Ok(BotShortDescription {
            short_description: info.description,
        })
    }

    async fn set_my_name(
        &self, name: Option<&str>, language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.client.invoke(&tl::functions::bots::SetBotInfo {
            bot: None,
            lang_code: language_code.unwrap_or("").to_string(),
            name: name.map(|s| s.to_string()),
            about: None,
            description: None,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn get_my_name(
        &self, language_code: Option<&str>,
    ) -> Result<BotName, ApiError> {
        let result = self.client.invoke(&tl::functions::bots::GetBotInfo {
            bot: None,
            lang_code: language_code.unwrap_or("").to_string(),
        }).await.map_err(Self::convert_error)?;
        let tl::enums::bots::BotInfo::Info(info) = result;
        Ok(BotName {
            name: info.name,
        })
    }

    // ── Menu Button ──

    async fn set_chat_menu_button(
        &self, chat_id: Option<ChatId>, menu_button: MenuButton,
    ) -> Result<(), ApiError> {
        let user = if let Some(cid) = chat_id {
            let peer = self.resolve(cid)?;
            tl::types::InputUser {
                user_id: peer.id.bare_id(),
                access_hash: peer.auth.hash(),
            }.into()
        } else {
            tl::types::InputUserEmpty {}.into()
        };

        let button: tl::enums::BotMenuButton = match menu_button {
            MenuButton::Default => tl::types::BotMenuButtonDefault {}.into(),
            MenuButton::Commands => tl::types::BotMenuButtonCommands {}.into(),
            MenuButton::WebApp { text, url } => tl::types::BotMenuButton {
                text,
                url,
            }.into(),
        };

        self.client.invoke(&tl::functions::bots::SetBotMenuButton {
            user_id: user,
            button,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn get_chat_menu_button(
        &self, chat_id: Option<ChatId>,
    ) -> Result<MenuButton, ApiError> {
        let user = if let Some(cid) = chat_id {
            let peer = self.resolve(cid)?;
            tl::types::InputUser {
                user_id: peer.id.bare_id(),
                access_hash: peer.auth.hash(),
            }.into()
        } else {
            tl::types::InputUserEmpty {}.into()
        };

        let result = self.client.invoke(&tl::functions::bots::GetBotMenuButton {
            user_id: user,
        }).await.map_err(Self::convert_error)?;

        Ok(match result {
            tl::enums::BotMenuButton::Button(b) => MenuButton::WebApp {
                text: b.text,
                url: b.url,
            },
            tl::enums::BotMenuButton::Commands => MenuButton::Commands,
            tl::enums::BotMenuButton::Default => MenuButton::Default,
        })
    }

    // ── Batch Operations ──

    async fn forward_messages(
        &self, chat_id: ChatId, from_chat_id: ChatId, message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        if message_ids.is_empty() { return Ok(vec![]); }
        let to_peer = self.resolve(chat_id)?;
        let from_peer = self.resolve(from_chat_id)?;
        let ids: Vec<i32> = message_ids.iter().map(|m| m.0).collect();
        let random_ids: Vec<i64> = (0..ids.len()).map(|_| rand_i64()).collect();
        let result = self.client.invoke(&tl::functions::messages::ForwardMessages {
            silent: false, background: false, with_my_score: false,
            drop_author: false, drop_media_captions: false, noforwards: false,
            allow_paid_floodskip: false,
            from_peer: from_peer.into(),
            id: ids,
            random_id: random_ids,
            to_peer: to_peer.into(),
            top_msg_id: None, reply_to: None, schedule_date: None,
            schedule_repeat_period: None, send_as: None,
            quick_reply_shortcut: None, effect: None, video_timestamp: None,
            allow_paid_stars: None, suggested_post: None,
        }).await.map_err(Self::convert_error)?;
        Ok(extract_all_msg_ids(&result).into_iter().map(MessageId).collect())
    }

    async fn copy_messages(
        &self, chat_id: ChatId, from_chat_id: ChatId, message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        if message_ids.is_empty() { return Ok(vec![]); }
        let to_peer = self.resolve(chat_id)?;
        let from_peer = self.resolve(from_chat_id)?;
        let ids: Vec<i32> = message_ids.iter().map(|m| m.0).collect();
        let random_ids: Vec<i64> = (0..ids.len()).map(|_| rand_i64()).collect();
        let result = self.client.invoke(&tl::functions::messages::ForwardMessages {
            silent: false, background: false, with_my_score: false,
            drop_author: true, drop_media_captions: false, noforwards: false,
            allow_paid_floodskip: false,
            from_peer: from_peer.into(),
            id: ids,
            random_id: random_ids,
            to_peer: to_peer.into(),
            top_msg_id: None, reply_to: None, schedule_date: None,
            schedule_repeat_period: None, send_as: None,
            quick_reply_shortcut: None, effect: None, video_timestamp: None,
            allow_paid_stars: None, suggested_post: None,
        }).await.map_err(Self::convert_error)?;
        Ok(extract_all_msg_ids(&result).into_iter().map(MessageId).collect())
    }

    // ── Sticker / Location convenience ──

    async fn send_sticker(
        &self, chat_id: ChatId, sticker: FileSource,
    ) -> Result<SentMessage, ApiError> {
        self.send_message(chat_id, MessageContent::Sticker { source: sticker }, SendOptions::default()).await
    }

    async fn send_location(
        &self, chat_id: ChatId, latitude: f64, longitude: f64,
    ) -> Result<SentMessage, ApiError> {
        self.send_message(chat_id, MessageContent::Location {
            latitude, longitude, keyboard: None,
        }, SendOptions::default()).await
    }

    // ── Forum Topics ──

    async fn create_forum_topic(
        &self, chat_id: ChatId, title: &str,
        icon_color: Option<i32>, icon_custom_emoji_id: Option<i64>,
    ) -> Result<ForumTopic, ApiError> {
        let peer = self.resolve(chat_id)?;
        let result = self.client.invoke(&tl::functions::messages::CreateForumTopic {
            title_missing: false,
            peer: peer.into(),
            title: title.to_string(),
            icon_color,
            icon_emoji_id: icon_custom_emoji_id,
            random_id: rand_i64(),
            send_as: None,
        }).await.map_err(Self::convert_error)?;
        // Extract topic ID from the Updates response
        let topic_id = extract_forum_topic_id(&result).unwrap_or(0);
        Ok(ForumTopic {
            id: topic_id,
            title: title.to_string(),
            icon_color,
            icon_custom_emoji_id: icon_custom_emoji_id.map(|id| id.to_string()),
            is_closed: false,
            is_hidden: false,
        })
    }

    async fn edit_forum_topic(
        &self, chat_id: ChatId, topic_id: i32,
        title: Option<&str>, icon_custom_emoji_id: Option<i64>,
        closed: Option<bool>, hidden: Option<bool>,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client.invoke(&tl::functions::messages::EditForumTopic {
            peer: peer.into(),
            topic_id,
            title: title.map(|s| s.to_string()),
            icon_emoji_id: icon_custom_emoji_id,
            closed,
            hidden,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn delete_forum_topic(
        &self, chat_id: ChatId, topic_id: i32,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client.invoke(&tl::functions::messages::DeleteTopicHistory {
            peer: peer.into(),
            top_msg_id: topic_id,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    async fn unpin_all_forum_topic_messages(
        &self, chat_id: ChatId, topic_id: i32,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client.invoke(&tl::functions::messages::UnpinAllMessages {
            peer: peer.into(),
            top_msg_id: Some(topic_id),
            saved_peer_id: None,
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }

    // ── Stars API ──

    async fn get_star_transactions(
        &self, offset: Option<&str>, limit: Option<i32>,
    ) -> Result<StarTransactions, ApiError> {
        let result = self.client.invoke(&tl::functions::payments::GetStarsTransactions {
            inbound: false,
            outbound: false,
            ascending: false,
            ton: false,
            subscription_id: None,
            peer: tl::types::InputPeerSelf {}.into(),
            offset: offset.unwrap_or("").to_string(),
            limit: limit.unwrap_or(100),
        }).await.map_err(Self::convert_error)?;

        let tl::enums::payments::StarsStatus::Status(status) = result;
        let tl::enums::StarsAmount::Amount(balance_amount) = status.balance else {
            return Ok(StarTransactions {
                balance: StarBalance { amount: 0, nanos: 0 },
                transactions: vec![],
                next_offset: None,
            });
        };

        let transactions = status.history.unwrap_or_default().into_iter().map(|tx| {
            let tl::enums::StarsTransaction::Transaction(t) = tx;
            let (amount, nanos) = match t.amount {
                tl::enums::StarsAmount::Amount(a) => (a.amount, a.nanos),
                _ => (0, 0),
            };
            let source = match t.peer {
                tl::enums::StarsTransactionPeer::Peer(p) => {
                    // p.peer is tl::enums::Peer
                    match p.peer {
                        tl::enums::Peer::User(u) => StarTransactionPeer::User(UserId(u.user_id as u64)),
                        _ => StarTransactionPeer::Unknown,
                    }
                }
                tl::enums::StarsTransactionPeer::AppStore => StarTransactionPeer::AppStore,
                tl::enums::StarsTransactionPeer::PlayMarket => StarTransactionPeer::PlayMarket,
                tl::enums::StarsTransactionPeer::Fragment => StarTransactionPeer::Fragment,
                tl::enums::StarsTransactionPeer::PremiumBot => StarTransactionPeer::PremiumBot,
                tl::enums::StarsTransactionPeer::Ads => StarTransactionPeer::Ads,
                tl::enums::StarsTransactionPeer::Api => StarTransactionPeer::Api,
                _ => StarTransactionPeer::Unknown,
            };
            StarTransaction {
                id: t.id,
                amount,
                nanos,
                date: t.date,
                source,
                title: t.title,
                description: t.description,
                is_refund: t.refund,
            }
        }).collect();

        Ok(StarTransactions {
            balance: StarBalance {
                amount: balance_amount.amount,
                nanos: balance_amount.nanos,
            },
            transactions,
            next_offset: status.next_offset,
        })
    }

    async fn refund_star_payment(
        &self, user_id: UserId, charge_id: &str,
    ) -> Result<(), ApiError> {
        let user_peer = self.resolve(ChatId(user_id.0 as i64))?;
        let input_user: tl::enums::InputUser = tl::types::InputUser {
            user_id: user_peer.id.bare_id(),
            access_hash: user_peer.auth.hash(),
        }.into();
        self.client.invoke(&tl::functions::payments::RefundStarsCharge {
            user_id: input_user,
            charge_id: charge_id.to_string(),
        }).await.map_err(Self::convert_error)?;
        Ok(())
    }
}

// ── Helpers ──

fn extract_forwarded_msg_id(updates: &tl::enums::Updates) -> Option<i32> {
    match updates {
        tl::enums::Updates::Updates(u) => {
            for update in &u.updates {
                if let tl::enums::Update::NewMessage(tl::types::UpdateNewMessage { message: tl::enums::Message::Message(m), .. })
                    | tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage { message: tl::enums::Message::Message(m), .. }) = update
                {
                    return Some(m.id);
                }
            }
            None
        }
        tl::enums::Updates::Combined(u) => {
            for update in &u.updates {
                if let tl::enums::Update::NewMessage(tl::types::UpdateNewMessage { message: tl::enums::Message::Message(m), .. }) = update {
                    return Some(m.id);
                }
            }
            None
        }
        tl::enums::Updates::UpdateShortSentMessage(m) => Some(m.id),
        _ => None,
    }
}

fn extract_forum_topic_id(updates: &tl::enums::Updates) -> Option<i32> {
    // The topic creation service message's ID is the topic_id
    match updates {
        tl::enums::Updates::Updates(u) => {
            for update in &u.updates {
                if let tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage { message, .. }) = update {
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

fn extract_all_msg_ids(updates: &tl::enums::Updates) -> Vec<i32> {
    let mut ids = Vec::new();
    match updates {
        tl::enums::Updates::Updates(u) => {
            for update in &u.updates {
                match update {
                    tl::enums::Update::NewMessage(tl::types::UpdateNewMessage { message: tl::enums::Message::Message(m), .. })
                    | tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage { message: tl::enums::Message::Message(m), .. }) => {
                        ids.push(m.id);
                    }
                    _ => {}
                }
            }
        }
        tl::enums::Updates::Combined(u) => {
            for update in &u.updates {
                if let tl::enums::Update::NewMessage(tl::types::UpdateNewMessage { message: tl::enums::Message::Message(m), .. }) = update {
                    ids.push(m.id);
                }
            }
        }
        tl::enums::Updates::UpdateShortSentMessage(m) => { ids.push(m.id); }
        _ => {}
    }
    ids
}

fn rand_i64() -> i64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::SystemTime;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let d = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let cnt = COUNTER.fetch_add(1, Ordering::Relaxed);
    (d.as_nanos() as i64) ^ (cnt as i64 * 6_364_136_223_846_793_005 + 1)
}
