use serde::{Deserialize, Serialize};

use super::chat::UserInfo;
use super::content::{FileSource, ParseMode};

// ─── Poll ───

/// Configuration for sending a native Telegram poll.
///
/// Build one and pass to [`BotApi::send_poll`](crate::bot_api::BotApi::send_poll).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPoll {
    /// The poll question (1–300 characters).
    pub question: String,
    /// Answer options (2–10 strings, each 1–100 characters).
    pub options: Vec<String>,
    /// Whether voters are anonymous (default `true`).
    pub is_anonymous: bool,
    /// Regular poll or quiz (single correct answer).
    pub poll_type: PollType,
    /// For quiz polls: zero-based index of the correct option.
    pub correct_option_id: Option<usize>,
    /// Explanation shown after the user answers a quiz poll (0–200 chars).
    pub explanation: Option<String>,
    /// Time in seconds the poll will be active (5–600), or `None` for unlimited.
    pub open_period: Option<i32>,
    /// Whether the poll allows selecting multiple answers.
    pub allows_multiple_answers: bool,
}

/// Whether a poll is a regular vote or a quiz with one correct answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PollType {
    /// Regular poll — users pick one or more options.
    Regular,
    /// Quiz — exactly one correct answer, shown after voting.
    Quiz,
}

impl Default for SendPoll {
    fn default() -> Self {
        Self {
            question: String::new(),
            options: Vec::new(),
            is_anonymous: true,
            poll_type: PollType::Regular,
            correct_option_id: None,
            explanation: None,
            open_period: None,
            allows_multiple_answers: false,
        }
    }
}

// ─── Dice ───

/// Dice emoji type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum DiceEmoji {
    /// 🎲 standard die (values 1–6).
    #[default]
    Dice,
    /// 🎯 darts (values 1–6).
    Darts,
    /// 🏀 basketball (values 1–5).
    Basketball,
    /// ⚽ football (values 1–5).
    Football,
    /// 🎰 slot machine (values 1–64).
    SlotMachine,
    /// 🎳 bowling (values 1–6).
    Bowling,
}

impl DiceEmoji {
    /// Returns the emoji character that Telegram expects in the API call.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dice => "\u{1f3b2}",
            Self::Darts => "\u{1f3af}",
            Self::Basketball => "\u{1f3c0}",
            Self::Football => "\u{26bd}",
            Self::SlotMachine => "\u{1f3b0}",
            Self::Bowling => "\u{1f3b3}",
        }
    }
}

// ─── Contact ───

/// A shared contact card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contact {
    /// Phone number in international format.
    pub phone_number: String,
    /// Contact's first name.
    pub first_name: String,
    /// Contact's last name.
    pub last_name: Option<String>,
    /// Telegram user ID of the contact, if known.
    pub user_id: Option<u64>,
    /// vCard string with additional info.
    pub vcard: Option<String>,
}

// ─── Venue ───

/// A venue (place) with coordinates and address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Venue {
    /// Latitude in degrees.
    pub latitude: f64,
    /// Longitude in degrees.
    pub longitude: f64,
    /// Name of the venue.
    pub title: String,
    /// Street address.
    pub address: String,
    /// Foursquare venue identifier.
    pub foursquare_id: Option<String>,
    /// Foursquare venue type (e.g. `"arts_entertainment/default"`).
    pub foursquare_type: Option<String>,
}

// ─── Chat Member ───

/// A user's membership record in a chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMember {
    /// The user.
    pub user: UserInfo,
    /// Their current status in the chat.
    pub status: ChatMemberStatus,
}

/// Possible membership statuses in a Telegram chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatMemberStatus {
    /// Chat owner (has all permissions, cannot be restricted).
    Creator,
    /// Promoted administrator.
    Administrator,
    /// Regular member.
    Member,
    /// Restricted member (some permissions revoked).
    Restricted,
    /// Not in the chat (left voluntarily).
    Left,
    /// Banned from the chat.
    Banned,
}

impl ChatMemberStatus {
    /// Returns `true` for [`Creator`](Self::Creator) and [`Administrator`](Self::Administrator).
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Creator | Self::Administrator)
    }

    /// Returns `true` if the user is currently in the chat
    /// (creator, admin, member, or restricted).
    pub fn is_member(&self) -> bool {
        matches!(
            self,
            Self::Creator | Self::Administrator | Self::Member | Self::Restricted
        )
    }
}

// ─── Chat Info ───

