use poem_openapi::Object;

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct ErrorResponse {
    message: String,
}

impl From<&str> for ErrorResponse {
    fn from(s: &str) -> Self {
        Self {
            message: s.to_string(),
        }
    }
}
