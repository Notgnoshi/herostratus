use tempfile::{Builder as TempBuilder, TempDir};

pub struct TempRepository {
    pub tempdir: TempDir,
    pub repo: gix::Repository,
}

impl TempRepository {
    pub fn path(&self) -> &std::path::Path {
        self.tempdir.path()
    }

    /// Consume the TempDir without deleting the on-disk repository
    ///
    /// You probably don't want to use this in the final state of a test, but it can be useful for
    /// troubleshooting when things aren't working as you think they should.
    pub fn forget(&mut self) {
        self.tempdir.disable_cleanup(true)
    }

    pub fn remember(&mut self) {
        self.tempdir.disable_cleanup(false)
    }

    /// Start building a commit on this repository
    pub fn commit(&self, subject: &str) -> PendingCommit<'_> {
        PendingCommit {
            repo: &self.repo,
            spec: CommitSpec::new(subject),
        }
    }

    /// Start building a merge commit that merges the given branch into the current branch
    ///
    /// Creates a commit with two parents: HEAD and the tip of `branch_name`.
    pub fn merge(&self, branch_name: &str, subject: &str) -> PendingCommit<'_> {
        let branch_ref = format!("refs/heads/{branch_name}");
        let target = self
            .repo
            .find_reference(&branch_ref)
            .unwrap_or_else(|_| panic!("branch {branch_name:?} not found"))
            .id()
            .detach();
        PendingCommit {
            repo: &self.repo,
            spec: CommitSpec {
                extra_parents: vec![target],
                ..CommitSpec::new(subject)
            },
        }
    }

    /// Switch HEAD to the specified branch, creating it at the current HEAD if necessary
    pub fn set_branch(&self, branch_name: &str) -> eyre::Result<()> {
        set_default_branch(&self.repo, branch_name)
    }

    /// Create a branch pointing at the given target (or HEAD)
    pub fn create_branch(
        &self,
        branch_name: &str,
        target: Option<&str>,
    ) -> eyre::Result<gix::Reference<'_>> {
        create_branch(&self.repo, branch_name, target)
    }

    /// Create a lightweight tag
    pub fn tag(
        &self,
        name: &str,
        target: impl Into<gix::ObjectId>,
    ) -> eyre::Result<gix::Reference<'_>> {
        create_lightweight_tag(&self.repo, name, target)
    }

    /// Create an annotated tag
    pub fn annotated_tag(
        &self,
        name: &str,
        target: impl Into<gix::ObjectId>,
        message: &str,
    ) -> eyre::Result<gix::Reference<'_>> {
        create_annotated_tag(&self.repo, name, target, message)
    }
}

const DEFAULT_TIME: gix::date::SecondsSinceUnixEpoch = 1711656630;
const DEFAULT_NAME: &str = "Herostratus";
const DEFAULT_EMAIL: &str = "Herostratus@example.com";

struct CommitSpec {
    subject: String,
    body: Option<String>,
    author_name: Option<String>,
    author_email: Option<String>,
    committer_name: Option<String>,
    committer_email: Option<String>,
    seconds: Option<gix::date::SecondsSinceUnixEpoch>,
    files: Vec<(String, Vec<u8>)>,
    extra_parents: Vec<gix::ObjectId>,
}

impl CommitSpec {
    fn new(subject: &str) -> Self {
        Self {
            subject: subject.to_owned(),
            body: None,
            author_name: None,
            author_email: None,
            committer_name: None,
            committer_email: None,
            seconds: None,
            files: Vec::new(),
            extra_parents: Vec::new(),
        }
    }

    fn message(&self) -> String {
        match &self.body {
            Some(body) => format!("{}\n\n{body}", self.subject),
            None => self.subject.clone(),
        }
    }

