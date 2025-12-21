///Generic error type for module-related operations.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
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
