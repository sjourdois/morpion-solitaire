//! Encoding and decoding of MSR records.
//!
//! Two equivalent forms carry the same JSON:
//! - **Compact** (`MS1:` + URL-safe Base64 of DEFLATE-compressed JSON) — short,
//!   for storage and transport.
//! - **JSON** — the human-readable, diff-friendly interchange form.
//!
//! [`decode`] reads either, so a tool need not know which it was given.

use crate::model::Record;
use crate::Error;
use base64::Engine as _;

/// Tag identifying the compact MSR form (version 1 of the envelope).
pub const PREFIX: &str = "MS1:";

/// Encode a record to the compact `MS1:` form.
pub fn encode(record: &Record) -> Result<String, Error> {
    let json = serde_json::to_vec(record)?;
    let compressed = miniz_oxide::deflate::compress_to_vec(&json, 9);
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(compressed);
    Ok(format!("{PREFIX}{b64}"))
}

/// Encode a record to pretty-printed JSON (the readable interchange form).
pub fn encode_json(record: &Record) -> Result<String, Error> {
    Ok(serde_json::to_string_pretty(record)?)
}

/// Decode a record from either form: the `MS1:` compact envelope, or raw JSON
/// (pretty or compact). Surrounding whitespace is ignored.
pub fn decode(text: &str) -> Result<Record, Error> {
    let trimmed = text.trim();
    if let Some(b64) = trimmed.strip_prefix(PREFIX) {
        let compressed = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(b64.trim())
            .map_err(|e| Error::Base64(e.to_string()))?;
        let json = miniz_oxide::inflate::decompress_to_vec(&compressed)
            .map_err(|e| Error::Inflate(format!("{e:?}")))?;
        return Ok(serde_json::from_slice(&json)?);
    }
    Ok(serde_json::from_str(trimmed)?)
}
