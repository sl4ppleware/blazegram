//! Branching conversation system — multi-step dialogues with conditional flow.
//!
//! Conversations are a superset of [`Form`](crate::form::Form): they support
//! branching, unconditional jumps, and custom input handlers per step.
//!
//! ```rust,ignore
//! let conv = Conversation::builder("onboarding")
//!     .step("name", |_data, lang| {
//!         Screen::text("conv_name", "What's your name?").build()
//!     }, None)
//!     .step("role", |_data, lang| {
//!         Screen::text("conv_role", "Are you a student or teacher?").build()
//!     }, None)
//!     .branch("role", Arc::new(|data| {
//!         match data.get("role").and_then(|v| v.as_str()) {
//!             Some("student") => "student_year".to_string(),
//!             _ => "done".to_string(),
//!         }
//!     }))
//!     .step("student_year", |_data, lang| {
//!         Screen::text("conv_year", "What year are you in?").build()
//!     }, None)
//!     .step("done", |data, _lang| {
//!         Screen::text("conv_done", "All set!").build()
//!     }, None)
//!     .on_complete(Arc::new(|ctx, data| Box::pin(async move {
//!         ctx.navigate(Screen::text("home", "Welcome!").build()).await
//!     })))
//!     .build()
//!     .unwrap();
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::ctx::Ctx;
use crate::error::HandlerResult;
use crate::screen::Screen;

/// Collected conversation data (same type as FormData).
pub type ConversationData = HashMap<String, serde_json::Value>;

/// A screen-producing function for a conversation step.
pub type StepScreenFn = Arc<dyn Fn(&ConversationData, &str) -> Screen + Send + Sync>;

/// Handler that processes user input for a step and returns the field value, or `None` to retry.
pub type StepInputFn = Arc<
    dyn for<'a> Fn(
            &'a mut Ctx,
            &'a str,
            &'a ConversationData,
        ) -> Pin<
            Box<dyn Future<Output = Result<Option<serde_json::Value>, String>> + Send + 'a>,
        > + Send
        + Sync,
>;

/// Branch function — given collected data, returns the next step name.
pub type BranchFn = Arc<dyn Fn(&ConversationData) -> String + Send + Sync>;

/// Handler called when conversation completes.
pub type ConversationCompleteHandler = Arc<
    dyn Fn(&mut Ctx, ConversationData) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync,
>;

/// Handler called when conversation is cancelled.
pub type ConversationCancelHandler =
    Arc<dyn Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync>;

/// A single step in a conversation.
pub struct ConversationStep {
    pub(crate) name: String,
    pub(crate) screen_fn: StepScreenFn,
    pub(crate) input_fn: Option<StepInputFn>,
    pub(crate) next: StepNext,
}

/// How to determine the next step after this one completes.
pub(crate) enum StepNext {
    /// Go to the next step in insertion order.
    Sequential,
    /// Evaluate a branch function to decide.
    Branch(BranchFn),
    /// Go to a specific named step.
    Goto(String),
    /// This is the final step — complete the conversation.
    End,
}

/// A branching multi-step conversation.
pub struct Conversation {
    /// Conversation identifier (used in ctx state as `__conv_id`).
    pub(crate) id: String,
    /// Steps in order.
    pub(crate) steps: Vec<ConversationStep>,
    /// Step name → index mapping.
    pub(crate) step_index: HashMap<String, usize>,
    /// Called when the conversation reaches the end.
    pub(crate) on_complete: ConversationCompleteHandler,
    /// Called when the user cancels.
    pub(crate) on_cancel: Option<ConversationCancelHandler>,
}

/// Builder for constructing a [`Conversation`].
pub struct ConversationBuilder {
    id: String,
    steps: Vec<ConversationStep>,
    step_index: HashMap<String, usize>,
    on_complete: Option<ConversationCompleteHandler>,
    on_cancel: Option<ConversationCancelHandler>,
    /// Pending branch/goto overrides: step_name → StepNext
    overrides: HashMap<String, StepNext>,
}

impl Conversation {
    /// Create a new conversation builder with the given ID.
    pub fn builder(id: impl Into<String>) -> ConversationBuilder {
        ConversationBuilder {
            id: id.into(),
            steps: Vec::new(),
            step_index: HashMap::new(),
            on_complete: None,
            on_cancel: None,
            overrides: HashMap::new(),
        }
    }
}

