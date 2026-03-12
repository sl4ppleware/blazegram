//! Core messaging: send, edit, delete, answer callback, chat action, inline query, forward, copy.

use grammers_client::tl;

use super::GrammersAdapter;
use super::helpers::{extract_all_msg_ids, extract_forwarded_msg_id, rand_i64};
use crate::bot_api::SendOptions;
use crate::error::ApiError;
use crate::keyboard::InlineKeyboard;
use crate::types::*;

/// Core BotApi methods — send, edit, delete, callback, chat action, inline, forward, copy.
impl GrammersAdapter {
    pub(crate) async fn impl_send_message(
        &self,
        chat_id: ChatId,
        content: MessageContent,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        let peer = self.resolve(chat_id)?;
        let mut msg = self
            .build_input(&content, opts.reply_keyboard.as_ref())
            .await?;
        if let Some(reply_id) = opts.reply_to {
            msg = msg.reply_to(Some(reply_id.0));
        }
        let sent = self
            .client
            .send_message(peer, msg)
            .await
            .map_err(Self::convert_error)?;
        Ok(SentMessage {
            message_id: MessageId(sent.id()),
            chat_id,
        })
    }

    pub(crate) async fn impl_edit_message_text(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        text: String,
        parse_mode: ParseMode,
        keyboard: Option<InlineKeyboard>,
        link_preview: bool,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let msg = Self::text_msg(&text, parse_mode, link_preview);
        let msg = if let Some(kb) = &keyboard {
            msg.reply_markup(Self::to_inline_markup(kb))
        } else {
            msg
        };
        self.client
            .edit_message(peer, message_id.0, msg)
            .await
            .map_err(Self::convert_error)
    }

    pub(crate) async fn impl_edit_message_caption(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        caption: Option<String>,
        parse_mode: ParseMode,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let cap = caption.as_deref().unwrap_or("");
        let msg = Self::text_msg(cap, parse_mode, false);
        let msg = if let Some(kb) = &keyboard {
            msg.reply_markup(Self::to_inline_markup(kb))
        } else {
            msg
        };
        self.client
            .edit_message(peer, message_id.0, msg)
            .await
            .map_err(Self::convert_error)
    }

    pub(crate) async fn impl_edit_message_media(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        content: MessageContent,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let msg = self.build_input(&content, None).await?;
        let msg = if let Some(kb) = &keyboard {
            msg.reply_markup(Self::to_inline_markup(kb))
        } else {
            msg
        };
        self.client
            .edit_message(peer, message_id.0, msg)
            .await
            .map_err(Self::convert_error)
    }

