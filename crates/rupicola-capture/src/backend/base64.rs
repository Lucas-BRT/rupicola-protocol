//! Standard base64 (RFC 4648), decoding only: the browser sends screenshots in it.

/// The base64 alphabet.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
/// The character that pads the last group.
const PADDING: u8 = b'=';
/// Bits one character carries, and bits in a byte.
const SEXTET_BITS: u32 = 6;
const BYTE_BITS: u32 = 8;

/// Decodes `text`; `None` when it holds a character outside the alphabet or is cut short.
/// Whitespace is ignored, so a wrapped body decodes too.
pub(crate) fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    let mut padded = false;
    for byte in text.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == PADDING {
            padded = true;
            continue;
        }
        if padded {
            // Nothing may follow the padding.
            return None;
        }
        let value = ALPHABET.iter().position(|letter| *letter == byte)?;
        buffer = (buffer << SEXTET_BITS) | u32::try_from(value).ok()?;
        bits += SEXTET_BITS;
        if bits >= BYTE_BITS {
            bits -= BYTE_BITS;
            out.push(u8::try_from((buffer >> bits) & 0xff).ok()?);
        }
    }
    // A last group of one character cannot hold a whole byte.
    (bits < SEXTET_BITS).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rfc_vectors_decode() {
        for (encoded, plain) in [
            ("", ""),
            ("Zg==", "f"),
            ("Zm8=", "fo"),
            ("Zm9v", "foo"),
            ("Zm9vYg==", "foob"),
            ("Zm9vYmE=", "fooba"),
            ("Zm9vYmFy", "foobar"),
        ] {
            assert_eq!(decode(encoded).unwrap(), plain.as_bytes(), "{encoded}");
        }
    }

    #[test]
    fn padding_is_optional_and_whitespace_is_ignored() {
        assert_eq!(decode("Zm9vYg").unwrap(), b"foob");
        assert_eq!(decode("Zm9v\nYmFy\n").unwrap(), b"foobar");
    }

    #[test]
    fn bytes_that_are_not_text_survive() {
        assert_eq!(
            decode("iVBORw0KGgo=").unwrap(),
            [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']
        );
    }

    #[test]
    fn what_is_not_base64_is_refused() {
        assert_eq!(decode("Zm9v!mFy"), None);
        assert_eq!(decode("Zm9vY"), None, "a lone last character holds no byte");
        assert_eq!(decode("Zg==Zg"), None, "nothing follows the padding");
    }
}
