use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Origin {
    pub repo: String,
    pub path: String,
    pub revision: String,
    pub translator: String,
    pub lang: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub span: Option<Span>,
}

impl Origin {
    pub fn new(
        repo: impl Into<String>,
        path: impl Into<String>,
        revision: impl Into<String>,
        translator: impl Into<String>,
        lang: impl Into<String>,
    ) -> Self {
        Origin {
            repo: repo.into(),
            path: path.into(),
            revision: revision.into(),
            translator: translator.into(),
            lang: lang.into(),
            span: None,
        }
    }

    pub fn with_span(mut self, start: u32, end: u32) -> Self {
        self.span = Some(Span { start, end });
        self
    }

    pub fn canonical(&self) -> String {
        let span = match self.span {
            Some(s) => format!("{}:{}", s.start, s.end),
            None => String::new(),
        };
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.repo, self.path, self.revision, self.translator, self.lang, span
        )
    }
}
