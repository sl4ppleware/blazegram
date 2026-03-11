//! Bot settings: commands, descriptions, name, menu button.

use grammers_client::tl;

use super::GrammersAdapter;
use crate::error::ApiError;
use crate::types::*;

impl GrammersAdapter {
    pub(crate) async fn impl_set_my_commands(
        &self,
        commands: Vec<BotCommand>,
    ) -> Result<(), ApiError> {
        let tl_commands: Vec<tl::enums::BotCommand> = commands
            .into_iter()
            .map(|c| {
                tl::types::BotCommand {
                    command: c.command,
                    description: c.description,
                }
                .into()
            })
            .collect();
        self.client
            .invoke(&tl::functions::bots::SetBotCommands {
                scope: tl::types::BotCommandScopeDefault {}.into(),
                lang_code: String::new(),
                commands: tl_commands,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_delete_my_commands(&self) -> Result<(), ApiError> {
        self.client
            .invoke(&tl::functions::bots::ResetBotCommands {
                scope: tl::types::BotCommandScopeDefault {}.into(),
                lang_code: String::new(),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_get_me(&self) -> Result<BotInfo, ApiError> {
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

    pub(crate) async fn impl_get_my_commands(&self) -> Result<Vec<BotCommand>, ApiError> {
        let result = self
            .client
            .invoke(&tl::functions::bots::GetBotCommands {
                scope: tl::types::BotCommandScopeDefault {}.into(),
                lang_code: String::new(),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(result
            .into_iter()
            .map(|cmd| {
                let tl::enums::BotCommand::Command(c) = cmd;
                BotCommand {
                    command: c.command,
                    description: c.description,
                }
            })
            .collect())
    }

    pub(crate) async fn impl_set_my_description(
        &self,
        description: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.client
            .invoke(&tl::functions::bots::SetBotInfo {
                bot: None,
                lang_code: language_code.unwrap_or("").to_string(),
                name: None,
                about: description.map(|s| s.to_string()),
                description: None,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_get_my_description(
        &self,
        language_code: Option<&str>,
    ) -> Result<BotDescription, ApiError> {
        let result = self
            .client
            .invoke(&tl::functions::bots::GetBotInfo {
                bot: None,
                lang_code: language_code.unwrap_or("").to_string(),
            })
            .await
            .map_err(Self::convert_error)?;
        let tl::enums::bots::BotInfo::Info(info) = result;
        Ok(BotDescription {
            description: info.about,
        })
    }

    pub(crate) async fn impl_set_my_short_description(
        &self,
        short_description: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.client
            .invoke(&tl::functions::bots::SetBotInfo {
                bot: None,
                lang_code: language_code.unwrap_or("").to_string(),
                name: None,
                about: None,
                description: short_description.map(|s| s.to_string()),
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_get_my_short_description(
        &self,
        language_code: Option<&str>,
    ) -> Result<BotShortDescription, ApiError> {
        let result = self
            .client
            .invoke(&tl::functions::bots::GetBotInfo {
                bot: None,
                lang_code: language_code.unwrap_or("").to_string(),
            })
            .await
            .map_err(Self::convert_error)?;
        let tl::enums::bots::BotInfo::Info(info) = result;
        Ok(BotShortDescription {
            short_description: info.description,
        })
    }

    pub(crate) async fn impl_set_my_name(
        &self,
        name: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.client
            .invoke(&tl::functions::bots::SetBotInfo {
                bot: None,
                lang_code: language_code.unwrap_or("").to_string(),
                name: name.map(|s| s.to_string()),
                about: None,
                description: None,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_get_my_name(
        &self,
        language_code: Option<&str>,
    ) -> Result<BotName, ApiError> {
        let result = self
            .client
            .invoke(&tl::functions::bots::GetBotInfo {
                bot: None,
                lang_code: language_code.unwrap_or("").to_string(),
            })
            .await
            .map_err(Self::convert_error)?;
        let tl::enums::bots::BotInfo::Info(info) = result;
        Ok(BotName { name: info.name })
    }

    pub(crate) async fn impl_set_chat_menu_button(
        &self,
        chat_id: Option<ChatId>,
        menu_button: MenuButton,
    ) -> Result<(), ApiError> {
        let user = if let Some(cid) = chat_id {
            let peer = self.resolve(cid)?;
            tl::types::InputUser {
                user_id: peer.id.bare_id(),
                access_hash: peer.auth.hash(),
            }
            .into()
        } else {
            tl::types::InputUserEmpty {}.into()
        };

        let button: tl::enums::BotMenuButton = match menu_button {
            MenuButton::Default => tl::types::BotMenuButtonDefault {}.into(),
            MenuButton::Commands => tl::types::BotMenuButtonCommands {}.into(),
            MenuButton::WebApp { text, url } => tl::types::BotMenuButton { text, url }.into(),
        };

        self.client
            .invoke(&tl::functions::bots::SetBotMenuButton {
                user_id: user,
                button,
            })
            .await
            .map_err(Self::convert_error)?;
        Ok(())
    }

    pub(crate) async fn impl_get_chat_menu_button(
        &self,
        chat_id: Option<ChatId>,
    ) -> Result<MenuButton, ApiError> {
        let user = if let Some(cid) = chat_id {
            let peer = self.resolve(cid)?;
            tl::types::InputUser {
                user_id: peer.id.bare_id(),
                access_hash: peer.auth.hash(),
            }
            .into()
        } else {
            tl::types::InputUserEmpty {}.into()
        };

        let result = self
            .client
            .invoke(&tl::functions::bots::GetBotMenuButton { user_id: user })
            .await
            .map_err(Self::convert_error)?;

        Ok(match result {
            tl::enums::BotMenuButton::Button(b) => MenuButton::WebApp {
                text: b.text,
                url: b.url,
            },
            tl::enums::BotMenuButton::Commands => MenuButton::Commands,
            tl::enums::BotMenuButton::Default => MenuButton::Default,
        })
    }
}
