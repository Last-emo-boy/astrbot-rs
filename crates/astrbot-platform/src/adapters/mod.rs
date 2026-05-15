mod console;
mod mock;
mod onebot;
mod webchat;

pub use console::{ConsolePlatform, ConsoleSink};
pub use mock::MockPlatform;
pub use onebot::OneBotPlatform;
pub use webchat::WebChatPlatform;