impl ConversationBuilder {
    /// Add a step to the conversation.
    ///
    /// - `name`: unique step identifier
    /// - `screen_fn`: produces the screen shown at this step
    /// - `input_fn`: optional custom input handler. If `None`, raw text is stored as-is.
    pub fn step(
        mut self,
        name: &str,
        screen_fn: impl Fn(&ConversationData, &str) -> Screen + Send + Sync + 'static,
        input_fn: Option<StepInputFn>,
    ) -> Self {
        if self.step_index.contains_key(name) {
            panic!("duplicate conversation step name: '{}'", name);
        }
        let idx = self.steps.len();
        self.step_index.insert(name.to_string(), idx);
        self.steps.push(ConversationStep {
            name: name.to_string(),
            screen_fn: Arc::new(screen_fn),
            input_fn,
            next: StepNext::Sequential,
        });
        self
    }

    /// After step `step_name` completes, evaluate `branch_fn` to decide the next step.
    pub fn branch(mut self, step_name: &str, branch_fn: BranchFn) -> Self {
        self.overrides
            .insert(step_name.to_string(), StepNext::Branch(branch_fn));
        self
    }

    /// After step `step_name` completes, unconditionally jump to `target`.
    pub fn goto(mut self, step_name: &str, target: &str) -> Self {
        self.overrides
            .insert(step_name.to_string(), StepNext::Goto(target.to_string()));
        self
    }

    /// Mark a step as the final step (completes the conversation after it).
    pub fn end_at(mut self, step_name: &str) -> Self {
        self.overrides.insert(step_name.to_string(), StepNext::End);
        self
    }

    /// Set the completion handler.
    pub fn on_complete(mut self, handler: ConversationCompleteHandler) -> Self {
        self.on_complete = Some(handler);
        self
    }

    /// Set the cancel handler.
    pub fn on_cancel(mut self, handler: ConversationCancelHandler) -> Self {
        self.on_cancel = Some(handler);
        self
    }

    /// Build the conversation. Returns an error if no steps or no on_complete handler.
    pub fn build(mut self) -> Result<Conversation, &'static str> {
        if self.steps.is_empty() {
            return Err("conversation must have at least one step");
        }
        if self.on_complete.is_none() {
            return Err("conversation must have an on_complete handler");
        }

        // Apply overrides
        for (name, next) in self.overrides {
            if let Some(&idx) = self.step_index.get(&name) {
                self.steps[idx].next = next;
            } else {
                return Err("branch/goto/end_at references unknown step");
            }
        }

        Ok(Conversation {
            id: self.id,
            steps: self.steps,
            step_index: self.step_index,
            on_complete: self.on_complete.expect("checked above"),
            on_cancel: self.on_cancel,
        })
    }
}

