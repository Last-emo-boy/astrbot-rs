use astrbot_core::{AstrbotError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UnsupportedStreamingPolicy {
    provider_name: &'static str,
}

impl UnsupportedStreamingPolicy {
    fn reject(provider_name: &'static str) -> Self {
        Self { provider_name }
    }

    fn ensure_supported(self, requested: bool) -> Result<()> {
        if requested {
            return Err(AstrbotError::Provider(format!(
                "{} streaming is not implemented yet",
                self.provider_name
            )));
        }

        Ok(())
    }
}

pub(crate) fn reject_unsupported_streaming(
    provider_name: &'static str,
    requested: bool,
) -> Result<()> {
    UnsupportedStreamingPolicy::reject(provider_name).ensure_supported(requested)
}

#[cfg(test)]
mod tests {
    use super::reject_unsupported_streaming;

    #[test]
    fn rejects_requested_unsupported_streaming() {
        let error = reject_unsupported_streaming("Gemini", true)
            .expect_err("unsupported streaming should be rejected");

        assert_eq!(
            error.to_string(),
            "provider error: Gemini streaming is not implemented yet"
        );
    }

    #[test]
    fn ignores_non_streaming_requests() {
        reject_unsupported_streaming("Gemini", false)
            .expect("non-streaming request should pass through");
    }
}
