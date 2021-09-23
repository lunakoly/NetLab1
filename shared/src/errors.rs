#[derive(Debug)]
pub enum ErrorKind {
    NothingToRead,
    MessageSizeExceeded,
    Io { source: std::io::Error },
    ParsingJson { source: serde_json::Error },
    MalformedMessage { message: String },
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ErrorKind::NothingToRead => {
                write!(formatter, "Unable to parse a message, closing the connection")
            }
            ErrorKind::MessageSizeExceeded => {
                write!(formatter, "Message maximum size exceeded")
            }
            ErrorKind::Io { source } => {
                write!(formatter, "Io > {}", source)
            }
            ErrorKind::ParsingJson { source } => {
                write!(formatter, "Parsing JSON > {}", source)
            },
            ErrorKind::MalformedMessage { message } => {
                write!(formatter, "Received a message with incorrect format > {}", message)
            }
        }
    }
}

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "Shared Code Error > {}", self.kind)
    }
}

impl std::error::Error for Error {}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Error { kind: kind }
    }
}

impl From<serde_json::Error> for Error {
    fn from(source: serde_json::Error) -> Self {
        // Unwrap embedded io::Error
        match source.classify() {
            serde_json::error::Category::Io => {
                std::io::Error::from(source).into()
            }
            _ => Error {
                kind: ErrorKind::ParsingJson {
                    source: source,
                }
            }
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error {
            kind: ErrorKind::Io {
                source: source,
            }
        }
    }
}

impl From<ErrorKind> for std::io::Error {
    fn from(kind: ErrorKind) -> Self {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            Error { kind: kind }
        )
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn with_error_report<F: FnOnce() -> Result<()>>(run: F) -> Result<()> {
    let result = run();

    match &result {
        Err(error) => {
            println!("Error > {}", error);
        }
        _ => {}
    };

    result
}
