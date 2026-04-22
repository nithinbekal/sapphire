use crate::vm::{VmError, VmValue};
use std::collections::HashMap;

pub fn build_regex(pattern: &str, ignore_case: bool, line: u32) -> Result<regex::Regex, VmError> {
    let mut builder = regex::RegexBuilder::new(pattern);
    builder.case_insensitive(ignore_case);
    builder.build().map_err(|e| {
        VmError::TypeError {
            message: format!("Invalid regex pattern: {}", e),
            line,
        }
    })
}

pub fn extract_id(fields: &HashMap<String, VmValue>, line: u32) -> Result<i64, VmError> {
    match fields.get("id") {
        Some(VmValue::Int(n)) => Ok(*n),
        _ => Err(VmError::TypeError {
            message: "regex instance has invalid id".into(),
            line,
        }),
    }
}

pub fn regex_match_bool(re: &regex::Regex, text: &str) -> bool {
    re.is_match(text)
}

pub fn regex_scan(re: &regex::Regex, text: &str) -> Vec<String> {
    re.find_iter(text).map(|m| m.as_str().to_string()).collect()
}

pub fn regex_replace(re: &regex::Regex, text: &str, replacement: &str) -> String {
    re.replace(text, replacement).to_string()
}

pub fn regex_replace_all(re: &regex::Regex, text: &str, replacement: &str) -> String {
    re.replace_all(text, replacement).to_string()
}