    fn author_signature(&self) -> gix::actor::Signature {
        let seconds = self.seconds.unwrap_or(DEFAULT_TIME);
        let name = self.author_name.as_deref().unwrap_or(DEFAULT_NAME);
        let email = self.author_email.as_deref().unwrap_or(DEFAULT_EMAIL);
        get_signature_at_time_as(seconds, name, email)
    }

    fn committer_signature(&self) -> gix::actor::Signature {
        let seconds = self.seconds.unwrap_or(DEFAULT_TIME);
        let name = self
            .committer_name
            .as_deref()
            .or(self.author_name.as_deref())
            .unwrap_or(DEFAULT_NAME);
        let email = self
            .committer_email
            .as_deref()
            .or(self.author_email.as_deref())
            .unwrap_or(DEFAULT_EMAIL);
        get_signature_at_time_as(seconds, name, email)
    }

    /// Create the commit on the given repository, returning its ID
    fn execute<'r>(&self, repo: &'r gix::Repository) -> eyre::Result<gix::Id<'r>> {
        let tree_id = if self.files.is_empty() {
            repo.head_tree_id()
                .unwrap_or_else(|_| repo.empty_tree().id())
        } else {
            let base_tree_id = repo
                .head_tree_id()
                .unwrap_or_else(|_| repo.empty_tree().id());
            let mut editor = repo.edit_tree(base_tree_id)?;
            for (path, content) in &self.files {
                let blob_id: gix::ObjectId = repo.write_blob(content)?.into();
                editor.upsert(path, gix::object::tree::EntryKind::Blob, blob_id)?;
            }
            editor.write()?
        };

        let author_sig = self.author_signature();
        let committer_sig = self.committer_signature();
        let mut buf_a = gix::date::parse::TimeBuf::default();
        let authored = author_sig.to_ref(&mut buf_a);
        let mut buf_c = gix::date::parse::TimeBuf::default();
        let committed = committer_sig.to_ref(&mut buf_c);

        let mut parents: Vec<gix::Id<'_>> = Vec::new();
        if let Ok(head) = repo.head_commit() {
            parents.push(head.id());
        }
        for oid in &self.extra_parents {
            parents.push(repo.find_commit(*oid)?.id());
        }
        let message = self.message();
        let commit_id = repo.commit_as(committed, authored, "HEAD", &message, tree_id, parents)?;
        Ok(commit_id)
    }
}

pub struct PendingCommit<'r> {
    repo: &'r gix::Repository,
    spec: CommitSpec,
}

impl<'r> PendingCommit<'r> {
    pub fn body(mut self, body: &str) -> Self {
        self.spec.body = Some(body.to_owned());
        self
    }

    pub fn author(mut self, name: &str, email: &str) -> Self {
        self.spec.author_name = Some(name.to_owned());
        self.spec.author_email = Some(email.to_owned());
        self
    }

    pub fn committer(mut self, name: &str, email: &str) -> Self {
        self.spec.committer_name = Some(name.to_owned());
        self.spec.committer_email = Some(email.to_owned());
        self
    }

    pub fn time(mut self, seconds: gix::date::SecondsSinceUnixEpoch) -> Self {
        self.spec.seconds = Some(seconds);
        self
    }

    pub fn file(mut self, path: &str, content: &[u8]) -> Self {
        self.spec.files.push((path.to_owned(), content.to_vec()));
        self
    }

    /// Add an additional parent to this commit by branch name.
    ///
    /// Used together with [TempRepository::merge] to construct merges with more than two
    /// parents (octopus merges). Can be called multiple times to add many parents.
    pub fn with_extra_parent(mut self, branch_name: &str) -> Self {
        let branch_ref = format!("refs/heads/{branch_name}");
        let target = self
            .repo
            .find_reference(&branch_ref)
            .unwrap_or_else(|_| panic!("branch {branch_name:?} not found"))
            .id()
            .detach();
        self.spec.extra_parents.push(target);
        self
    }

    /// Execute: create the commit and return its ID
    pub fn create(self) -> eyre::Result<gix::Id<'r>> {
        self.spec.execute(self.repo)
    }
}

