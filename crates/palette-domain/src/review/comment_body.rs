use std::fmt;

const MAX_LEN: usize = 10_000;

/// Body text of a review comment.
#[derive(Debug, Clone)]
pub struct CommentBody(String);

impl CommentBody {
    pub fn parse(s: impl Into<String>) -> Result<Self, InvalidCommentBody> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err(InvalidCommentBody::Empty);
        }
        if s.len() > MAX_LEN {
            return Err(InvalidCommentBody::TooLong { len: s.len() });
        }
        Ok(Self(s))
    }
}

impl AsRef<str> for CommentBody {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommentBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<CommentBody> for String {
    fn from(b: CommentBody) -> Self {
        b.0
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
#[reason_namespace = "body"]
pub enum InvalidCommentBody {
    Empty,
    TooLong { len: usize },
}
