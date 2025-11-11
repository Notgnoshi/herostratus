use crate::achievement::{Achievement, Rule, RuleFactory};

#[derive(Default)]
pub struct NonUnicode;
inventory::submit!(RuleFactory::default::<NonUnicode>());

impl Rule for NonUnicode {
    fn id(&self) -> usize {
        4
    }
    fn human_id(&self) -> &'static str {
        "non-unicode"
    }
    fn name(&self) -> &'static str {
        "But ... How?!"
    }
    fn description(&self) -> &'static str {
        "Make a commit message containing a non UTF-8 byte"
    }

    fn process(&mut self, commit: &gix::Commit, repo: &gix::Repository) -> Option<Achievement> {
        let bytes = commit.message_raw_sloppy();
        let msg = str::from_utf8(bytes);
        if msg.is_err() {
            return Some(self.grant(commit, repo));
        }
        None
    }
}

// TODO: I think this is possible with gitoxide?
// NOTE: It's not possible to create a commit containing non-unicode bytes from git2, so there's a
// test/non-unicode branch with a hand-crafted commit and a tests/h004_non_unicode.rs integration
// test.
//
// This branch was created like
//
//     git checkout --orphan test/non-unicode
//     git rm -rf .
//     # can be anything that's not UTF-8
//     git -c i18n.commitEncoding=FUBAR
//
// and then in vim
//
//    :set binary
//    :%!xxd
//    # change placeholder bytes to FF
//    :%!xxd -r
//    :w
//
// and then verified with
//
//    xxd .git/COMMIT_EDITMSG
//    git cat-file -p HEAD | xxd
//
// NOTE: Unless you use the i18n.commitEncoding configuration, git cat-file -p HEAD will contain
// C3BF characters, even if .git/COMMIT_EDITMSG contains the expected FF bytes.
