use std::fmt::Display;

// CSharp-like output streams
#[derive(Debug, Clone, PartialEq)]
pub enum CSharpStream {
    Success, // Stream 1 - regular output
    Error,   // Stream 2 - errors
    Warning, // Stream 3 - warnings
    Verbose, // Stream 4 - verbose messages
}

impl Display for CSharpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let msg = match self {
            CSharpStream::Success => "",
            CSharpStream::Error => "ERROR",
            CSharpStream::Warning => "WARNING",
            CSharpStream::Verbose => "VERBOSE",
        };
        write!(f, "{}", msg)
    }
}

#[derive(Debug, Clone)]
pub struct StreamMessage {
    pub content: String,
    pub stream: CSharpStream,
    pub timestamp: std::time::SystemTime,
}

impl From<String> for StreamMessage {
    fn from(content: String) -> Self {
        StreamMessage::success(content)
    }
}

impl From<StreamMessage> for String {
    fn from(content: StreamMessage) -> Self {
        content.content
    }
}

impl Display for StreamMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.stream == CSharpStream::Success {
            write!(f, "{}", self.content)
        } else {
            write!(
                f,
                "[{}] {}: {}",
                self.stream,
                self.timestamp
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                self.content
            )
        }
    }
}

impl StreamMessage {
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn success(content: String) -> Self {
        StreamMessage {
            content,
            stream: CSharpStream::Success,
            timestamp: std::time::SystemTime::now(),
        }
    }

    pub fn warning(message: String) -> Self {
        StreamMessage {
            content: format!("WARNING: {}", message),
            stream: CSharpStream::Warning,
            timestamp: std::time::SystemTime::now(),
        }
    }

    pub fn error(message: String) -> Self {
        StreamMessage {
            content: format!("ERROR: {}", message),
            stream: CSharpStream::Error,
            timestamp: std::time::SystemTime::now(),
        }
    }

    pub fn verbose(message: String) -> Self {
        StreamMessage {
            content: format!("VERBOSE: {}", message),
            stream: CSharpStream::Verbose,
            timestamp: std::time::SystemTime::now(),
        }
    }
}