/// Full information about a Telegram chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    /// Chat identifier.
    pub id: super::ChatId,
    /// Type of chat (private, group, supergroup, channel).
    pub chat_type: ChatType,
    /// Chat title (groups, supergroups, channels).
    pub title: Option<String>,
    /// `@username` of the chat (if set).
    pub username: Option<String>,
    /// First name (private chats only).
    pub first_name: Option<String>,
    /// Last name (private chats only).
    pub last_name: Option<String>,
    /// Number of members (may require admin privileges to read).
    pub member_count: Option<i32>,
}

/// The type of a Telegram chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatType {
    /// One-on-one conversation with the bot.
    Private,
    /// Classic group (up to 200 members, no supergroup features).
    Group,
    /// Supergroup (up to 200 000 members, persistent history, admin tools).
    Supergroup,
    /// Broadcast channel.
    Channel,
}

// ─── Bot Command ───

/// A bot command entry for the menu / command list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BotCommand {
    /// The command string without the leading `/` (e.g. `"start"`).
    pub command: String,
    /// Human-readable description (1–256 characters).
    pub description: String,
}

// ─── Permissions ───

/// Fine-grained chat permissions that can be set per-user or for the whole group.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatPermissions {
    /// Can send text messages, contacts, invoices, locations, venues.
    pub can_send_messages: Option<bool>,
    /// Can send audios, documents, photos, videos, video notes, voice notes.
    pub can_send_media_messages: Option<bool>,
    /// Can send polls.
    pub can_send_polls: Option<bool>,
    /// Can send stickers, GIFs, games, use inline bots.
    pub can_send_other_messages: Option<bool>,
    /// Can add web page previews to messages.
    pub can_add_web_page_previews: Option<bool>,
    /// Can change the chat title, photo, and other settings.
    pub can_change_info: Option<bool>,
    /// Can invite new users to the chat.
    pub can_invite_users: Option<bool>,
    /// Can pin messages (supergroups only).
    pub can_pin_messages: Option<bool>,
}

// ─── Invoice / Payment ───

/// A Telegram Payments invoice ready to be sent.
///
/// Use `provider_token: None` + `currency: "XTR"` for Telegram Stars payments.
#[derive(Debug, Clone)]
pub struct Invoice {
    /// Product name (1–32 characters).
    pub title: String,
    /// Product description (1–255 characters).
    pub description: String,
    /// Bot-defined payload (not shown to the user, returned in callbacks).
    pub payload: String,
    /// Payment provider token from @BotFather. `None` for Stars.
    pub provider_token: Option<String>,
    /// Three-letter ISO 4217 currency code (`"USD"`, `"EUR"`, `"XTR"` for Stars).
    pub currency: String,
    /// Price breakdown: `Vec<(label, amount)>` where amount is in the smallest
    /// currency unit (e.g. cents for USD, Stars for XTR).
    pub prices: Vec<(String, i64)>,
    /// Deep link parameter for the `/start` command when paying via link.
    pub start_parameter: Option<String>,
    /// URL of the product photo for the invoice.
    pub photo_url: Option<String>,
    /// Request the user's full name.
    pub need_name: bool,
    /// Request the user's phone number.
    pub need_phone_number: bool,
    /// Request the user's email address.
    pub need_email: bool,
    /// Request the user's shipping address.
    pub need_shipping_address: bool,
    /// Whether the final price depends on the shipping method.
    pub is_flexible: bool,
}

// ─── Media Group Item ───

/// A single item in a media group (album).
///
/// Send a `Vec<MediaGroupItem>` via
/// [`BotApi::send_media_group`](crate::bot_api::BotApi::send_media_group).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaGroupItem {
    /// A photo in the album.
    Photo {
        /// Image source.
        source: FileSource,
        /// Optional caption (only the first item's caption is shown by Telegram).
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
        /// Send under a spoiler overlay.
        spoiler: bool,
    },
    /// A video in the album.
    Video {
        /// Video source.
        source: FileSource,
        /// Optional caption.
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
        /// Send under a spoiler overlay.
        spoiler: bool,
    },
    /// A document in the album.
    Document {
        /// Document source.
        source: FileSource,
        /// Optional caption.
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
    },
    /// An audio track in the album.
    Audio {
        /// Audio source.
        source: FileSource,
        /// Optional caption.
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
    },
}

// ─── Downloaded File ───

/// Raw bytes of a file downloaded from Telegram servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadedFile {
    /// The file's raw bytes.
    pub data: Vec<u8>,
    /// File size in bytes, if reported by Telegram.
    pub file_size: Option<usize>,
}

