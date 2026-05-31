//! Internal byte-reading helpers.
//!
//! Wrap the common `u16/u32/f32/u64/i32::from_le_bytes(bytes[a..b].try_into().unwrap())`
//! pattern in small `Result`-returning functions so a malformed/truncated input
//! returns a typed error instead of panicking.

use crate::{Error, Result};

#[inline]
fn slice_at<const N: usize>(bytes: &[u8], offset: usize, what: &str) -> Result<[u8; N]> {
    bytes
        .get(offset..offset + N)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| {
            Error::Parse(format!(
                "truncated {what} at offset 0x{offset:X} (len {})",
                bytes.len()
            ))
        })
}

#[inline]
pub(crate) fn read_u16_le(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(slice_at::<2>(bytes, offset, "u16")?))
}

#[inline]
pub(crate) fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(slice_at::<4>(bytes, offset, "u32")?))
}

#[inline]
pub(crate) fn read_f32_le(bytes: &[u8], offset: usize) -> Result<f32> {
    Ok(f32::from_le_bytes(slice_at::<4>(bytes, offset, "f32")?))
}
