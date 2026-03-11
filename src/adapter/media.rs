//! Media operations: download, poll, dice, contact, venue, sticker, location.

use grammers_client::{message::InputMessage, tl};

use super::GrammersAdapter;
use super::helpers::rand_i64;
use crate::bot_api::SendOptions;
use crate::error::ApiError;
use crate::types::*;

impl GrammersAdapter {
    pub(crate) async fn impl_download_file(
        &self,
        file_id: &str,
    ) -> Result<DownloadedFile, ApiError> {
        let id: i64 = file_id
            .parse()
            .map_err(|_| ApiError::Unknown(format!("invalid file_id: {}", file_id)))?;

        let input_location: tl::enums::InputFileLocation = tl::types::InputDocumentFileLocation {
            id,
            access_hash: 0,
            file_reference: Vec::new(),
            thumb_size: String::new(),
        }
        .into();

        let mut data = Vec::new();
        let mut offset = 0i64;
        let limit = 512 * 1024;

        loop {
            let result = self
                .client
                .invoke(&tl::functions::upload::GetFile {
                    precise: false,
                    cdn_supported: false,
                    location: input_location.clone(),
                    offset,
                    limit,
                })
                .await;

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

    pub(crate) async fn impl_send_poll(
        &self,
        chat_id: ChatId,
        poll: SendPoll,
    ) -> Result<SentMessage, ApiError> {
        let peer = self.resolve(chat_id)?;
        let answers: Vec<tl::enums::PollAnswer> = poll
            .options
            .iter()
            .enumerate()
            .map(|(i, opt)| {
                tl::types::PollAnswer {
                    text: tl::types::TextWithEntities {
                        text: opt.clone(),
                        entities: vec![],
                    }
                    .into(),
                    option: vec![i as u8],
                }
                .into()
            })
            .collect();

        let tl_poll = tl::types::Poll {
            id: rand_i64(),
            closed: false,
            public_voters: !poll.is_anonymous,
            multiple_choice: poll.allows_multiple_answers,
            quiz: poll.poll_type == PollType::Quiz,
            question: tl::types::TextWithEntities {
                text: poll.question,
                entities: vec![],
            }
            .into(),
            answers,
            close_period: poll.open_period,
            close_date: None,
        };

        let media: tl::enums::InputMedia = tl::types::InputMediaPoll {
            poll: tl_poll.into(),
            correct_answers: poll.correct_option_id.map(|i| vec![vec![i as u8]]),
            solution: poll.explanation,
            solution_entities: None,
        }
        .into();

        let msg = InputMessage::new().media(media);
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

    pub(crate) async fn impl_stop_poll(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<(), ApiError> {
        let peer = self.resolve(chat_id)?;
        self.client
            .invoke(&tl::functions::messages::EditMessage {
                no_webpage: false,
                invert_media: false,
                peer: peer.into(),
                id: message_id.0,
                message: None,
                media: Some(
                    tl::types::InputMediaPoll {
                        poll: tl::types::Poll {
                            id: 0,
                            closed: true,
                            public_voters: false,
                            multiple_choice: false,
                            quiz: false,
                            question: tl::types::TextWithEntities {
                                text: String::new(),
                                entities: vec![],
                            }
                            .into(),
                            answers: vec![],
                            close_period: None,
                            close_date: None,
                        }
                        .into(),
                        correct_answers: None,
                        solution: None,
                        solution_entities: None,
                    }
                    .into(),
                ),
                reply_markup: None,
                entities: None,
                schedule_date: None,
                schedule_repeat_period: None,
                quick_reply_shortcut_id: None,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_send_dice(
        &self,
        chat_id: ChatId,
        emoji: DiceEmoji,
    ) -> Result<SentMessage, ApiError> {
        let peer = self.resolve(chat_id)?;
        let media: tl::enums::InputMedia = tl::types::InputMediaDice {
            emoticon: emoji.as_str().to_string(),
        }
        .into();
        let msg = InputMessage::new().media(media);
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

    pub(crate) async fn impl_send_contact(
        &self,
        chat_id: ChatId,
        contact: Contact,
    ) -> Result<SentMessage, ApiError> {
        let peer = self.resolve(chat_id)?;
        let media: tl::enums::InputMedia = tl::types::InputMediaContact {
            phone_number: contact.phone_number,
            first_name: contact.first_name,
            last_name: contact.last_name.unwrap_or_default(),
            vcard: contact.vcard.unwrap_or_default(),
        }
        .into();
        let msg = InputMessage::new().media(media);
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

    pub(crate) async fn impl_send_venue(
        &self,
        chat_id: ChatId,
        venue: Venue,
    ) -> Result<SentMessage, ApiError> {
        let peer = self.resolve(chat_id)?;
        let media: tl::enums::InputMedia = tl::types::InputMediaVenue {
            geo_point: tl::types::InputGeoPoint {
                lat: venue.latitude,
                long: venue.longitude,
                accuracy_radius: None,
            }
            .into(),
            title: venue.title,
            address: venue.address,
            provider: "foursquare".to_string(),
            venue_id: venue.foursquare_id.unwrap_or_default(),
            venue_type: venue.foursquare_type.unwrap_or_default(),
        }
        .into();
        let msg = InputMessage::new().media(media);
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

    pub(crate) async fn impl_send_sticker(
        &self,
        chat_id: ChatId,
        sticker: FileSource,
    ) -> Result<SentMessage, ApiError> {
        self.impl_send_message(
            chat_id,
            MessageContent::Sticker { source: sticker },
            SendOptions::default(),
        )
        .await
    }

    pub(crate) async fn impl_send_location(
        &self,
        chat_id: ChatId,
        latitude: f64,
        longitude: f64,
    ) -> Result<SentMessage, ApiError> {
        self.impl_send_message(
            chat_id,
            MessageContent::Location {
                latitude,
                longitude,
                keyboard: None,
            },
            SendOptions::default(),
        )
        .await
    }
}
