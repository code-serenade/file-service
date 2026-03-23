const ALPHABET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Encodes a u64 value into a Base62 string.
#[must_use]
pub fn encode_u64(mut value: u64) -> String {
    if value == 0 {
        return "0".to_string();
    }

    let mut buf: Vec<char> = Vec::new();
    while value > 0 {
        let idx = (value % 62) as usize;
        buf.push(ALPHABET[idx] as char);
        value /= 62;
    }
    buf.iter().rev().collect()
}

/// Encodes a u128 value into a Base62 string.
#[must_use]
pub fn encode_u128(mut value: u128) -> String {
    if value == 0 {
        return "0".to_string();
    }

    let mut buf: Vec<char> = Vec::new();
    while value > 0 {
        let idx = (value % 62) as usize;
        buf.push(ALPHABET[idx] as char);
        value /= 62;
    }
    buf.iter().rev().collect()
}

/// Decodes a Base62 string into u64.
///
/// Returns `None` when input is empty, contains invalid characters,
/// or overflows u64.
#[must_use]
pub fn decode_to_u64(input: &str) -> Option<u64> {
    if input.is_empty() {
        return None;
    }

    let mut result: u64 = 0;
    for ch in input.bytes() {
        let digit = match ch {
            b'0'..=b'9' => (ch - b'0') as u64,
            b'A'..=b'Z' => (ch - b'A' + 10) as u64,
            b'a'..=b'z' => (ch - b'a' + 36) as u64,
            _ => return None,
        };
        result = result.checked_mul(62)?;
        result = result.checked_add(digit)?;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::{decode_to_u64, encode_u128, encode_u64};

    #[test]
    fn test_encode_decode_zero() {
        let encoded = encode_u64(0);
        assert_eq!(encoded, "0");
        assert_eq!(decode_to_u64(&encoded), Some(0));
    }

    #[test]
    fn test_encode_decode_values() {
        let values = [
            1_u64,
            9,
            10,
            35,
            36,
            61,
            62,
            63,
            999,
            123_456_789,
            u32::MAX as u64,
            u64::MAX,
        ];

        for value in values {
            let encoded = encode_u64(value);
            let decoded = decode_to_u64(&encoded);
            assert_eq!(decoded, Some(value));
        }
    }

    #[test]
    fn test_decode_invalid_input() {
        assert_eq!(decode_to_u64(""), None);
        assert_eq!(decode_to_u64("!"), None);
        assert_eq!(decode_to_u64("abc-123"), None);
    }

    #[test]
    fn test_encode_u128_zero() {
        assert_eq!(encode_u128(0), "0");
    }

    #[test]
    fn test_encode_u128_non_zero() {
        let encoded = encode_u128(u128::MAX);
        assert!(!encoded.is_empty());
        assert!(encoded.chars().all(|ch| ch.is_ascii_alphanumeric()));
    }
}
