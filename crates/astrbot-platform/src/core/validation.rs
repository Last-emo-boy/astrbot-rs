use astrbot_core::{AstrbotError, Result};

pub(crate) fn validate_platform_id(id: &str) -> Result<()> {
    if id.trim().is_empty() {
        return Err(AstrbotError::Platform(
            "platform id must not be empty".to_string(),
        ));
    }
    if id.contains(':') || id.contains('!') {
        return Err(AstrbotError::Platform(format!(
            "platform id {id} must not contain ':' or '!'"
        )));
    }
    Ok(())
}