impl Conversation {
    /// Resolve the next step index from the current step.
    pub(crate) fn next_step(&self, current_idx: usize, data: &ConversationData) -> Option<usize> {
        let step = &self.steps[current_idx];
        match &step.next {
            StepNext::Sequential => {
                let next = current_idx + 1;
                if next < self.steps.len() {
                    Some(next)
                } else {
                    None // end of conversation
                }
            }
            StepNext::Branch(f) => {
                let target = f(data);
                let idx = self.step_index.get(&target).copied();
                if idx.is_none() {
                    tracing::warn!(
                        conv_id = %self.id,
                        step = %step.name,
                        target = %target,
                        "branch returned unknown step name — ending conversation"
                    );
                }
                idx
            }
            StepNext::Goto(target) => self.step_index.get(target).copied(),
            StepNext::End => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Screen;

    #[test]
    fn build_conversation_basic() {
        let conv = Conversation::builder("test")
            .step(
                "name",
                |_data, _lang| Screen::text("s1", "Name?").build(),
                None,
            )
            .step(
                "age",
                |_data, _lang| Screen::text("s2", "Age?").build(),
                None,
            )
            .on_complete(Arc::new(|_ctx, _data| Box::pin(async move { Ok(()) })))
            .build()
            .unwrap();

        assert_eq!(conv.id, "test");
        assert_eq!(conv.steps.len(), 2);
        assert_eq!(conv.step_index["name"], 0);
        assert_eq!(conv.step_index["age"], 1);
    }

    #[test]
    fn build_conversation_no_steps_fails() {
        let result = Conversation::builder("empty")
            .on_complete(Arc::new(|_ctx, _data| Box::pin(async { Ok(()) })))
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn build_conversation_no_on_complete_fails() {
        let result = Conversation::builder("no_complete")
            .step("s1", |_data, _lang| Screen::text("s1", "?").build(), None)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn next_step_sequential() {
        let conv = Conversation::builder("seq")
            .step("a", |_data, _lang| Screen::text("a", "A").build(), None)
            .step("b", |_data, _lang| Screen::text("b", "B").build(), None)
            .on_complete(Arc::new(|_ctx, _data| Box::pin(async { Ok(()) })))
            .build()
            .unwrap();

        assert_eq!(conv.next_step(0, &HashMap::new()), Some(1));
        assert_eq!(conv.next_step(1, &HashMap::new()), None);
    }

    #[test]
    fn next_step_branch() {
        let conv = Conversation::builder("br")
            .step("q", |_data, _lang| Screen::text("q", "?").build(), None)
            .step(
                "yes",
                |_data, _lang| Screen::text("yes", "Yes").build(),
                None,
            )
            .step("no", |_data, _lang| Screen::text("no", "No").build(), None)
            .branch(
                "q",
                Arc::new(|data| {
                    if data.get("q").and_then(|v| v.as_str()).unwrap_or("") == "yes" {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    }
                }),
            )
            .end_at("yes")
            .end_at("no")
            .on_complete(Arc::new(|_ctx, _data| Box::pin(async { Ok(()) })))
            .build()
            .unwrap();

        let mut data = HashMap::new();
        data.insert("q".into(), serde_json::json!("yes"));
        assert_eq!(conv.next_step(0, &data), Some(1));

        data.insert("q".into(), serde_json::json!("no"));
        assert_eq!(conv.next_step(0, &data), Some(2));

        // end_at: should return None
        assert_eq!(conv.next_step(1, &data), None);
        assert_eq!(conv.next_step(2, &data), None);
    }

    #[test]
    fn next_step_goto() {
        let conv = Conversation::builder("gt")
            .step("a", |_data, _lang| Screen::text("a", "A").build(), None)
            .step("b", |_data, _lang| Screen::text("b", "B").build(), None)
            .step("c", |_data, _lang| Screen::text("c", "C").build(), None)
            .goto("a", "c")
            .on_complete(Arc::new(|_ctx, _data| Box::pin(async { Ok(()) })))
            .build()
            .unwrap();

        assert_eq!(conv.next_step(0, &HashMap::new()), Some(2)); // a → c (skip b)
    }

    #[test]
    fn conversation_with_cancel() {
        let conv = Conversation::builder("cancel")
            .step("s1", |_data, _lang| Screen::text("s1", "?").build(), None)
            .on_complete(Arc::new(|_ctx, _data| Box::pin(async { Ok(()) })))
            .on_cancel(Arc::new(|_ctx| Box::pin(async { Ok(()) })))
            .build()
            .unwrap();

        assert!(conv.on_cancel.is_some());
    }

    #[test]
    fn build_fails_on_unknown_branch_target() {
        let result = Conversation::builder("bad")
            .step("a", |_data, _lang| Screen::text("a", "A").build(), None)
            .branch("nonexistent", Arc::new(|_| "a".into()))
            .on_complete(Arc::new(|_ctx, _data| Box::pin(async { Ok(()) })))
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn build_fails_on_unknown_goto_target() {
        let result = Conversation::builder("bad")
            .step("a", |_data, _lang| Screen::text("a", "A").build(), None)
            .goto("typo", "a")
            .on_complete(Arc::new(|_ctx, _data| Box::pin(async { Ok(()) })))
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn build_fails_on_unknown_end_at_target() {
        let result = Conversation::builder("bad")
            .step("a", |_data, _lang| Screen::text("a", "A").build(), None)
            .end_at("nope")
            .on_complete(Arc::new(|_ctx, _data| Box::pin(async { Ok(()) })))
            .build();
        assert!(result.is_err());
    }

    #[test]
    #[should_panic(expected = "duplicate conversation step name: 'a'")]
    fn duplicate_step_name_panics() {
        Conversation::builder("dup")
            .step("a", |_data, _lang| Screen::text("a", "A").build(), None)
            .step("a", |_data, _lang| Screen::text("a2", "A2").build(), None);
    }
}
