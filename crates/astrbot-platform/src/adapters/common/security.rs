use astrbot_core::{AstrbotError, Result};
use sha1::{Digest, Sha1};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebhookSignatureInput {
    pub timestamp: String,
    pub nonce: String,
    pub payload: String,
}

impl WebhookSignatureInput {
    pub fn new(
        timestamp: impl Into<String>,
        nonce: impl Into<String>,
        payload: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: timestamp.into(),
            nonce: nonce.into(),
            payload: payload.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebhookSignatureVerdict {
    Match,
    Mismatch,
}

impl WebhookSignatureVerdict {
    pub fn is_match(&self) -> bool {
        matches!(self, Self::Match)
    }
}

pub trait WebhookSignatureVerifier: Send + Sync {
    fn sign(&self, input: &WebhookSignatureInput) -> Result<String>;

    fn verify(
        &self,
        input: &WebhookSignatureInput,
        received_signature: &str,
    ) -> Result<WebhookSignatureVerdict> {
        let expected = self.sign(input)?;
        if constant_time_eq(expected.as_bytes(), received_signature.as_bytes()) {
            Ok(WebhookSignatureVerdict::Match)
        } else {
            Ok(WebhookSignatureVerdict::Mismatch)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sha1SortedFieldsVerifier {
    token: String,
}

impl Sha1SortedFieldsVerifier {
    pub fn new(token: impl Into<String>) -> Result<Self> {
        let token = token.into().trim().to_string();
        if token.is_empty() {
            return Err(AstrbotError::Platform(
                "webhook signature token cannot be empty".to_string(),
            ));
        }
        Ok(Self { token })
    }

    fn sorted_fields<'a>(&'a self, input: &'a WebhookSignatureInput) -> [&'a str; 4] {
        let mut fields = [
            self.token.as_str(),
            input.timestamp.as_str(),
            input.nonce.as_str(),
            input.payload.as_str(),
        ];
        fields.sort_unstable();
        fields
    }
}

impl WebhookSignatureVerifier for Sha1SortedFieldsVerifier {
    fn sign(&self, input: &WebhookSignatureInput) -> Result<String> {
        let fields = self.sorted_fields(input);
        let mut hasher = Sha1::new();
        for field in fields {
            hasher.update(field.as_bytes());
        }
        Ok(format!("{:x}", hasher.finalize()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncryptedWebhookEnvelope {
    pub ciphertext: String,
    pub signature: Option<String>,
    pub timestamp: Option<String>,
    pub nonce: Option<String>,
}

impl EncryptedWebhookEnvelope {
    pub fn new(ciphertext: impl Into<String>) -> Self {
        Self {
            ciphertext: ciphertext.into(),
            signature: None,
            timestamp: None,
            nonce: None,
        }
    }

    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    pub fn with_nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = Some(nonce.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedWebhookPayload {
    pub body: Vec<u8>,
    pub receive_id: Option<String>,
    pub content_type: Option<String>,
}

impl DecodedWebhookPayload {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            body: text.into().into_bytes(),
            receive_id: None,
            content_type: Some("text/plain; charset=utf-8".to_string()),
        }
    }

    pub fn body_text(&self) -> Result<String> {
        String::from_utf8(self.body.clone()).map_err(|err| {
            AstrbotError::Platform(format!("decoded webhook payload is not UTF-8: {err}"))
        })
    }
}

pub trait WebhookPayloadCodec: Send + Sync {
    fn decrypt(&self, envelope: &EncryptedWebhookEnvelope) -> Result<DecodedWebhookPayload>;

    fn encrypt(
        &self,
        payload: &DecodedWebhookPayload,
        timestamp: Option<&str>,
        nonce: Option<&str>,
    ) -> Result<EncryptedWebhookEnvelope>;
}

#[derive(Clone, Debug, Default)]
pub struct PlainWebhookPayloadCodec;

impl WebhookPayloadCodec for PlainWebhookPayloadCodec {
    fn decrypt(&self, envelope: &EncryptedWebhookEnvelope) -> Result<DecodedWebhookPayload> {
        Ok(DecodedWebhookPayload {
            body: envelope.ciphertext.as_bytes().to_vec(),
            receive_id: None,
            content_type: Some("text/plain; charset=utf-8".to_string()),
        })
    }

    fn encrypt(
        &self,
        payload: &DecodedWebhookPayload,
        timestamp: Option<&str>,
        nonce: Option<&str>,
    ) -> Result<EncryptedWebhookEnvelope> {
        let ciphertext = payload.body_text()?;
        let mut envelope = EncryptedWebhookEnvelope::new(ciphertext);
        if let Some(timestamp) = timestamp {
            envelope = envelope.with_timestamp(timestamp);
        }
        if let Some(nonce) = nonce {
            envelope = envelope.with_nonce(nonce);
        }
        Ok(envelope)
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut diff = left.len() ^ right.len();
    let max_len = left.len().max(right.len());
    for index in 0..max_len {
        let lhs = left.get(index).copied().unwrap_or_default();
        let rhs = right.get(index).copied().unwrap_or_default();
        diff |= usize::from(lhs ^ rhs);
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::{
        DecodedWebhookPayload, EncryptedWebhookEnvelope, PlainWebhookPayloadCodec,
        Sha1SortedFieldsVerifier, WebhookPayloadCodec, WebhookSignatureInput,
        WebhookSignatureVerdict, WebhookSignatureVerifier,
    };

    #[test]
    fn sha1_sorted_fields_verifier_matches_wecom_signature_shape() {
        let verifier = Sha1SortedFieldsVerifier::new("token").expect("token should be valid");
        let input = WebhookSignatureInput::new("timestamp", "nonce", "encrypted");

        let signature = verifier.sign(&input).expect("signature should be built");
        let verdict = verifier
            .verify(&input, &signature)
            .expect("verification should work");

        assert_eq!(signature, "3fd032d4cf5a3b022c47bf64daa235b035b45619");
        assert!(verdict.is_match());
        assert!(matches!(
            verifier
                .verify(&input, "bad")
                .expect("verification should work"),
            WebhookSignatureVerdict::Mismatch
        ));
    }

    #[test]
    fn plain_codec_round_trips_text_payload() {
        let codec = PlainWebhookPayloadCodec;
        let payload = DecodedWebhookPayload::text("hello");

        let envelope = codec
            .encrypt(&payload, Some("1"), Some("nonce"))
            .expect("payload should encode");
        assert_eq!(envelope.ciphertext, "hello");
        assert_eq!(envelope.timestamp.as_deref(), Some("1"));

        let decoded = codec
            .decrypt(&EncryptedWebhookEnvelope::new("hello"))
            .expect("payload should decode");
        assert_eq!(decoded.body_text().expect("text should decode"), "hello");
    }
}
