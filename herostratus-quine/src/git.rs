use sha1::{Digest, Sha1};

fn make_empty_commit(
    repo: &git2::Repository,
    sig: &git2::Signature,
    msg: &str,
) -> eyre::Result<git2::Oid> {
    let mut index = repo.index()?;
    let head = repo.find_reference("HEAD")?;
    let parent = head.peel_to_commit();
    let parents = if let Ok(ref parent) = parent {
        vec![parent]
    } else {
        Vec::new()
    };

    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;

    let oid = repo.commit(Some("HEAD"), sig, sig, msg, &tree, &parents)?;
    Ok(oid)
}

/// Generate the initial commit
///
/// If `try_replace` is true, replace the existing commit, if it looks like a quine, otherwise make a
/// new commit.
pub fn generate_initial_commit(
    repo: &git2::Repository,
    prefix_length: u8,
) -> eyre::Result<git2::Commit> {
    let who = repo.signature()?; // extracts default signature from repository/global config
    tracing::debug!("Making quine commit with Author and Committer signature '{who}'");

    let hash_placeholder = "X".repeat(prefix_length as usize);

    let message = format!("Quine: {hash_placeholder}");
    let oid = make_empty_commit(repo, &who, &message)?;
    let commit = repo.find_commit(oid)?;
    Ok(commit)
}

/// The raw commit is the string that is used to generate the commit hash
///
/// It's format is
///
/// ```text
/// commit=HEAD
/// len=$(git cat-file $commit | wc -c)
/// printf "commit %s\0" len; git cat-file commit $commit
/// ```
///
/// which you can pipe into `sha1sum` to verify
///
/// ```text
/// (printf "commit %s\0" $(git --no-replace-objects cat-file commit HEAD | wc -c); git cat-file commit HEAD) | sha1sum
/// ```
///
/// Note that this does *not* contain the "diff" of the commit. (Commits are snapshots, not diffs
/// anyways). But it still does depend on the committed content, because this string contains the
/// commit of the tree, which itself depends on the committed content.
///
/// See:
/// * <https://gist.github.com/masak/2415865>
/// * <https://stackoverflow.com/questions/35430584/how-is-the-git-hash-calculated>
pub fn get_raw_commit(commit: &git2::Commit) -> String {
    let cat_file_contents = cat_file(commit);
    let num_bytes = cat_file_contents.bytes().len();
    format!("commit {num_bytes}\0{cat_file_contents}")
}

fn fmt_signature(sig: &git2::Signature) -> String {
    let offset = sig.when().offset_minutes();
    let (sign, offset) = if offset < 0 {
        ('-', -offset)
    } else {
        ('+', offset)
    };
    // UTC offset could be a partial hour (time zones are stupid).
    let (hours, minutes) = (offset / 60, offset % 60);
    format!(
        "{} {} {}{:02}{:02}",
        sig,
        sig.when().seconds(),
        sign,
        hours,
        minutes
    )
}
fn cat_file(commit: &git2::Commit) -> String {
    let mut result = format!("tree {}\n", commit.tree_id());
    for parent in commit.parent_ids() {
        result += format!("parent {parent}\n").as_str();
    }
    result += format!("author {}\n", fmt_signature(&commit.author())).as_str();
    result += format!("committer {}\n", fmt_signature(&commit.committer())).as_str();
    result += "\n";
    result += commit
        .message_raw()
        .expect("Commit contained non-utf-8 data");
    result
}

pub fn sha1(raw: &str) -> git2::Oid {
    let mut hasher = Sha1::new();
    hasher.update(raw.as_bytes());
    let hash = hasher.finalize();
    git2::Oid::from_bytes(hash.as_slice()).expect("Failed to create OID")
}
