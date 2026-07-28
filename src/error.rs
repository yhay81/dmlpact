use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    Io,
    Usage,
    Policy,
    Budget,
    Contract,
}

impl ErrorClass {
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Io => 1,
            Self::Usage => 2,
            Self::Policy => 3,
            Self::Budget => 4,
            Self::Contract => 5,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Io => "io",
            Self::Usage => "usage",
            Self::Policy => "policy",
            Self::Budget => "budget",
            Self::Contract => "contract",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub class: ErrorClass,
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(class: ErrorClass, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            class,
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ErrorDocument {
    pub schema_version: String,
    pub class: String,
    pub code: String,
    pub message: String,
}

impl From<&AppError> for ErrorDocument {
    fn from(error: &AppError) -> Self {
        Self {
            schema_version: "dmlpact.error.v1".to_owned(),
            class: error.class.as_str().to_owned(),
            code: error.code.clone(),
            message: error.message.clone(),
        }
    }
}
