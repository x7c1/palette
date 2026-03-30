use std::fmt;

const MAX_VALUE: i32 = 1_000_000;

/// Line number in a review comment.
#[derive(Debug, Clone, Copy)]
pub struct LineNumber(i32);

impl LineNumber {
    pub fn parse(n: i32) -> Result<Self, InvalidLineNumber> {
        if n < 0 {
            return Err(InvalidLineNumber::Negative { value: n });
        }
        if n > MAX_VALUE {
            return Err(InvalidLineNumber::TooLarge { value: n });
        }
        Ok(Self(n))
    }

    pub fn value(&self) -> i32 {
        self.0
    }
}

impl fmt::Display for LineNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, palette_macros::ReasonKey)]
#[reason_namespace = "line"]
pub enum InvalidLineNumber {
    Negative { value: i32 },
    TooLarge { value: i32 },
}
