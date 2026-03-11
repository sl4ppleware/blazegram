//! Ergonomic macros for handler registration.
//!
//! Eliminates the `Box::pin(async move { ... })` boilerplate that plagues
//! every handler definition.
//!
//! # Before (verbose)
//! ```ignore
//! .command("start", |ctx| Box::pin(async move {
//!     ctx.navigate(Screen::text("home", "Hello").build()).await
//! }))
//! ```
//!
//! # After (with macros)
//! ```ignore
//! .command("start", handler!(ctx => {
//!     ctx.navigate(Screen::text("home", "Hello").build()).await
//! }))
//! ```

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
