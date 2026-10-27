pub const EXIT_OK: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_ROUTING: i32 = 2;
pub const EXIT_PROTOCOL: i32 = 3;
pub const EXIT_BLOCKED: i32 = 4;
/// A run stopped because the engine hit its quota or rate limit (D-15). Nothing retries or switches engine.
pub const EXIT_QUOTA: i32 = 5;
pub const EXIT_INTERRUPTED: i32 = 130;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainError {
    pub message: String,
    pub code: i32,
}

impl BrainError {
    pub fn new(message: impl Into<String>, code: i32) -> Self {
        Self {
            message: message.into(),
            code,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, EXIT_ERROR)
    }

    pub fn not_available(command: &str) -> Self {
        Self::new(
            format!("Lệnh '{command}' chưa có trong bản này"),
            EXIT_ERROR,
        )
    }
}

impl std::fmt::Display for BrainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BrainError {}

impl From<std::io::Error> for BrainError {
    fn from(e: std::io::Error) -> Self {
        Self::new(format!("Lỗi I/O: {e}"), EXIT_ERROR)
    }
}

pub type BrainResult<T> = Result<T, BrainError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_exit_codes() {
        assert_eq!(EXIT_OK, 0);
        assert_eq!(EXIT_ERROR, 1);
        assert_eq!(EXIT_ROUTING, 2);
        assert_eq!(EXIT_PROTOCOL, 3);
        assert_eq!(EXIT_BLOCKED, 4);
        assert_eq!(EXIT_QUOTA, 5);
        assert_eq!(EXIT_INTERRUPTED, 130);
    }

    #[test]
    fn test_brain_error_constructors() {
        let err = BrainError::new("custom", 42);
        assert_eq!(err.message, "custom");
        assert_eq!(err.code, 42);

        let err_default = BrainError::error("x");
        assert_eq!(err_default.message, "x");
        assert_eq!(err_default.code, 1);
        assert_eq!(err_default.code, EXIT_ERROR);

        let err_na = BrainError::not_available("stats");
        assert_eq!(err_na.message, "Lệnh 'stats' chưa có trong bản này");
        assert_eq!(err_na.code, 1);
        assert_eq!(err_na.code, EXIT_ERROR);
    }

    #[test]
    fn test_display_and_error_trait() {
        let err = BrainError::new("m", 3);
        assert_eq!(format!("{err}"), "m");
        assert_eq!(format!("{}", BrainError::new("m", 3)), "m");

        let trait_obj: &dyn std::error::Error = &err;
        assert_eq!(trait_obj.to_string(), "m");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "disk read failure");
        let brain_err = BrainError::from(io_err);
        assert_eq!(brain_err.code, 1);
        assert_eq!(brain_err.code, EXIT_ERROR);
        assert!(brain_err.message.starts_with("Lỗi I/O: "));
        assert!(brain_err.message.contains("disk read failure"));
    }

    #[test]
    fn test_brain_result() {
        let ok_val: BrainResult<&str> = Ok("success");
        assert_eq!(ok_val.unwrap(), "success");

        let err_val: BrainResult<i32> = Err(BrainError::error("failed"));
        assert!(err_val.is_err());
    }
}
