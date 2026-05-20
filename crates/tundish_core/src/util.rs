/// Extract JSON from a response that may contain markdown fences.
#[must_use]
pub fn extract_json(text: &str) -> Option<&str> {
    // Try to find ```json ... ``` fences first
    if let Some(start) = text.find("```json") {
        let json_start = start + 7; // skip "```json"
        if let Some(end) = text[json_start..].find("```") {
            return Some(text[json_start..json_start + end].trim());
        }
    }
    // Try bare ``` ... ```
    if let Some(start) = text.find("```") {
        let content_start = start + 3;
        if let Some(end) = text[content_start..].find("```") {
            let content = text[content_start..content_start + end].trim();
            if content.starts_with('{') {
                return Some(content);
            }
        }
    }
    // Try raw JSON (starts with {)
    let trimmed = text.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    None
}

/// Check JSON nesting depth. Returns `Err(actual_depth)` if exceeding `max_depth`.
pub fn check_json_depth(text: &str, max_depth: usize) -> Result<(), usize> {
    let mut depth: usize = 0;
    let mut max_seen: usize = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for c in text.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if c == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match c {
            '{' | '[' => {
                depth += 1;
                max_seen = max_seen.max(depth);
                if max_seen > max_depth {
                    return Err(max_seen);
                }
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_from_fenced() {
        let text = "Some text\n```json\n{\"score\": 8}\n```\nMore text";
        assert_eq!(extract_json(text), Some("{\"score\": 8}"));
    }

    #[test]
    fn extract_json_from_bare_fenced() {
        let text = "```\n{\"score\": 8}\n```";
        assert_eq!(extract_json(text), Some("{\"score\": 8}"));
    }

    #[test]
    fn extract_json_raw() {
        let text = "{\"score\": 8}";
        assert_eq!(extract_json(text), Some("{\"score\": 8}"));
    }

    #[test]
    fn extract_json_no_json() {
        let text = "No JSON here";
        assert_eq!(extract_json(text), None);
    }

    #[test]
    fn check_json_depth_ok() {
        let json = r#"{"a": {"b": {"c": 1}}}"#;
        assert!(check_json_depth(json, 10).is_ok());
    }

    #[test]
    fn check_json_depth_exceeded() {
        let json =
            r#"{"a": {"b": {"c": {"d": {"e": {"f": {"g": {"h": {"i": {"j": {"k": 1}}}}}}}}}}}"#;
        assert!(check_json_depth(json, 10).is_err());
    }

    #[test]
    fn check_json_depth_strings_not_counted() {
        let json = r#"{"a": "{{{{{{{{{{{{{{{{}"}"#;
        assert!(check_json_depth(json, 2).is_ok());
    }
}
