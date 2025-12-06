use crate::achievement::{Achievement, Rule, RuleFactory};

#[derive(Default)]
pub struct WhitespaceOnly;

inventory::submit!(RuleFactory::default::<WhitespaceOnly>());

impl Rule for WhitespaceOnly {
    fn id(&self) -> usize {
        6
    }
    fn human_id(&self) -> &'static str {
        "whitespace-only"
    }
    fn name(&self) -> &'static str {
        "Whitespace Warrior"
    }
    fn description(&self) -> &'static str {
        "Make a whitespace-only change"
    }

    fn process(&mut self, commit: &gix::Commit, repo: &gix::Repository) -> Option<Achievement> {
        self.impl_process(commit, repo)
            .inspect_err(|e| {
                tracing::error!(
                    "Error processing commit {} for rule {}: {}",
                    commit.id(),
                    self.pretty_id(),
                    e
                );
            })
            .ok()
            .flatten()
    }
}

impl WhitespaceOnly {
    fn impl_process(
        &self,
        commit: &gix::Commit,
        repo: &gix::Repository,
    ) -> eyre::Result<Option<Achievement>> {
        // Get the parent of the commit, which may not exist if it's the root commit.
        let mut parents = commit.parent_ids();
        let parent = parents.next();
        if parents.next().is_some() {
            // This is a merge commit, and we want to skip it
            return Ok(None);
        }

        let commit_tree = commit.tree()?;
        let parent_tree = match parent {
            Some(pid) => {
                let parent_commit = repo.find_commit(pid)?;
                parent_commit.tree()?
            }
            None => repo.empty_tree(),
        };

        let mut changes = parent_tree.changes()?;
        changes.options(|o| {
            o.track_rewrites(None);
        });

        let mut found_non_whitespace = false;
        // Empty commits won't trigger the on_change callback, so we keep track if any changes were
        // found, because empty commits aren't whitespace changes.
        let mut found_any_change = false;
        // TODO: Does the cache need any custom config options?
        let mut cache = repo.diff_resource_cache_for_tree_diff()?;
        match changes.for_each_to_obtain_tree_with_cache(
            &commit_tree,
            &mut cache,
            |change| -> eyre::Result<gix::object::tree::diff::Action> {
                on_change(
                    repo,
                    change,
                    &mut found_non_whitespace,
                    &mut found_any_change,
                )
            },
        ) {
            Ok(_) => {}
            // It's not an error for the diff iterator to cancel iteration; that means it found a
            // non-whitespace difference, and is short-circuiting.
            Err(gix::object::tree::diff::for_each::Error::Diff(
                gix::diff::tree_with_rewrites::Error::Diff(gix::diff::tree::Error::Cancelled),
            )) => {}
            Err(e) => return Err(e.into()),
        }

        if found_non_whitespace || !found_any_change {
            Ok(None)
        } else {
            Ok(Some(self.grant(commit, repo)))
        }
    }
}

fn on_change(
    repo: &gix::Repository,
    change: gix::object::tree::diff::Change,
    found_non_whitespace: &mut bool,
    found_any_change: &mut bool,
) -> eyre::Result<gix::object::tree::diff::Action> {
    *found_any_change = true;
    match change {
        gix::object::tree::diff::Change::Modification {
            previous_id, id, ..
        } => on_modification(repo, previous_id, id, found_non_whitespace),
        _ => {
            *found_non_whitespace = true;
            Ok(gix::object::tree::diff::Action::Cancel)
        }
    }
}

fn on_modification(
    repo: &gix::Repository,
    previous_id: gix::Id,
    id: gix::Id,
    found_non_whitespace: &mut bool,
) -> eyre::Result<gix::object::tree::diff::Action> {
    let before = repo.find_object(previous_id).unwrap();
    let after = repo.find_object(id).unwrap();
    if before.kind == gix::object::Kind::Tree {
        return Ok(gix::object::tree::diff::Action::Continue);
    }

    let before_s = gix::bstr::BStr::new(&before.data);
    let after_s = gix::bstr::BStr::new(&after.data);

    // tracing::debug!("Diffing {before:?} and {after:?}");
    // tracing::debug!("Before: {before_s:?}");
    // tracing::debug!("After: {after_s:?}");

    if !is_equal_ignoring_whitespace(before_s, after_s) {
        *found_non_whitespace = true;
        Ok(gix::object::tree::diff::Action::Cancel)
    } else {
        Ok(gix::object::tree::diff::Action::Continue)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharOrByte {
    Char(char),
    Byte(u8),
}

struct WhiteSpaceSkipper<'a, I> {
    chunks: &'a mut I,
    current_valid: std::str::Chars<'a>,
    current_invalid: std::slice::Iter<'a, u8>,
}

// This might not be very cache or branch predictor friendlt, but it's simple enough until
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

/// Do a two-finger comparison of `a` and `b` skipping over all whitespace
fn is_equal_ignoring_whitespace<A: AsRef<gix::bstr::BStr>, B: AsRef<gix::bstr::BStr>>(
    a: A,
    b: B,
) -> bool {
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
