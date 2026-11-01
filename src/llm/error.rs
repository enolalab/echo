#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmError {
    Auth,
    NotFound,
    RateLimit,
    Timeout(String),
    Network(String),
    Config(String),
    Other { status: Option<u16>, body: String },
}

impl LlmError {
    pub fn from_status(status: u16, body: String) -> Self {
        match status {
            401 | 403 => Self::Auth,
            404 => Self::NotFound,
            429 => Self::RateLimit,
            _ => Self::Other { status: Some(status), body: body.chars().take(200).collect() },
        }
    }

    pub fn from_reqwest(err: &reqwest::Error, url: &str) -> Self {
        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_else(|| url.to_string());
        if err.is_timeout() {
            Self::Timeout(host)
        } else if err.is_connect() || err.is_request() {
            Self::Network(host)
        } else {
            Self::Other { status: err.status().map(|s| s.as_u16()), body: err.to_string() }
        }
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Auth | Self::NotFound | Self::Config(_) => false,
            Self::Other { status: Some(s), .. } => *s >= 500,
            _ => true,
        }
    }

    pub fn should_backoff(&self) -> bool {
        matches!(self, Self::RateLimit) || matches!(self, Self::Other { status: Some(s), .. } if *s >= 500)
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::Auth => "API key không hợp lệ".into(),
            Self::NotFound => "Sai base URL hoặc tên model".into(),
            Self::RateLimit => "Bị giới hạn tốc độ, thử lại sau".into(),
            Self::Timeout(h) | Self::Network(h) => format!("Không kết nối được tới {h}"),
            Self::Config(m) => m.clone(),
            Self::Other { status: Some(s), body } => format!("Lỗi provider ({s}): {body}"),
            Self::Other { status: None, body } => format!("Lỗi provider: {body}"),
        }
    }
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.user_message())
    }
}

impl std::error::Error for LlmError {}
