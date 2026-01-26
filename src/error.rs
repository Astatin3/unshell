use std::fmt;

pub type Result<T> = std::result::Result<T, ModuleError>;

///Generic error type for module-related operations.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ModuleError {
    LibLoadingError(String),
    // LogError(log::SetLoggerError),
    LinkError(String),
    CryptError(String),
    DatabaseError(String),
    SerdeJsonError(String),

    TreeMessageError(String),

    Error(String),
}

impl From<&str> for ModuleError {
    fn from(value: &str) -> Self {
        Self::Error(value.into())
    }
}

impl From<serde_json::Error> for ModuleError {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeJsonError(value.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for ModuleError {
    fn from(value: Box<dyn std::error::Error>) -> Self {
        ModuleError::Error(value.to_string())
    }
}

impl std::error::Error for ModuleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        Some(self)
    }
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(format!("{:?}", self).as_str())
    }
}

impl From<ModuleError> for std::string::String {
    fn from(value: ModuleError) -> Self {
        value.to_string()
    }
}