// ─── Bot Info ───

/// Information about the bot itself, returned by `get_me`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInfo {
    /// Bot's user ID.
    pub id: super::UserId,
    /// Bot's `@username` (without the `@`).
    pub username: String,
    /// Bot's display name.
    pub first_name: String,
    /// Whether the bot can be added to groups.
    pub can_join_groups: bool,
    /// Whether the bot has [privacy mode](https://core.telegram.org/bots/features#privacy-mode) disabled.
    pub can_read_all_group_messages: bool,
    /// Whether the bot supports inline queries.
    pub supports_inline_queries: bool,
}

// ─── User Profile Photos ───

/// A user's profile photo collection.
#[derive(Debug, Clone)]
pub struct UserProfilePhotos {
    /// Total number of profile photos.
    pub total_count: i32,
    /// File IDs of the photos (largest size per photo).
    pub photos: Vec<String>,
}

// ─── Chat Invite Link ───

/// A chat invite link created by the bot or an admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInviteLink {
    /// The invite link URL.
    pub invite_link: String,
    /// Who created this link.
    pub creator: Option<UserInfo>,
    /// Whether users joining via this link need admin approval.
    pub creates_join_request: bool,
    /// Whether this is the chat's primary invite link.
    pub is_primary: bool,
    /// Whether the link has been revoked.
    pub is_revoked: bool,
    /// Human-readable link name.
    pub name: Option<String>,
    /// Unix timestamp when the link expires.
    pub expire_date: Option<i64>,
    /// Maximum number of users that can join via this link.
    pub member_limit: Option<i32>,
    /// Number of pending join requests using this link.
    pub pending_join_request_count: Option<i32>,
}

// ─── Menu Button ───

/// The bot's menu button in the chat input area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MenuButton {
    /// Default behavior (Telegram decides).
    Default,
    /// Open bot's list of commands.
    Commands,
    /// Open a Web App.
    WebApp {
        /// Button label.
        text: String,
        /// Web App URL.
        url: String,
    },
}

// ─── Bot Description ───

/// The bot's long description (shown in the bot's profile and on the start screen).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotDescription {
    /// The description text.
    pub description: String,
}

/// The bot's short description (shown in inline mode search results).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotShortDescription {
    /// The short description text.
    pub short_description: String,
}

/// The bot's display name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotName {
    /// The bot's name.
    pub name: String,
}

// ─── Shipping Query ───

/// A shipping option offered to the buyer during Telegram Payments checkout.
#[derive(Debug, Clone)]
pub struct ShippingOption {
    /// Unique option identifier.
    pub id: String,
    /// Human-readable option title.
    pub title: String,
    /// Prices in smallest units: `Vec<(label, amount)>`.
    pub prices: Vec<(String, i64)>,
}

// ─── Forum Topics ───

/// Represents a forum topic in a supergroup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForumTopic {
    /// Topic ID (same as the `message_id` of the topic's creation service message).
    pub id: i32,
    /// Topic name.
    pub title: String,
    /// RGB icon color as a 24-bit integer, or `None` for custom emoji icons.
    pub icon_color: Option<i32>,
    /// Custom emoji ID used as the topic icon.
    pub icon_custom_emoji_id: Option<String>,
    /// Whether the topic is closed (no new messages).
    pub is_closed: bool,
    /// Whether the topic is hidden from the topic list (only General topic can be hidden).
    pub is_hidden: bool,
}

// ─── Stars / Payments (extended) ───

/// A Telegram Stars transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarTransaction {
    /// Unique transaction identifier.
    pub id: String,
    /// Amount in Stars (can be negative for outbound).
    pub amount: i64,
    /// Fractional nanos (for sub-star precision).
    pub nanos: i32,
    /// Unix timestamp of the transaction.
    pub date: i32,
    /// The other party in the transaction.
    pub source: StarTransactionPeer,
    /// Product / purchase title.
    pub title: Option<String>,
    /// Product description.
    pub description: Option<String>,
    /// Whether this transaction is a refund of a previous one.
    pub is_refund: bool,
}

/// Who is on the other side of a star transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StarTransactionPeer {
    /// A Telegram user paid or was refunded.
    User(super::UserId),
    /// Transaction through the App Store.
    AppStore,
    /// Transaction through Google Play.
    PlayMarket,
    /// Transaction through Fragment.
    Fragment,
    /// Telegram Premium bot subscription.
    PremiumBot,
    /// Telegram Ads platform.
    Ads,
    /// Bot API (e.g. `refundStarPayment`).
    Api,
    /// Source not recognized by this version of blazegram.
    Unknown,
}

