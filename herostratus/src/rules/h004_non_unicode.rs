use crate::achievement::{Achievement, AchievementDescriptor, Rule, RuleFactory};

pub struct NonUnicode {
    descriptors: [AchievementDescriptor; 1],
}

impl Default for NonUnicode {
    fn default() -> Self {
        Self {
            descriptors: [AchievementDescriptor {
                enabled: true,
                id: 4,
                human_id: "non-unicode",
                name: "But ... How?!",
                description: "Make a commit message containing a non UTF-8 byte",
            }],
        }
    }
}
inventory::submit!(RuleFactory::default::<NonUnicode>());

impl Rule for NonUnicode {
    fn get_descriptors(&self) -> &[AchievementDescriptor] {
        &self.descriptors
    }
    fn get_descriptors_mut(&mut self) -> &mut [AchievementDescriptor] {
        &mut self.descriptors
    }

    fn process(&mut self, commit: &gix::Commit, _repo: &gix::Repository) -> Vec<Achievement> {
        let bytes = commit.message_raw_sloppy();
        let msg = str::from_utf8(bytes);
        if msg.is_err() {
            return vec![Achievement {
                name: self.descriptors[0].name,
                commit: commit.id,
            }];
        }
        Vec::new()
    }
}

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
