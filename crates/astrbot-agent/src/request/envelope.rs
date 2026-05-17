use astrbot_core::{MessageEvent, ProviderRequest};

pub struct ProviderRequestEnvelope {
    pub request: ProviderRequest,
    pub explicit: bool,
}

impl ProviderRequestEnvelope {
    pub fn from_event(event: &MessageEvent) -> Option<Self> {
        if let Some(request) = event.provider_request() {
            return Some(Self {
                request: request.clone().with_event_defaults(event),
                explicit: true,
            });
        }

        let request = ProviderRequest::from_event(event);
        request.has_user_content().then_some(Self {
            request,
            explicit: false,
        })
    }
}
