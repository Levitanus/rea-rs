use std::{error::Error, fmt::Display};

#[derive(Debug, PartialEq)]
pub enum ReaperError {
    UserAborted,
    Unexpected,
    InvalidObject(&'static str),
    UnsuccessfulOperation(&'static str),
    NullPtr,
    Str(&'static str),
}
impl Error for ReaperError {}
impl Display for ReaperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unexpected => write!(f, "Unexpected error happened."),
            Self::UserAborted => write!(f, "User aborted operation."),
            Self::InvalidObject(s) => write!(f, "Invalid object: {}", *s),
            Self::UnsuccessfulOperation(s) => write!(f, "Unsuccessful operation: {}", *s),
            Self::NullPtr => write!(f, "NullPtr!"),
            Self::Str(s) => write!(f, "{}", *s),
        }
    }
}

pub type ReaperResult<T> = Result<T, Box<dyn Error>>;
pub type ReaperStaticResult<T> = Result<T, ReaperError>;
