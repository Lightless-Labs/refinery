pub mod error;
pub mod progress;
pub mod types;
pub mod util;

pub use error::ProviderError;
pub use progress::ProgressFn;
pub use types::{Message, ModelId, Role};
pub use util::{check_json_depth, extract_json};

use async_trait::async_trait;

/// A model provider that can send messages and receive responses.
#[async_trait]
pub trait ModelProvider: Send + Sync + std::fmt::Debug {
    /// Send a sequence of messages and return the model's text response.
    async fn send_message(&self, messages: &[Message]) -> Result<String, ProviderError>;

    /// The unique identifier for this model.
    fn model_id(&self) -> &ModelId;
}