/// The bot's current Telegram Stars balance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarBalance {
    /// Whole Stars amount.
    pub amount: i64,
    /// Fractional nanos (for sub-star precision).
    pub nanos: i32,
}

/// Response from `get_star_transactions`.
#[derive(Debug, Clone)]
pub struct StarTransactions {
    /// Current balance at the time of the query.
    pub balance: StarBalance,
    /// List of transactions.
    pub transactions: Vec<StarTransaction>,
    /// Offset for the next page, or `None` if there are no more.
    pub next_offset: Option<String>,
}

// ─── Paid Media ───

/// A single media item for paid media messages (Telegram Stars).
///
/// Used with [`BotApi::send_paid_media`](crate::bot_api::BotApi::send_paid_media).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaidMediaInput {
    /// A photo to be sent as paid media.
    Photo {
        /// Photo source (file ID, URL, local path, or raw bytes).
        source: FileSource,
    },
    /// A video to be sent as paid media.
    Video {
        /// Video source.
        source: FileSource,
        /// Duration of the video in seconds.
        duration: Option<i32>,
        /// Video width.
        width: Option<i32>,
        /// Video height.
        height: Option<i32>,
        /// Whether the video needs to support streaming.
        supports_streaming: Option<bool>,
    },
}

// ─── Checklist (Bot API 9.5+) ───

/// An item in a checklist message (Bot API 9.5+).
///
/// Used with [`BotApi::send_checklist`](crate::bot_api::BotApi::send_checklist)
/// and [`BotApi::edit_message_checklist`](crate::bot_api::BotApi::edit_message_checklist).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    /// The text of the checklist item.
    pub text: String,
    /// Whether this item is checked / completed.
    pub checked: bool,
}

// ─── User Profile Audios (Bot API 9.4+) ───

/// Response from [`BotApi::get_user_profile_audios`](crate::bot_api::BotApi::get_user_profile_audios).
///
/// Contains audio files set as the user's profile audio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfileAudios {
    /// Total number of profile audios the user has.
    pub total_count: i32,
    /// Requested audio file IDs.
    pub audios: Vec<String>,
}

// ─── Business ───

/// Describes a connection of the bot with a business account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessConnection {
    /// Unique identifier of the business connection.
    pub id: String,
    /// Business account user.
    pub user: UserInfo,
    /// Identifier of a private chat with the user.
    pub user_chat_id: super::ChatId,
    /// Date the connection was established, Unix time.
    pub date: i64,
    /// Whether the bot can act on behalf of the business account in chats.
    pub can_reply: bool,
    /// Whether the connection is active.
    pub is_enabled: bool,
}

/// Describes the types of gifts accepted by a business account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedGiftTypes {
    /// Whether unlimited regular gifts are accepted.
    pub unlimited_gifts: bool,
    /// Whether limited regular gifts are accepted.
    pub limited_gifts: bool,
    /// Whether unique gifts are accepted.
    pub unique_gifts: bool,
    /// Whether premium subscriptions are accepted.
    pub premium_subscription: bool,
}

// ─── Gifts ───

/// Represents a gift that can be sent by the bot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gift {
    /// Unique identifier of the gift.
    pub id: String,
    /// The sticker that represents the gift.
    pub sticker_file_id: String,
    /// The number of Stars that must be paid to send the gift.
    pub star_count: i64,
    /// The total number of gifts of this type that can be sent (for limited gifts).
    pub total_count: Option<i64>,
    /// The number of remaining gifts of this type (for limited gifts).
    pub remaining_count: Option<i64>,
}

/// Represents a gift owned by a user or chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedGift {
    /// The gift ID.
    pub gift_id: String,
    /// Unique identifier of this owned gift instance.
    pub owned_gift_id: String,
    /// Sender user info, if not anonymous.
    pub sender_user: Option<UserInfo>,
    /// Text message attached to the gift.
    pub text: Option<String>,
    /// Whether the gift is saved (displayed on profile).
    pub is_saved: bool,
    /// Whether the gift is sold (converted to Stars).
    pub is_sold: bool,
    /// Number of Stars that can be received by converting the gift.
    pub convert_star_count: Option<i64>,
    /// Number of Stars that can be received by upgrading the gift.
    pub upgrade_star_count: Option<i64>,
    /// Date the gift was sent, Unix time.
    pub date: i64,
}

