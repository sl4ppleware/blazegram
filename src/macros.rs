//! Ergonomic macros for handler registration and BotApi delegation.
//!
//! ## Handler macros
//!
//! Eliminates the `Box::pin(async move { ... })` boilerplate that plagues
//! every handler definition.
//!
//! ### Before (verbose)
//! ```ignore
//! .command("start", |ctx| Box::pin(async move {
//!     ctx.navigate(Screen::text("home", "Hello").build()).await
//! }))
//! ```
//!
//! ### After (with macros)
//! ```ignore
//! .command("start", handler!(ctx => {
//!     ctx.navigate(Screen::text("home", "Hello").build()).await
//! }))
//! ```
//!
//! ## BotApi impl generators
//!
//! Each macro generates a complete `#[async_trait] impl BotApi for X { ... }` block.
//! This works because macro_rules expand BEFORE proc macros on the generated items.

/// Create a handler closure that wraps the body in `Box::pin(async move { ... })`.
///
/// # Variants
///
/// ```ignore
/// // 1-arg: command, callback, on_unrecognized
/// handler!(ctx => { ctx.navigate(...).await })
///
/// // 2-arg: on_input, on_any_text, on_message_edited
/// handler!(ctx, text => { ctx.navigate(...).await })
///
/// // 3-arg: on_inline
/// handler!(ctx, query, offset => { ... })
/// ```
#[macro_export]
macro_rules! handler {
    // 1-arg handler: |ctx| -> Pin<Box<Future>>
    ($ctx:ident => $body:expr) => {
        |$ctx: &mut $crate::ctx::Ctx| ::std::boxed::Box::pin(async move { $body })
    };
    // 2-arg handler: |ctx, arg| -> Pin<Box<Future>>
    ($ctx:ident, $arg:ident => $body:expr) => {
        |$ctx: &mut $crate::ctx::Ctx, $arg| ::std::boxed::Box::pin(async move { $body })
    };
    // 3-arg handler: |ctx, arg1, arg2| -> Pin<Box<Future>>
    ($ctx:ident, $arg1:ident, $arg2:ident => $body:expr) => {
        |$ctx: &mut $crate::ctx::Ctx, $arg1, $arg2| ::std::boxed::Box::pin(async move { $body })
    };
}

/// Create a form completion handler.
///
/// ```ignore
/// .on_complete(form_handler!(ctx, data => {
///     ctx.navigate(Screen::text("done", "Thanks!").build()).await
/// }))
/// ```
#[macro_export]
macro_rules! form_handler {
    ($ctx:ident, $data:ident => $body:expr) => {
        |$ctx: &mut $crate::ctx::Ctx, $data: $crate::form::FormData| {
            ::std::boxed::Box::pin(async move { $body })
        }
    };
}

// ─── BotApi impl generators ───
//
// Each macro generates a complete `#[async_trait] impl BotApi for X { ... }` block.
// This works because macro_rules expand BEFORE proc macros on the generated items.

/// Generate `impl BotApi for RateLimitedBotApi<B>` with three method categories:
/// - `rate_limited`: per-chat + global token acquisition, retry on 429
/// - `bypass`: retry on 429 without consuming tokens
/// - `passthrough`: direct delegation, no rate limiting
macro_rules! impl_rate_limited_botapi {
    (
        rate_limited: [
            $(fn $rl_name:ident($rl_chat:ident: ChatId $(, $rl_arg:ident: $rl_ty:ty)*) -> $rl_ret:ty;)*
        ]
        bypass: [
            $(fn $bp_name:ident($($bp_arg:ident: $bp_ty:ty),*) -> $bp_ret:ty;)*
        ]
        passthrough: [
            $(fn $pt_name:ident($($pt_arg:ident: $pt_ty:ty),*) -> $pt_ret:ty;)*
        ]
    ) => {
        #[async_trait::async_trait]
        impl<B: BotApi> BotApi for RateLimitedBotApi<B> {
            $(
                async fn $rl_name(&self, $rl_chat: ChatId $(, $rl_arg: $rl_ty)*) -> Result<$rl_ret, ApiError> {
                    self.rate_limited_call(Some($rl_chat), || {
                        self.inner.$rl_name($rl_chat $(, $rl_arg.clone())*)
                    }).await
                }
            )*
            $(
                async fn $bp_name(&self, $($bp_arg: $bp_ty),*) -> Result<$bp_ret, ApiError> {
                    self.bypass_call(|| {
                        self.inner.$bp_name($($bp_arg.clone()),*)
                    }).await
                }
            )*
            $(
                async fn $pt_name(&self, $($pt_arg: $pt_ty),*) -> Result<$pt_ret, ApiError> {
                    self.inner.$pt_name($($pt_arg),*).await
                }
            )*
        }
    };
}

/// Generate `impl BotApi for GrammersAdapter` delegating each method to `self.impl_*`.
macro_rules! impl_adapter_botapi {
    (
        delegate: [
            $(fn $d_name:ident => $d_impl:ident($($d_arg:ident: $d_ty:ty),*) -> $d_ret:ty;)*
        ]
        manual: { $($manual:tt)* }
    ) => {
        #[async_trait::async_trait]
        impl BotApi for GrammersAdapter {
            $(
                async fn $d_name(&self, $($d_arg: $d_ty),*) -> Result<$d_ret, ApiError> {
                    self.$d_impl($($d_arg),*).await
                }
            )*
            $($manual)*
        }
    };
}

/// Generate `impl BotApi for MockBotApi` with auto-generated and manual methods.
macro_rules! impl_mock_botapi {
    (
        ok_unit: [
            $(fn $u_name:ident($($u_arg:ident: $u_ty:ty),*);)*
        ]
        ok_sent: [
            $(fn $s_name:ident($s_chat:ident: ChatId $(, $s_arg:ident: $s_ty:ty)*);)*
        ]
        manual: { $($manual:tt)* }
    ) => {
        #[async_trait::async_trait]
        impl BotApi for MockBotApi {
            $($manual)*
            $(
                async fn $u_name(&self, $($u_arg: $u_ty),*) -> Result<(), ApiError> {
                    let _ = ($(&$u_arg,)*);
                    Ok(())
                }
            )*
            $(
                async fn $s_name(&self, $s_chat: ChatId $(, $s_arg: $s_ty)*) -> Result<SentMessage, ApiError> {
                    let _ = ($(&$s_arg,)*);
                    Ok(SentMessage {
                        message_id: MessageId(self.next_id()),
                        chat_id: $s_chat,
                    })
                }
            )*
        }
    };
}