/// Deferred operations for Builder
enum BuildOp {
    Commit(CommitSpec),
    Branch { name: String },
    LightweightTag { name: String },
    AnnotatedTag { name: String, message: String },
}

pub struct Builder {
    bare: bool,
    operations: Vec<BuildOp>,
}

impl Builder {
    /// Create a new builder (bare repository by default)
    pub fn new() -> Self {
        Self {
            bare: true,
            operations: Vec::new(),
        }
    }

    /// Make the repository non-bare (has a worktree)
    pub fn non_bare(mut self) -> Self {
        self.bare = false;
        self
    }

    /// Start building a commit
    pub fn commit(self, subject: &str) -> CommitBuilder {
        CommitBuilder {
            builder: self,
            spec: CommitSpec::new(subject),
        }
    }

    /// Switch HEAD to the named branch
    pub fn branch(mut self, name: &str) -> Self {
        self.operations.push(BuildOp::Branch {
            name: name.to_owned(),
        });
        self
    }

    /// Create a lightweight tag at HEAD
    pub fn tag(mut self, name: &str) -> Self {
        self.operations.push(BuildOp::LightweightTag {
            name: name.to_owned(),
        });
        self
    }

    /// Create an annotated tag at HEAD
    pub fn annotated_tag(mut self, name: &str, message: &str) -> Self {
        self.operations.push(BuildOp::AnnotatedTag {
            name: name.to_owned(),
            message: message.to_owned(),
        });
        self
    }