/// Response containing a list of owned gifts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedGifts {
    /// Total number of gifts owned by the entity.
    pub total_count: i32,
    /// The list of gifts.
    pub gifts: Vec<OwnedGift>,
    /// Offset for the next request, or `None` if there are no more.
    pub next_offset: Option<String>,
}

// ─── Stories ───

/// Content of a story to be posted.
#[derive(Debug, Clone)]
pub enum StoryContent {
    /// A photo story.
    Photo {
        /// Photo source.
        photo: FileSource,
    },
    /// A video story.
    Video {
        /// Video source.
        video: FileSource,
        /// Duration in seconds.
        duration: Option<f64>,
    },
}

/// A posted story.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    /// Unique identifier of the story in the chat.
    pub id: i32,
    /// Chat that posted the story.
    pub chat_id: super::ChatId,
    /// Date the story was posted, Unix time.
    pub date: i64,
}

// ─── User Chat Boosts ───

/// Represents boosts applied by a user to a chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserChatBoosts {
    /// The list of boosts applied by the user to the chat.
    pub boosts: Vec<ChatBoost>,
}

/// A single chat boost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatBoost {
    /// Unique identifier of the boost.
    pub boost_id: String,
    /// Point in time (Unix timestamp) when the chat was boosted.
    pub add_date: i64,
    /// Point in time (Unix timestamp) when the boost will expire.
    pub expiration_date: i64,
    /// Source of the boost.
    pub source: ChatBoostSource,
}

/// Source of a chat boost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatBoostSource {
    /// Boost from a Telegram Premium subscription.
    Premium {
        /// The user who boosted.
        user: UserInfo,
    },
    /// Boost from a gifted Premium subscription.
    GiftCode {
        /// The user who boosted.
        user: UserInfo,
    },
    /// Boost from a giveaway.
    Giveaway {
        /// The giveaway message ID.
        giveaway_message_id: Option<i32>,
        /// The user, if known.
        user: Option<UserInfo>,
        /// Whether the boost was unclaimed.
        is_unclaimed: bool,
    },
}

// ─── Prepared Keyboard Button ───

/// Data for a keyboard button to be prepared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedKeyboardButtonData {
    /// The type of button.
    pub button_type: PreparedKeyboardButtonType,
}

/// Type of prepared keyboard button.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreparedKeyboardButtonType {
    /// Request users.
    RequestUsers {
        /// Identifier of the request.
        request_id: i32,
        /// Whether the user must be a bot.
        user_is_bot: Option<bool>,
        /// Whether the user must be a Premium user.
        user_is_premium: Option<bool>,
        /// Maximum number of users to select.
        max_quantity: Option<i32>,
    },
    /// Request a chat.
    RequestChat {
        /// Identifier of the request.
        request_id: i32,
        /// Whether the chat must be a channel.
        chat_is_channel: bool,
    },
}

/// A prepared keyboard button stored by Telegram.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedKeyboardButton {
    /// Unique identifier of the prepared button.
    pub id: String,
    /// Expiration date, Unix time. The button can be used until this date.
    pub expiration_date: i64,
}

// ─── Sticker Types ───

/// Format of a sticker (static, animated, or video).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StickerFormat {
    /// PNG image (`image/png`).
    Static,
    /// TGS animation (`application/x-tgsticker`).
    Animated,
    /// WEBM video (`video/webm`).
    Video,
}

/// Type of a sticker set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StickerType {
    /// Normal stickers.
    Regular,
    /// Mask stickers that overlay on photos.
    Mask,
    /// Custom emoji stickers usable in messages.
    CustomEmoji,
}

/// Position on a face for mask stickers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MaskPosition {
    /// Part of the face.
    pub point: MaskPoint,
    /// Shift by X-axis measured in widths of the mask scaled to the face size (−1.0 to 1.0).
    pub x_shift: f64,
    /// Shift by Y-axis measured in heights of the mask scaled to the face size (−1.0 to 1.0).
    pub y_shift: f64,
    /// Mask scaling coefficient (0.0–2.0).
    pub scale: f64,
}

/// Face part for [`MaskPosition`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaskPoint {
    /// Forehead area.
    Forehead,
    /// Eyes area.
    Eyes,
    /// Mouth area.
    Mouth,
    /// Chin area.
    Chin,
}

