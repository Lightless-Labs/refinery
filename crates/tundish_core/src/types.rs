use serde::{Deserialize, Serialize};

/// Unique identifier for a model participating in dispatch.
///
/// Combines a provider name (e.g. `claude-code`) and a model name (e.g. `claude-opus-4-6`).
/// Display format is `provider/model`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelId {
    provider: String,
    model: String,
}

impl ModelId {
    /// Parse a `"provider/model"` string. Panics on invalid format.
    ///
    /// For fallible parsing, use [`ModelId::parse`].
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        let s = s.into();
        Self::parse(&s)
            .unwrap_or_else(|_| panic!("invalid ModelId format: '{s}' (expected 'provider/model')"))
    }

    /// Parse a `"provider/model"` string, returning `Err` on invalid format.
    pub fn parse(s: &str) -> Result<Self, String> {
        if let Some((provider, model)) = s.split_once('/') {
            if provider.is_empty() {
                return Err(format!("empty provider in '{s}'"));
            }
            if model.is_empty() {
                return Err(format!("empty model in '{s}'"));
            }
            if model.contains('/') {
                return Err(format!("model name must not contain '/': '{s}'"));
            }
            Ok(Self {
                provider: provider.to_string(),
                model: model.to_string(),
            })
        } else {
            Err(format!("expected 'provider/model' format, got '{s}'"))
        }
    }

    /// Construct from explicit provider and model parts.
    #[must_use]
    pub fn from_parts(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
        }
    }

    /// The provider name (e.g. `claude-code`).
    #[must_use]
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// The model name (e.g. `claude-opus-4-6`).
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }
}

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.provider, self.model)
    }
}

impl Serialize for ModelId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ModelId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Role in a conversation message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single message in a conversation.
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn model_id_parse_valid() {
        let id = ModelId::new("claude-code/opus-4-6");
        assert_eq!(id.provider(), "claude-code");
        assert_eq!(id.model(), "opus-4-6");
        assert_eq!(id.to_string(), "claude-code/opus-4-6");
    }

    #[test]
    fn model_id_from_parts() {
        let id = ModelId::from_parts("codex-cli", "gpt-5.4");
        assert_eq!(id.provider(), "codex-cli");
        assert_eq!(id.model(), "gpt-5.4");
        assert_eq!(id.to_string(), "codex-cli/gpt-5.4");
    }

    #[test]
    fn model_id_parse_errors() {
        assert!(ModelId::parse("no-slash").is_err());
        assert!(ModelId::parse("/no-provider").is_err());
        assert!(ModelId::parse("no-model/").is_err());
        assert!(ModelId::parse("a/b/c").is_err());
    }

    #[test]
    fn model_id_serde_roundtrip() {
        let id = ModelId::new("claude-code/opus-4-6");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"claude-code/opus-4-6\"");
        let parsed: ModelId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn model_id_as_hashmap_key() {
        let mut map = HashMap::new();
        let id = ModelId::new("test/claude");
        map.insert(id.clone(), "hello");
        assert_eq!(map.get(&id), Some(&"hello"));
    }
}
