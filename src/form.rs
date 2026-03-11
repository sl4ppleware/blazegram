//! Form Wizard — declarative multi-step forms.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::ctx::Ctx;
use crate::error::HandlerResult;
use crate::i18n::{ft, ft_with};
use crate::keyboard::KeyboardBuilder;
use crate::screen::Screen;
use crate::types::*;

pub type FormData = HashMap<String, serde_json::Value>;

pub type FormCompleteHandler = Arc<
    dyn Fn(&mut Ctx, FormData) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync,
>;

pub type FormCancelHandler = Arc<
    dyn Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync,
>;

pub struct Form {
    pub id: String,
    pub steps: Vec<FormStep>,
    pub on_complete: FormCompleteHandler,
    pub on_cancel: Option<FormCancelHandler>,
}

/// Function that builds a screen for a form step.
pub type FormScreenFn = Arc<dyn Fn(&FormData, &str) -> Screen + Send + Sync>;

pub struct FormStep {
    pub id: String,
    pub field: String,
    /// `lang` is the user's language code, passed automatically.
    pub screen_fn: FormScreenFn,
    pub parser: FieldParser,
}

#[derive(Clone)]
pub enum FieldParser {
    Text {
        validator: Option<ValidatorFn>,
    },
    Integer {
        min: Option<i64>,
        max: Option<i64>,
    },
    Choice {
        options: Vec<(String, String)>,
    },
    Photo,
}

impl FieldParser {
    /// Validate input. `lang` is the user's language for error messages.
    pub fn validate(&self, input: &str, lang: &str) -> Result<serde_json::Value, String> {
        match self {
            Self::Text { validator } => {
                if let Some(v) = validator {
                    v(input)?;
                }
                Ok(serde_json::Value::String(input.to_string()))
            }
            Self::Integer { min, max } => {
                let n: i64 = input
                    .parse()
                    .map_err(|_| ft(lang, "bg-err-nan"))?;
                if let Some(min) = min {
                    if n < *min {
                        return Err(ft_with(lang, "bg-err-min", &[("min", &min.to_string())]));
                    }
                }
                if let Some(max) = max {
                    if n > *max {
                        return Err(ft_with(lang, "bg-err-max", &[("max", &max.to_string())]));
                    }
                }
                Ok(serde_json::Value::Number(n.into()))
            }
            Self::Choice { options } => {
                if options.iter().any(|(_, v)| v == input) {
                    Ok(serde_json::Value::String(input.to_string()))
                } else {
                    Err(ft(lang, "bg-err-choice"))
                }
            }
            Self::Photo => Err(ft(lang, "bg-err-photo")),
        }
    }
}

// ─── Builder ───

pub struct FormBuilder {
    id: String,
    steps: Vec<FormStep>,
    on_complete: Option<FormCompleteHandler>,
    on_cancel: Option<FormCancelHandler>,
}

impl Form {
    pub fn builder(id: &str) -> FormBuilder {
        FormBuilder {
            id: id.to_string(),
            steps: Vec::new(),
            on_complete: None,
            on_cancel: None,
        }
    }
}

impl FormBuilder {
    pub fn text_step(
        self,
        id: &str,
        field: &str,
        question: impl Into<String>,
    ) -> FormStepTextBuilder {
        FormStepTextBuilder {
            parent: self,
            id: id.to_string(),
            field: field.to_string(),
            question: question.into(),
            validator: None,
            placeholder: None,
        }
    }

    pub fn integer_step(
        self,
        id: &str,
        field: &str,
        question: impl Into<String>,
    ) -> FormStepIntBuilder {
        FormStepIntBuilder {
            parent: self,
            id: id.to_string(),
            field: field.to_string(),
            question: question.into(),
            min: None,
            max: None,
        }
    }

    pub fn choice_step(
        mut self,
        id: &str,
        field: &str,
        question: impl Into<String>,
        options: Vec<(impl Into<String>, impl Into<String>)>,
    ) -> Self {
        let options: Vec<(String, String)> = options
            .into_iter()
            .map(|(d, v)| (d.into(), v.into()))
            .collect();
        let q = question.into();
        let step_id = id.to_string();
        let opts_clone = options.clone();

        self.steps.push(FormStep {
            id: step_id.clone(),
            field: field.to_string(),
            screen_fn: Arc::new(move |_data, lang| {
                let mut kb = KeyboardBuilder::with_lang(lang);
                for (display, value) in &opts_clone {
                    kb = kb.button_row(
                        display.clone(),
                        format!("__form_choice:{}", value),
                    );
                }
                kb = kb.button_row(ft(lang, "bg-form-cancel"), "__form_cancel");
                Screen::builder(format!("__form__{}", step_id))
                    .text(q.clone())
                    .keyboard(move |_| kb)
                    .build()
            }),
            parser: FieldParser::Choice { options },
        });
        self
    }