/// Info about an individual sticker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerInfo {
    /// Unique file identifier.
    pub file_id: String,
    /// Unique file identifier (stable across re-uploads).
    pub file_unique_id: String,
    /// Sticker type.
    pub sticker_type: StickerType,
    /// Sticker width.
    pub width: i32,
    /// Sticker height.
    pub height: i32,
    /// Whether the sticker is animated (TGS).
    pub is_animated: bool,
    /// Whether the sticker is a video (WEBM).
    pub is_video: bool,
    /// Emoji associated with this sticker.
    pub emoji: Option<String>,
    /// Name of the sticker set this belongs to.
    pub set_name: Option<String>,
    /// Mask position data (for mask stickers).
    pub mask_position: Option<MaskPosition>,
    /// Custom emoji identifier (for custom_emoji stickers).
    pub custom_emoji_id: Option<String>,
    /// File size in bytes.
    pub file_size: Option<i64>,
}

/// A sticker set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerSet {
    /// Sticker set name (used in URLs: `t.me/addstickers/<name>`).
    pub name: String,
    /// Human-readable title.
    pub title: String,
    /// Type of sticker set.
    pub sticker_type: StickerType,
    /// List of stickers in the set.
    pub stickers: Vec<StickerInfo>,
}

/// A sticker to be added to a sticker set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSticker {
    /// The sticker file.
    pub sticker: FileSource,
    /// Format of the sticker.
    pub format: StickerFormat,
    /// List of 1–20 emoji associated with the sticker.
    pub emoji_list: Vec<String>,
    /// Position for mask stickers.
    pub mask_position: Option<MaskPosition>,
    /// List of 0–20 search keywords (regular and custom emoji stickers only).
    pub keywords: Vec<String>,
}

/// A file stored on the Telegram servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramFile {
    /// Unique file identifier.
    pub file_id: String,
    /// Unique file identifier (stable across re-uploads).
    pub file_unique_id: String,
    /// File size in bytes, if known.
    pub file_size: Option<i64>,
    /// File path for downloading via `https://api.telegram.org/file/bot<token>/<file_path>`.
    pub file_path: Option<String>,
}

// ─── Game Types ───

/// A row in a game high score table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameHighScore {
    /// Position in the high score table (1-based).
    pub position: i32,
    /// User who achieved the score.
    pub user: UserInfo,
    /// Score value.
    pub score: i64,
}

// ─── Inline Extras ───

/// Describes an inline message sent by a Web App on behalf of a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentWebAppMessage {
    /// Identifier of the sent inline message (available only if there is an inline keyboard).
    pub inline_message_id: Option<String>,
}

/// Describes a prepared inline message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedInlineMessage {
    /// Unique identifier of the prepared message.
    pub id: String,
    /// Expiration date of the prepared message (Unix timestamp).
    pub expiration_date: i64,
}

// ─── Passport Types ───

/// Represents an error in a Telegram Passport element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PassportElementError {
    /// Error in a data field.
    DataField {
        /// Section type (e.g. `"personal_details"`, `"passport"`).
        element_type: String,
        /// Name of the data field with the error.
        field_name: String,
        /// Base64-encoded data hash.
        data_hash: String,
        /// Error message.
        message: String,
    },
    /// Error in the front side of a document.
    FrontSide {
        /// Section type.
        element_type: String,
        /// Base64-encoded file hash.
        file_hash: String,
        /// Error message.
        message: String,
    },
    /// Error in the reverse side of a document.
    ReverseSide {
        /// Section type.
        element_type: String,
        /// Base64-encoded file hash.
        file_hash: String,
        /// Error message.
        message: String,
    },
    /// Error in a selfie with a document.
    Selfie {
        /// Section type.
        element_type: String,
        /// Base64-encoded file hash.
        file_hash: String,
        /// Error message.
        message: String,
    },
    /// Error in an uploaded file.
    File {
        /// Section type.
        element_type: String,
        /// Base64-encoded file hash.
        file_hash: String,
        /// Error message.
        message: String,
    },
    /// Error in multiple uploaded files.
    Files {
        /// Section type.
        element_type: String,
        /// Base64-encoded file hashes.
        file_hashes: Vec<String>,
        /// Error message.
        message: String,
    },
    /// Error in a translation file.
    TranslationFile {
        /// Section type.
        element_type: String,
        /// Base64-encoded file hash.
        file_hash: String,
        /// Error message.
        message: String,
    },
    /// Error in translation files.
    TranslationFiles {
        /// Section type.
        element_type: String,
        /// Base64-encoded file hashes.
        file_hashes: Vec<String>,
        /// Error message.
        message: String,
    },
    /// Unspecified issue.
    Unspecified {
        /// Section type.
        element_type: String,
        /// Base64-encoded element hash.
        element_hash: String,
        /// Error message.
        message: String,
    },
}
