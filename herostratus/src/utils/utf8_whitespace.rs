use crate::bstr::BStr;

/// Do a two-finger comparison of `a` and `b` skipping over all unicode whitespace
pub fn is_equal_ignoring_whitespace<A: AsRef<BStr>, B: AsRef<BStr>>(a: A, b: B) -> bool {
    let mut a_chunks = a.as_ref().utf8_chunks();
    let mut a_chars = WhiteSpaceSkipper::new(&mut a_chunks);
    let mut b_chunks = b.as_ref().utf8_chunks();
    let mut b_chars = WhiteSpaceSkipper::new(&mut b_chunks);
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
pub enum CharOrByte {
    Char(char),
    Byte(u8),
}

/// A utility to skip over ascii and unicode whitespace in a [`BStr`]
pub struct WhiteSpaceSkipper<'a, I> {
    chunks: &'a mut I,
    current_valid: std::str::Chars<'a>,
    current_invalid: std::slice::Iter<'a, u8>,
}

// This might not be very cache or branch predictor friendly, but it's simple enough until
// performance becomes a concern.
impl<'a, I> WhiteSpaceSkipper<'a, I>
where
    I: Iterator<Item = std::str::Utf8Chunk<'a>>,
{
    pub fn new(chunks: &'a mut I) -> Self {
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

impl<'a, I> Iterator for WhiteSpaceSkipper<'a, I>
where
    I: Iterator<Item = std::str::Utf8Chunk<'a>>,
{
    type Item = CharOrByte;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_non_whitespace()
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
}