    pub(crate) async fn impl_edit_message_keyboard(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        let markup = keyboard.as_ref().map(Self::to_inline_markup);
        self.client
            .invoke(&tl::functions::messages::EditMessage {
                no_webpage: false,
                invert_media: false,
                peer: peer.into(),
                id: message_id.0,
                message: None,
                media: None,
                reply_markup: markup.map(|m| m.raw),
                entities: None,
                schedule_date: None,
                schedule_repeat_period: None,
                quick_reply_shortcut_id: None,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_delete_messages(
        &self,
        chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<(), ApiError> {
        if message_ids.is_empty() {
            return Ok(());
        }
        let peer = self.resolve(chat_id)?;
        let ids: Vec<i32> = message_ids.iter().map(|m| m.0).collect();
        for chunk in ids.chunks(100) {
            match self.client.delete_messages(peer, chunk).await {
                Ok(_) => {}
                Err(e) => {
                    let err = Self::convert_error(e);
                    if !matches!(err, ApiError::MessageNotFound) {
                        return Err(err);
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn impl_answer_callback_query(
        &self,
        id: String,
        text: Option<String>,
        show_alert: bool,
    ) -> Result<(), ApiError> {
        let query_id: i64 = id
            .parse()
            .map_err(|_| ApiError::Unknown(format!("invalid callback query id: {id}")))?;
        self.client
            .invoke(&tl::functions::messages::SetBotCallbackAnswer {
                alert: show_alert,
                query_id,
                message: text,
                url: None,
                cache_time: 0,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_send_chat_action(
        &self,
        chat_id: ChatId,
        action: ChatAction,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client
            .invoke(&tl::functions::messages::SetTyping {
                peer: peer.into(),
                top_msg_id: None,
                action: Self::tl_action(action),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_answer_inline_query(
        &self,
        query_id: String,
        results: Vec<InlineQueryResult>,
        next_offset: Option<String>,
        cache_time: Option<i32>,
        is_personal: bool,
    ) -> Result<(), ApiError> {
        use crate::types::InlineResultKind;
        let qid: i64 = query_id
            .parse()
            .map_err(|_| ApiError::Unknown(format!("invalid inline query id: {query_id}")))?;

        let tl_results: Vec<tl::enums::InputBotInlineResult> = results
            .into_iter()
            .map(|r| {
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
                        no_webpage: true,
                        invert_media: false,
                        message: text,
                        entities,
                        reply_markup: markup,
                    }
                    .into();

                match r.kind {
                    InlineResultKind::Article => tl::types::InputBotInlineResult {
                        id: r.id,
                        r#type: "article".into(),
                        title: r.title,
                        description: r.description,
                        url: None,
                        thumb: r.thumb_url.map(|u| {
                            tl::types::InputWebDocument {
                                url: u,
                                size: 0,
                                mime_type: "image/jpeg".into(),
                                attributes: vec![],
                            }
                            .into()
                        }),
                        content: None,
                        send_message,
                    }
                    .into(),
                    InlineResultKind::Photo { url } => tl::types::InputBotInlineResult {
                        id: r.id,
                        r#type: "photo".into(),
                        title: r.title,
                        description: r.description,
                        url: Some(url.clone()),
                        thumb: Some(
                            tl::types::InputWebDocument {
                                url,
                                size: 0,
                                mime_type: "image/jpeg".into(),
                                attributes: vec![],
                            }
                            .into(),
                        ),
                        content: None,
                        send_message,
                    }
                    .into(),
                    InlineResultKind::Gif { url } => tl::types::InputBotInlineResult {
                        id: r.id,
                        r#type: "gif".into(),
                        title: r.title,
                        description: r.description,
                        url: Some(url.clone()),
                        thumb: Some(
                            tl::types::InputWebDocument {
                                url,
                                size: 0,
                                mime_type: "image/gif".into(),
                                attributes: vec![],
                            }
                            .into(),
                        ),
                        content: None,
                        send_message,
                    }
                    .into(),
                    InlineResultKind::Video { url, mime } => tl::types::InputBotInlineResult {
                        id: r.id,
                        r#type: "video".into(),
                        title: r.title,
                        description: r.description,
                        url: Some(url.clone()),
                        thumb: r.thumb_url.map(|u| {
                            tl::types::InputWebDocument {
                                url: u,
                                size: 0,
                                mime_type: "image/jpeg".into(),
                                attributes: vec![],
                            }
                            .into()
                        }),
                        content: Some(
                            tl::types::InputWebDocument {
                                url,
                                size: 0,
                                mime_type: mime,
                                attributes: vec![],
                            }
                            .into(),
                        ),
                        send_message,
                    }
                    .into(),
                    InlineResultKind::Voice { url } => tl::types::InputBotInlineResult {
                        id: r.id,
                        r#type: "voice".into(),
                        title: r.title,
                        description: r.description,
                        url: Some(url.clone()),
                        thumb: None,
                        content: Some(
                            tl::types::InputWebDocument {
                                url,
                                size: 0,
                                mime_type: "audio/ogg".into(),
                                attributes: vec![],
                            }
                            .into(),
                        ),
                        send_message,
                    }
                    .into(),
                    InlineResultKind::Document { url, mime } => tl::types::InputBotInlineResult {
                        id: r.id,
                        r#type: "document".into(),
                        title: r.title,
                        description: r.description,
                        url: Some(url.clone()),
                        thumb: r.thumb_url.map(|u| {
                            tl::types::InputWebDocument {
                                url: u,
                                size: 0,
                                mime_type: "image/jpeg".into(),
                                attributes: vec![],
                            }
                            .into()
                        }),
                        content: Some(
                            tl::types::InputWebDocument {
                                url,
                                size: 0,
                                mime_type: mime,
                                attributes: vec![],
                            }
                            .into(),
                        ),
                        send_message,
                    }
                    .into(),
                }
            })
            .collect();

        self.client
            .invoke(&tl::functions::messages::SetInlineBotResults {
                gallery: false,
                private: is_personal,
                query_id: qid,
                results: tl_results,
                cache_time: cache_time.unwrap_or(0),
                next_offset,
                switch_pm: None,
                switch_webview: None,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    // ── Forward / Copy ──

    pub(crate) async fn impl_forward_message(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<SentMessage, ApiError> {
        let to_peer = self.resolve(chat_id)?;
        let from_peer = self.resolve(from_chat_id)?;
        let result = self
            .client
            .invoke(&tl::functions::messages::ForwardMessages {
                silent: false,
                background: false,
                with_my_score: false,
                drop_author: false,
                drop_media_captions: false,
                noforwards: false,
                allow_paid_floodskip: false,
                from_peer: from_peer.into(),
                id: vec![message_id.0],
                random_id: vec![rand_i64()],
                to_peer: to_peer.into(),
                top_msg_id: None,
                reply_to: None,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                video_timestamp: None,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
            .map_err(Self::convert_error)?;
        let msg_id = extract_forwarded_msg_id(&result).unwrap_or(0);
        Ok(SentMessage {
            message_id: MessageId(msg_id),
            chat_id,
        })
    }

    pub(crate) async fn impl_copy_message(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<MessageId, ApiError> {
        let to_peer = self.resolve(chat_id)?;
        let from_peer = self.resolve(from_chat_id)?;
        let result = self
            .client
            .invoke(&tl::functions::messages::ForwardMessages {
                silent: false,
                background: false,
                with_my_score: false,
                drop_author: true,
                drop_media_captions: false,
                noforwards: false,
                allow_paid_floodskip: false,
                from_peer: from_peer.into(),
                id: vec![message_id.0],
                random_id: vec![rand_i64()],
                to_peer: to_peer.into(),
                top_msg_id: None,
                reply_to: None,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                video_timestamp: None,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
            .map_err(Self::convert_error)?;
        let msg_id = extract_forwarded_msg_id(&result).unwrap_or(0);
        Ok(MessageId(msg_id))
    }

    // ── Batch forward / copy ──

    pub(crate) async fn impl_forward_messages(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        if message_ids.is_empty() {
            return Ok(vec![]);
        }
        let to_peer = self.resolve(chat_id)?;
        let from_peer = self.resolve(from_chat_id)?;
        let ids: Vec<i32> = message_ids.iter().map(|m| m.0).collect();
        let random_ids: Vec<i64> = (0..ids.len()).map(|_| rand_i64()).collect();
        let result = self
            .client
            .invoke(&tl::functions::messages::ForwardMessages {
                silent: false,
                background: false,
                with_my_score: false,
                drop_author: false,
                drop_media_captions: false,
                noforwards: false,
                allow_paid_floodskip: false,
                from_peer: from_peer.into(),
                id: ids,
                random_id: random_ids,
                to_peer: to_peer.into(),
                top_msg_id: None,
                reply_to: None,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                video_timestamp: None,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(extract_all_msg_ids(&result)
            .into_iter()
            .map(MessageId)
            .collect())
    }

    pub(crate) async fn impl_copy_messages(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        if message_ids.is_empty() {
            return Ok(vec![]);
        }
        let to_peer = self.resolve(chat_id)?;
        let from_peer = self.resolve(from_chat_id)?;
        let ids: Vec<i32> = message_ids.iter().map(|m| m.0).collect();
        let random_ids: Vec<i64> = (0..ids.len()).map(|_| rand_i64()).collect();
        let result = self
            .client
            .invoke(&tl::functions::messages::ForwardMessages {
                silent: false,
                background: false,
                with_my_score: false,
                drop_author: true,
                drop_media_captions: false,
                noforwards: false,
                allow_paid_floodskip: false,
                from_peer: from_peer.into(),
                id: ids,
                random_id: random_ids,
                to_peer: to_peer.into(),
                top_msg_id: None,
                reply_to: None,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                video_timestamp: None,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(extract_all_msg_ids(&result)
            .into_iter()
            .map(MessageId)
            .collect())
    }
}