    /// Build the repository, executing all deferred operations
    pub fn build(self) -> eyre::Result<TempRepository> {
        let temp = if self.bare { bare()? } else { non_bare()? };

        for op in &self.operations {
            match op {
                BuildOp::Commit(spec) => {
                    spec.execute(&temp.repo)?;
                }
                BuildOp::Branch { name } => {
                    set_default_branch(&temp.repo, name)?;
                }
                BuildOp::LightweightTag { name } => {
                    let head = temp.repo.head_id()?;
                    create_lightweight_tag(&temp.repo, name, head)?;
                }
                BuildOp::AnnotatedTag { name, message } => {
                    let head = temp.repo.head_id()?;
                    create_annotated_tag(&temp.repo, name, head, message)?;
                }
            }
        }

        Ok(temp)
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CommitBuilder {
    builder: Builder,
    spec: CommitSpec,
}

impl CommitBuilder {
    pub fn body(mut self, body: &str) -> Self {
        self.spec.body = Some(body.to_owned());
        self
    }

    pub fn author(mut self, name: &str, email: &str) -> Self {
        self.spec.author_name = Some(name.to_owned());
        self.spec.author_email = Some(email.to_owned());
        self
    }

    pub fn committer(mut self, name: &str, email: &str) -> Self {
        self.spec.committer_name = Some(name.to_owned());
        self.spec.committer_email = Some(email.to_owned());
        self
    }

    pub fn time(mut self, seconds: gix::date::SecondsSinceUnixEpoch) -> Self {
        self.spec.seconds = Some(seconds);
        self
    }

    pub fn file(mut self, path: &str, content: &[u8]) -> Self {
        self.spec.files.push((path.to_owned(), content.to_vec()));
        self
    }

    /// Explicitly finalize the current commit and return the Builder
    pub fn finish(mut self) -> Builder {
        self.builder.operations.push(BuildOp::Commit(self.spec));
        self.builder
    }

    // -- Auto-finalize shortcuts --

    /// Finalize the current commit and start building a new one
    pub fn commit(self, subject: &str) -> CommitBuilder {
        self.finish().commit(subject)
    }

    /// Finalize the current commit and switch HEAD to the named branch
    pub fn branch(self, name: &str) -> Builder {
        self.finish().branch(name)
    }

    /// Finalize the current commit and create a lightweight tag at HEAD
    pub fn tag(self, name: &str) -> Builder {
        self.finish().tag(name)
    }

    /// Finalize the current commit and create an annotated tag at HEAD
    pub fn annotated_tag(self, name: &str, message: &str) -> Builder {
        self.finish().annotated_tag(name, message)
    }

    /// Finalize the current commit and build the repository
    pub fn build(self) -> eyre::Result<TempRepository> {
        self.finish().build()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn get_signature_at_time_as(
    seconds: gix::date::SecondsSinceUnixEpoch,
    name: &str,
    email: &str,
) -> gix::actor::Signature {
    let time = gix::date::Time { seconds, offset: 0 };
    gix::actor::Signature {
        name: name.into(),
        email: email.into(),
        time,
    }
}

fn bare() -> eyre::Result<TempRepository> {
    let tempdir = TempBuilder::new().prefix("tmp-").suffix(".git").tempdir()?;
    tracing::debug!(
        "Creating bare repo fixture in '{}'",
        tempdir.path().display()
    );

    let options = gix::create::Options {
        destination_must_be_empty: true,
        ..Default::default()
    };
    let repo = gix::ThreadSafeRepository::init(tempdir.path(), gix::create::Kind::Bare, options)?;
    let repo = repo.to_thread_local();

    Ok(TempRepository { tempdir, repo })
}

fn non_bare() -> eyre::Result<TempRepository> {
    let tempdir = TempBuilder::new().prefix("tmp-").tempdir()?;
    tracing::debug!(
        "Creating non-bare repo fixture in '{}'",
        tempdir.path().display()
    );

    let options = gix::create::Options {
        destination_must_be_empty: true,
        ..Default::default()
    };
    let repo =
        gix::ThreadSafeRepository::init(tempdir.path(), gix::create::Kind::WithWorktree, options)?;
    let repo = repo.to_thread_local();

    Ok(TempRepository { tempdir, repo })
}

/// Return a pair of empty [TempRepository]s with the upstream configured as the "origin" remote of
/// the downstream
pub fn upstream_downstream_empty() -> eyre::Result<(TempRepository, TempRepository)> {
    let upstream = Builder::new().build()?;
    let mut downstream = Builder::new().build()?;
    tracing::debug!(
        "Setting {:?} as upstream remote of {:?}",
        upstream.tempdir.path(),
        downstream.tempdir.path()
    );
    let url = format!("file://{}", upstream.tempdir.path().display());
    let mut config = downstream.repo.config_snapshot_mut();
    config.set_raw_value("remote.origin.url", url.as_bytes())?;
    config.commit()?;
    let _remote = downstream.repo.find_remote("origin")?;
    Ok((upstream, downstream))
}

pub fn upstream_downstream() -> eyre::Result<(TempRepository, TempRepository)> {
    let (upstream, downstream) = upstream_downstream_empty()?;
    upstream.commit("Initial upstream commit").create()?;
    downstream.commit("Initial downstream commit").create()?;
    Ok((upstream, downstream))
}

fn create_branch<'r>(
    repo: &'r gix::Repository,
    branch_name: &str,
    target: Option<&str>,
) -> eyre::Result<gix::Reference<'r>> {
    let target = target.unwrap_or("HEAD");
    tracing::debug!("Creating branch {branch_name:?} -> {target:?}");
    let rev = repo.rev_parse_single(target)?;
    let branch_name = format!("refs/heads/{branch_name}");
    let reference = repo.reference(
        branch_name.as_str(),
        rev,
        gix::refs::transaction::PreviousValue::Any,
        format!("Herostratus: Creating branch {branch_name:?} at {target:?}"),
    )?;
    Ok(reference)
}

/// Switch to the specified branch, creating it at the current HEAD if necessary
#[tracing::instrument(level = "debug", skip_all, fields(path = %repo.path().display()))]
fn set_default_branch(repo: &gix::Repository, branch_name: &str) -> eyre::Result<()> {
    tracing::debug!("Switching to branch {branch_name:?}");
    if repo.try_find_reference(branch_name)?.is_none() {
        // If HEAD doesn't exist yet, we can't create the reference it points to
        if repo.head_id().is_ok() {
            create_branch(repo, branch_name, None)?;
        }
    }

    // Now update the symbolic HEAD ref itself to point to the new branch
    let local_head = gix::refs::FullName::try_from("HEAD")?;
    let new_target = gix::refs::FullName::try_from(format!("refs/heads/{branch_name}"))?;

    let change = gix::refs::transaction::Change::Update {
        log: gix::refs::transaction::LogChange::default(),
        expected: gix::refs::transaction::PreviousValue::Any,
        new: gix::refs::Target::Symbolic(new_target),
    };

    let edit = gix::refs::transaction::RefEdit {
        change,
        name: local_head,
        deref: false,
    };

    repo.edit_reference(edit)?;

    Ok(())
}

#[tracing::instrument(level = "debug", skip_all, fields(path = %repo.path().display()))]
fn create_lightweight_tag<'r>(
    repo: &'r gix::Repository,
    name: &str,
    target: impl Into<gix::ObjectId>,
) -> eyre::Result<gix::Reference<'r>> {
    let reference = repo.tag_reference(
        name,
        target,
        gix::refs::transaction::PreviousValue::MustNotExist,
    )?;
    Ok(reference)
}

