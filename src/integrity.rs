use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult, ErrorClass};

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

pub fn sha256_json<T: Serialize>(value: &T) -> AppResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|_| {
        AppError::new(
            ErrorClass::Contract,
            "json_serialization_failed",
            "could not serialize a contract document",
        )
    })?;
    Ok(sha256_bytes(&bytes))
}

pub fn unix_ms() -> AppResult<u64> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        AppError::new(
            ErrorClass::Io,
            "clock_before_epoch",
            "the system clock is before the Unix epoch",
        )
    })?;
    u64::try_from(duration.as_millis()).map_err(|_| {
        AppError::new(
            ErrorClass::Io,
            "clock_out_of_range",
            "the system clock is outside the supported range",
        )
    })
}
