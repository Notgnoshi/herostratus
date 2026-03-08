use crate::bstr::BStr;

/// Do a two-finger comparison of `a` and `b` skipping over all unicode whitespace
#[tracing::instrument(target = "perf", level = "debug", skip_all)]
pub fn is_equal_ignoring_whitespace<A: AsRef<BStr>, B: AsRef<BStr>>(a: A, b: B) -> bool {
    let mut a_chunks = a.as_ref().utf8_chunks();
    let mut a_chars = WhitespaceSkipper::new(&mut a_chunks);
    let mut b_chunks = b.as_ref().utf8_chunks();
    let mut b_chars = WhitespaceSkipper::new(&mut b_chunks);
    loop {
        let a = a_chars.next();
        let b = b_chars.next();

        match (a, b) {
            // Both iterators exhausted
            (None, None) => return true,
            // Both have a non-whitespace character and they are equal
            (Some(ac), Some(bc)) if ac == bc => continue,
            // Otherwise they're not equal, or one iterator is exhausted before the other
            _ => return false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharOrByte {
    Char(char),
    Byte(u8),
}

/// A utility to skip over ascii and unicode whitespace in a [`BStr`]
struct WhitespaceSkipper<'a, I> {
    chunks: &'a mut I,
    current_valid: std::str::Chars<'a>,
    current_invalid: std::slice::Iter<'a, u8>,
}

// This might not be very cache or branch predictor friendly, but it's simple enough until
// performance becomes a concern.
impl<'a, I> WhitespaceSkipper<'a, I>
where
    I: Iterator<Item = std::str::Utf8Chunk<'a>>,
{
    fn new(chunks: &'a mut I) -> Self {
        let mut this = Self {
            chunks,
            current_valid: "".chars(),
            current_invalid: [].iter(),
        };
        this.advance_chunk();
        this
    }

    fn advance_chunk(&mut self) {
        if let Some(chunk) = self.chunks.next() {
            self.current_valid = chunk.valid().chars();
            self.current_invalid = chunk.invalid().iter();
        }
    }

    fn next_any_char(&mut self) -> Option<CharOrByte> {
        if let Some(c) = self.current_valid.next() {
            return Some(CharOrByte::Char(c));
        }

        if let Some(b) = self.current_invalid.next() {
            return Some(CharOrByte::Byte(*b));
        }

        self.advance_chunk();

        if let Some(c) = self.current_valid.next() {
            return Some(CharOrByte::Char(c));
        }

        if let Some(b) = self.current_invalid.next() {
            return Some(CharOrByte::Byte(*b));
        }

        None
    }

    fn next_non_whitespace(&mut self) -> Option<CharOrByte> {
        while let Some(c) = self.next_any_char() {
            match c {
                CharOrByte::Char(c) if c.is_whitespace() => continue,
                CharOrByte::Byte(b) if b.is_ascii_whitespace() => continue,
                _ => return Some(c),
            }
        }
        None
    }
}

impl<'a, I> Iterator for WhitespaceSkipper<'a, I>
where
    I: Iterator<Item = std::str::Utf8Chunk<'a>>,
{
    type Item = CharOrByte;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_non_whitespace()
    }
}

/// Compare two byte strings to check if they're equal ignoring whitespace.
///
/// The hot path handles ASCII bytes inline (single comparison + lookup), and only falls back to
/// UTF-8 character decoding when a multi-byte sequence is encountered.
///
/// Benchmarked with `cargo bench --bench bench-whitespace`
#[tracing::instrument(target = "perf", level = "debug", skip_all)]
pub fn is_equal_ignoring_whitespace_v2<A: AsRef<BStr>, B: AsRef<BStr>>(a: A, b: B) -> bool {
    let a = a.as_ref().as_ref();
    let b = b.as_ref().as_ref();
    let mut ai = 0;
    let mut bi = 0;

    // Do a two-finger comparison on the byte slices, skipping any ASCII or UTF-8 whitespace before
    // comparison
    loop {
        skip_whitespace(a, &mut ai);
        skip_whitespace(b, &mut bi);

        let a_done = ai >= a.len();
        let b_done = bi >= b.len();

        if a_done && b_done {
            return true;
        }
        if a_done || b_done {
            return false;
        }

        let ab = a[ai];
        let bb = b[bi];

        // hot path: if both bytes are ascii, we can just compare them
        if ab.is_ascii() && bb.is_ascii() {
            if ab != bb {
                return false;
            }
            ai += 1;
            bi += 1;
        }
        // early-out: if only one of the bytes is ascii, they're not equal and we can return
        // immediately.
        else if ab.is_ascii() || bb.is_ascii() {
            return false;
        }
        // cool path: both bytes are non-ascii, so we decode the next UTF-8 character from both
        // and compare. This decodes a single UTF-8 character at a time, which probably doesn't
        // take proper advantage of the cache + branch predictor, but in Git diffs we're likely not
        // to get large runs of unicode strings; it's likely to either be ASCII source code, or
        // non-string binary data.
        else {
            let (ac, a_len) = decode_utf8_char(&a[ai..]);
            let (bc, b_len) = decode_utf8_char(&b[bi..]);
            match (ac, bc) {
                (Some(ac), Some(bc)) if ac == bc => {}
                // Both invalid -- compare raw bytes
                (None, None) if ab == bb => {}
                _ => return false,
            }
            ai += a_len;
            bi += b_len;
        }
    }
}

/// Advance `*pos` past any whitespace (ASCII or Unicode) in `data`.
#[inline]
fn skip_whitespace(data: &[u8], pos: &mut usize) {
    while *pos < data.len() {
        let b = data[*pos];
        if b.is_ascii() {
            if b.is_ascii_whitespace() {
                *pos += 1;
            } else {
                return;
            }
        } else {
            let (ch, len) = decode_utf8_char(&data[*pos..]);
            match ch {
                Some(c) if c.is_whitespace() => *pos += len,
                _ => return,
            }
        }
    }
}

/// Decode one UTF-8 character from the start of `bytes`. Returns the decoded character (or None
/// for invalid bytes) and how many bytes were consumed.
///
/// Only examines the bytes needed for a single character (at most 4), determined by the leading
/// byte.
#[inline]
fn decode_utf8_char(bytes: &[u8]) -> (Option<char>, usize) {
    debug_assert!(!bytes.is_empty());
    let expected_len = utf8_char_width(bytes[0]);
    if expected_len == 0 {
        // Not a valid leading byte
        return (None, 1);
    }
    let len = expected_len.min(bytes.len());
    // decode just that one byte
    match std::str::from_utf8(&bytes[..len]) {
        Ok(s) => {
            let c = s.chars().next().unwrap();
            (Some(c), c.len_utf8())
        }
        // Not enough bytes or invalid continuation bytes
        Err(_) => (None, 1),
    }
}

/// Returns the expected length of a UTF-8 character given its leading byte, or 0 if the byte is
/// not a valid leading byte.
///
/// This is a stable equivalent of `std::str::utf8_char_width`, which is gated behind
/// `feature(str_internals)`.
#[inline]
fn utf8_char_width(b: u8) -> usize {
    match b {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bstr_equal_ignoring_whitespace() {
        let a = b"";
        let b = b"";
        assert!(is_equal_ignoring_whitespace(&a[..], &b[..]));

        let a = b"a";
        let b = b"a";
        assert!(is_equal_ignoring_whitespace(&a[..], &b[..]));

        let a = b"a";
        let b = b"b";
        assert!(!is_equal_ignoring_whitespace(&a[..], &b[..]));

        let s = b"\xC2\xA0\t \r\n ";
        let s = str::from_utf8(s).unwrap();
        assert!(s.chars().all(|c| c.is_whitespace()));

        let a = b"   a\t";
        let b = b"\xC2\xA0a\n\t \r\n "; // \u{A0} is non-breaking space
        assert!(is_equal_ignoring_whitespace(&a[..], &b[..]));

        let a = b"a\xFF b";
        let b = b"ab";
        assert!(!is_equal_ignoring_whitespace(&a[..], &b[..]));

        let a = b"a\xFF b";
        let b = b"a\xFFb";
        assert!(is_equal_ignoring_whitespace(&a[..], &b[..]));

        let a = b"a\xFF b";
        let b = b"ac\xFF b";
        assert!(!is_equal_ignoring_whitespace(&a[..], &b[..]));
    }

    #[test]
    fn bstr_equal_ignoring_whitespace_v2() {
        let a = b"";
        let b = b"";
        assert!(is_equal_ignoring_whitespace_v2(&a[..], &b[..]));

        let a = b"a";
        let b = b"a";
        assert!(is_equal_ignoring_whitespace_v2(&a[..], &b[..]));

        let a = b"a";
        let b = b"b";
        assert!(!is_equal_ignoring_whitespace_v2(&a[..], &b[..]));

        let s = b"\xC2\xA0\t \r\n ";
        let s = str::from_utf8(s).unwrap();
        assert!(s.chars().all(|c| c.is_whitespace()));

        let a = b"   a\t";
        let b = b"\xC2\xA0a\n\t \r\n "; // \u{A0} is non-breaking space
        assert!(is_equal_ignoring_whitespace_v2(&a[..], &b[..]));

        let a = b"a\xFF b";
        let b = b"ab";
        assert!(!is_equal_ignoring_whitespace_v2(&a[..], &b[..]));

        let a = b"a\xFF b";
        let b = b"a\xFFb";
        assert!(is_equal_ignoring_whitespace_v2(&a[..], &b[..]));

        let a = b"a\xFF b";
        let b = b"ac\xFF b";
        assert!(!is_equal_ignoring_whitespace_v2(&a[..], &b[..]));
    }
}