#[tracing::instrument(level = "debug", skip_all, fields(path = %repo.path().display()))]
fn create_annotated_tag<'r>(
    repo: &'r gix::Repository,
    name: &str,
    target: impl Into<gix::ObjectId>,
    message: &str,
) -> eyre::Result<gix::Reference<'r>> {
    let signature = get_signature_at_time_as(DEFAULT_TIME, DEFAULT_NAME, DEFAULT_EMAIL);
    let mut buf = gix::date::parse::TimeBuf::default();
    let tagger = signature.to_ref(&mut buf);

    let reference = repo.tag(
        name,
        target.into(),
        gix::objs::Kind::Commit,
        Some(tagger),
        message,
        gix::refs::transaction::PreviousValue::MustNotExist,
    )?;
    Ok(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forget() {
        let mut temp = Builder::new().commit("Initial commit").build().unwrap();
        temp.forget();

        assert!(temp.repo.path().exists());
        let path = temp.tempdir.path().to_path_buf();
        drop(temp);

        assert!(path.exists());
        std::fs::remove_dir_all(&path).unwrap();
        assert!(!path.exists());

        let mut temp = Builder::new().commit("Initial commit").build().unwrap();
        temp.forget();
        temp.remember();
        let path = temp.tempdir.path().to_path_buf();
        drop(temp);
        assert!(!path.exists());
    }

    #[test]
    fn test_bare_repository() {
        let repo = Builder::new().build().unwrap();
        assert!(repo.repo.is_bare());

        let default_branch = repo.repo.head_name().unwrap();
        assert_eq!(
            default_branch,
            Some(gix::refs::FullName::try_from("refs/heads/main").unwrap())
        );
    }

    #[test]
    fn test_set_default_branch() {
        let repo = Builder::new().build().unwrap();
        assert!(repo.repo.is_bare());

        let default_branch = repo.repo.head_name().unwrap();
        assert_eq!(
            default_branch,
            Some(gix::refs::FullName::try_from("refs/heads/main").unwrap())
        );

        repo.set_branch("trunk").unwrap();
        let default_branch = repo.repo.head_name().unwrap();
        assert_eq!(
            default_branch,
            Some(gix::refs::FullName::try_from("refs/heads/trunk").unwrap())
        );
    }

    #[test]
    fn test_add_empty_commits() {
        let repo = Builder::new().build().unwrap();

        let commit1 = repo.commit("commit1").create().unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(head, commit1);

        let commit2 = repo.commit("commit2").create().unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(head, commit2);
    }

    #[test]
    fn test_commits_on_branches() {
        let repo = Builder::new().build().unwrap();

        repo.set_branch("branch1").unwrap();
        let commit1 = repo.commit("commit1 on branch1").create().unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(head, commit1);

        repo.set_branch("branch2").unwrap();
        let commit2 = repo.commit("commit2 on branch2").create().unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(head, commit2);
    }

    #[test]
    fn test_add_empty_commit_as() {
        let repo = Builder::new().build().unwrap();

        let commit = repo
            .commit("custom author")
            .author("Alice", "alice@example.com")
            .create()
            .unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(head, commit);

        let commit_obj = repo.repo.find_commit(commit).unwrap();
        let author = commit_obj.author().unwrap();
        assert_eq!(author.name, "Alice");
        assert_eq!(author.email, "alice@example.com");
    }

    #[test]
    fn test_create_tags() {
        let repo = Builder::new().build().unwrap();
        let commit = repo.commit("commit1").create().unwrap();

        let tag = repo.tag("SMALL_TAG", commit).unwrap();
        assert_eq!(tag.id(), commit);

        let commit = repo.commit("commit2").create().unwrap();
        let mut tag = repo
            .annotated_tag("BIG_TAG", commit, "This is an annotated tag")
            .unwrap();
        assert_ne!(tag.id(), commit, "Annotated tags have their own object IDs");
        let points_to = tag.peel_to_id().unwrap();
        assert_eq!(points_to, commit);
    }

    #[test]
    fn test_builder_empty_bare() {
        let repo = Builder::new().build().unwrap();
        assert!(repo.repo.is_bare());
    }

    #[test]
    fn test_builder_non_bare() {
        let repo = Builder::new().non_bare().build().unwrap();
        assert!(!repo.repo.is_bare());
    }

    #[test]
    fn test_builder_single_commit() {
        let repo = Builder::new().commit("Initial commit").build().unwrap();
        let head = repo.repo.head_commit().unwrap();
        assert_eq!(head.message().unwrap().title, "Initial commit");
    }

    #[test]
    fn test_builder_multiple_commits() {
        let repo = Builder::new()
            .commit("first")
            .commit("second")
            .commit("third")
            .build()
            .unwrap();
        let head = repo.repo.head_commit().unwrap();
        assert_eq!(head.message().unwrap().title, "third");
    }

    #[test]
    fn test_builder_commit_with_author() {
        let repo = Builder::new()
            .commit("test")
            .author("Alice", "alice@example.com")
            .build()
            .unwrap();
        let head = repo.repo.head_commit().unwrap();
        let author = head.author().unwrap();
        assert_eq!(author.name, "Alice");
        assert_eq!(author.email, "alice@example.com");
    }

    #[test]
    fn test_builder_commit_with_time() {
        let repo = Builder::new()
            .commit("test")
            .time(1234567890)
            .build()
            .unwrap();
        let head = repo.repo.head_commit().unwrap();
        let author = head.author().unwrap();
        let time = author.time().unwrap();
        assert_eq!(time.seconds, 1234567890);
    }

    #[test]
    fn test_builder_commit_with_body() {
        let repo = Builder::new()
            .commit("subject")
            .body("This is the body")
            .build()
            .unwrap();
        let head = repo.repo.head_commit().unwrap();
        let msg = head.message().unwrap();
        assert_eq!(msg.title, "subject");
        assert!(msg.body.is_some());
    }

    #[test]
    fn test_builder_commit_with_file() {
        let repo = Builder::new()
            .commit("add file")
            .file("test.txt", b"hello world")
            .build()
            .unwrap();
        let head = repo.repo.head_commit().unwrap();
        let tree = head.tree().unwrap();
        let entry = tree.find_entry("test.txt").unwrap();
        let obj = entry.object().unwrap();
        assert_eq!(obj.data, b"hello world");
    }

    #[test]
    fn test_builder_branch() {
        let repo = Builder::new()
            .commit("Initial commit")
            .branch("dev")
            .commit("on dev")
            .build()
            .unwrap();
        let head_name = repo.repo.head_name().unwrap().unwrap();
        assert_eq!(head_name.as_bstr(), "refs/heads/dev");
    }

    #[test]
    fn test_builder_tags() {
        let repo = Builder::new()
            .commit("c1")
            .tag("v1")
            .commit("c2")
            .annotated_tag("v2", "release v2")
            .build()
            .unwrap();

        let tag = repo.repo.find_reference("v1").unwrap();
        assert!(tag.id() != gix::ObjectId::null(gix::hash::Kind::Sha1));

        let mut tag = repo.repo.find_reference("v2").unwrap();
        let peeled = tag.peel_to_id().unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(peeled, head);
    }

    #[test]
    fn test_builder_committer_defaults_to_author() {
        let repo = Builder::new()
            .commit("test")
            .author("Alice", "alice@example.com")
            .build()
            .unwrap();
        let head = repo.repo.head_commit().unwrap();
        let committer = head.committer().unwrap();
        assert_eq!(committer.name, "Alice");
        assert_eq!(committer.email, "alice@example.com");
    }

    #[test]
    fn test_builder_separate_committer() {
        let repo = Builder::new()
            .commit("test")
            .author("Alice", "alice@example.com")
            .committer("Bob", "bob@example.com")
            .build()
            .unwrap();
        let head = repo.repo.head_commit().unwrap();
        let author = head.author().unwrap();
        assert_eq!(author.name, "Alice");
        let committer = head.committer().unwrap();
        assert_eq!(committer.name, "Bob");
    }

    #[test]
    fn test_pending_commit() {
        let repo = Builder::new().build().unwrap();
        let id = repo.commit("first").create().unwrap();
        let head = repo.repo.head_id().unwrap();
        assert_eq!(head, id);
    }

    #[test]
    fn test_pending_commit_with_author() {
        let repo = Builder::new().build().unwrap();
        repo.commit("test")
            .author("Alice", "alice@example.com")
            .create()
            .unwrap();
        let head = repo.repo.head_commit().unwrap();
        let author = head.author().unwrap();
        assert_eq!(author.name, "Alice");
    }

    #[test]
    fn test_temp_repo_set_branch() {
        let repo = Builder::new().commit("Initial commit").build().unwrap();
        repo.set_branch("dev").unwrap();
        let head_name = repo.repo.head_name().unwrap().unwrap();
        assert_eq!(head_name.as_bstr(), "refs/heads/dev");
    }

    #[test]
    fn test_temp_repo_tags() {
        let repo = Builder::new().build().unwrap();
        let id = repo.commit("c1").create().unwrap();
        let tag = repo.tag("v1", id).unwrap();
        assert_eq!(tag.id(), id);

        let id2 = repo.commit("c2").create().unwrap();
        let mut tag = repo.annotated_tag("v2", id2, "release").unwrap();
        let peeled = tag.peel_to_id().unwrap();
        assert_eq!(peeled, id2);
    }

    #[test]
    fn test_merge_with_extra_parent_creates_three_parent_commit() {
        //  *-.   octopus
        //  |\ \
        //  | | * on side2
        //  | |/
        //  | * on side1
        //  |/
        //  * Initial commit
        let temp = Builder::new().commit("Initial commit").build().unwrap();

        temp.set_branch("side1").unwrap();
        temp.commit("on side1").create().unwrap();
        temp.set_branch("side2").unwrap();
        temp.commit("on side2").create().unwrap();

        // on the main branch, merge side1 and side2 into main
        temp.set_branch("main").unwrap();
        temp.merge("side1", "octopus")
            .with_extra_parent("side2")
            .create()
            .unwrap();

        let head = temp.repo.head_id().unwrap();
        let commit = head.object().unwrap().into_commit();
        assert_eq!(commit.parent_ids().count(), 3);
    }
}
