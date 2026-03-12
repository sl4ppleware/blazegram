use serde::{Deserialize, Serialize};

use super::chat::UserInfo;
use super::content::{FileSource, ParseMode};

// ─── Poll ───

/// Configuration for sending a native Telegram poll.
///
/// Build one and pass to [`BotApi::send_poll`](crate::bot_api::BotApi::send_poll).
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