    pub fn photo_step(
        mut self,
        id: &str,
        field: &str,
        question: impl Into<String>,
    ) -> Self {
        let q = question.into();
        let step_id = id.to_string();

        self.steps.push(FormStep {
            id: step_id.clone(),
            field: field.to_string(),
            screen_fn: Arc::new(move |_data, lang| {
                Screen::builder(format!("__form__{}", step_id))
                    .text(q.clone())
                    .keyboard(|kb| kb.button_row(ft(lang, "bg-form-cancel"), "__form_cancel"))
                    .expect_photo()
                    .build()
            }),
            parser: FieldParser::Photo,
        });
        self
    }

    pub fn confirm_step(
        mut self,
        formatter: impl Fn(&FormData) -> String + Send + Sync + 'static,
    ) -> Self {
        self.steps.push(FormStep {
            id: "__confirm__".to_string(),
            field: "__confirmed__".to_string(),
            screen_fn: Arc::new(move |data, lang| {
                let summary = formatter(data);
                let text = ft_with(lang, "bg-form-review", &[("summary", &summary)]);
                Screen::builder("__form__confirm")
                    .text(text)
                    .keyboard(|kb| {
                        kb.confirm_cancel(
                            ft(lang, "bg-form-confirm"),
                            "__form_confirm:yes",
                            ft(lang, "bg-form-cancel"),
                            "__form_cancel",
                        )
                    })
                    .build()
            }),
            parser: FieldParser::Choice {
                options: vec![("yes".to_string(), "yes".to_string())],
            },
        });
        self
    }

    pub fn on_complete(
        mut self,
        handler: impl Fn(&mut Ctx, FormData) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.on_complete = Some(Arc::new(handler));
        self
    }

    pub fn on_cancel(
        mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.on_cancel = Some(Arc::new(handler));
        self
    }

    pub fn build(self) -> Form {
        Form {
            id: self.id,
            steps: self.steps,
            on_complete: self.on_complete.expect("on_complete is required"),
            on_cancel: self.on_cancel,
        }
    }
}

// ─── Text step builder ───

pub struct FormStepTextBuilder {
    parent: FormBuilder,
    id: String,
    field: String,
    question: String,
    validator: Option<ValidatorFn>,
    placeholder: Option<String>,
}

impl FormStepTextBuilder {
    pub fn validator(
        mut self,
        f: impl Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.validator = Some(Arc::new(f));
        self
    }

    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = Some(p.into());
        self
    }

    pub fn done(self) -> FormBuilder {
        let q = self.question;
        let step_id = self.id.clone();
        let validator = self.validator.clone();

        let step = FormStep {
            id: self.id,
            field: self.field,
            screen_fn: Arc::new(move |_data, lang| {
                let mut builder = Screen::builder(format!("__form__{}", step_id))
                    .text(q.clone());
                builder = builder.keyboard(|kb| kb.button_row(ft(lang, "bg-form-cancel"), "__form_cancel"));
                builder.build()
            }),
            parser: FieldParser::Text { validator },
        };

        let mut parent = self.parent;
        parent.steps.push(step);
        parent
    }

    // Chain shortcuts
    pub fn text_step(self, id: &str, field: &str, question: impl Into<String>) -> FormStepTextBuilder {
        self.done().text_step(id, field, question)
    }

    pub fn integer_step(self, id: &str, field: &str, question: impl Into<String>) -> FormStepIntBuilder {
        self.done().integer_step(id, field, question)
    }

    pub fn on_complete(
        self,
        handler: impl Fn(&mut Ctx, FormData) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> FormBuilder {
        self.done().on_complete(handler)
    }

    pub fn build(self) -> Form {
        self.done().build()
    }
}

// ─── Integer step builder ───

pub struct FormStepIntBuilder {
    parent: FormBuilder,
    id: String,
    field: String,
    question: String,
    min: Option<i64>,
    max: Option<i64>,
}

impl FormStepIntBuilder {
    pub fn min(mut self, min: i64) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: i64) -> Self {
        self.max = Some(max);
        self
    }

    pub fn done(self) -> FormBuilder {
        let q = self.question;
        let step_id = self.id.clone();

        let step = FormStep {
            id: self.id,
            field: self.field,
            screen_fn: Arc::new(move |_data, lang| {
                Screen::builder(format!("__form__{}", step_id))
                    .text(q.clone())
                    .keyboard(|kb| kb.button_row(ft(lang, "bg-form-cancel"), "__form_cancel"))
                    .build()
            }),
            parser: FieldParser::Integer {
                min: self.min,
                max: self.max,
            },
        };

        let mut parent = self.parent;
        parent.steps.push(step);
        parent
    }

    pub fn on_complete(
        self,
        handler: impl Fn(&mut Ctx, FormData) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> FormBuilder {
        self.done().on_complete(handler)
    }

    pub fn build(self) -> Form {
        self.done().build()
    }
}
