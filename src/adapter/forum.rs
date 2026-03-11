//! Forum topics: create, edit, delete, unpin.

use grammers_client::tl;

use super::GrammersAdapter;
use super::helpers::{extract_forum_topic_id, rand_i64};
use crate::error::ApiError;
use crate::types::*;

impl GrammersAdapter {
    pub(crate) async fn impl_create_forum_topic(
        &self,
        chat_id: ChatId,
        title: &str,
        icon_color: Option<i32>,
        icon_custom_emoji_id: Option<i64>,
    ) -> Result<ForumTopic, ApiError> {
        let peer = self.resolve(chat_id)?;
        let result = self
            .client
            .invoke(&tl::functions::messages::CreateForumTopic {
                title_missing: false,
                peer: peer.into(),
                title: title.to_string(),
                icon_color,
                icon_emoji_id: icon_custom_emoji_id,
                random_id: rand_i64(),
                send_as: None,
            })
            .await
            .map_err(Self::convert_error)?;
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

    pub(crate) async fn impl_edit_forum_topic(
        &self,
        chat_id: ChatId,
        topic_id: i32,
        title: Option<&str>,
        icon_custom_emoji_id: Option<i64>,
        closed: Option<bool>,
        hidden: Option<bool>,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client
            .invoke(&tl::functions::messages::EditForumTopic {
                peer: peer.into(),
                topic_id,
                title: title.map(|s| s.to_string()),
                icon_emoji_id: icon_custom_emoji_id,
                closed,
                hidden,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_delete_forum_topic(
        &self,
        chat_id: ChatId,
        topic_id: i32,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client
            .invoke(&tl::functions::messages::DeleteTopicHistory {
                peer: peer.into(),
                top_msg_id: topic_id,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_unpin_all_forum_topic_messages(
        &self,
        chat_id: ChatId,
        topic_id: i32,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client
            .invoke(&tl::functions::messages::UnpinAllMessages {
                peer: peer.into(),
                top_msg_id: Some(topic_id),
                saved_peer_id: None,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }
}
