# Authentication

Herostratus needs to clone and fetch Git repositories. For public repositories, no authentication is
needed. For private repositories, Herostratus supports several authentication methods for both SSH
and HTTPS.

## SSH

### SSH agent (default)

If you can run `git clone git@github.com:owner/repo.git` from your terminal, then Herostratus can
clone it too. No configuration needed.

### Explicit SSH key

If you have an SSH private key but no SSH agent (common in CI environments), you can provide the key
path directly. The key must not be passphrase protected.

**CLI:**

```sh
herostratus add git@github.com:owner/private-repo.git --ssh-private-key /path/to/key
```

**config.toml:**

```toml
[repositories.private-repo]
url = "git@github.com:owner/private-repo.git"
ssh_private_key = "/path/to/key"
```

The `remote_username` config option _may_ be set if you need to, but it defaults to `git` for SSH
remotes.

### GIT_SSH_COMMAND

You can also set the `GIT_SSH_COMMAND` environment variable, which overrides the Git
`core.sshCommand` config option. This is useful if you need to pass additional SSH options.

```sh
GIT_SSH_COMMAND="ssh -i /path/to/key -o IdentitiesOnly=yes" herostratus fetch-all
```

## HTTPS

### Git credential helper (default)

If you can run `git clone https://github.com/owner/repo.git` from your terminal, then Herostratus
can clone it too. Herostratus uses the same `credential.helper` chain that Git uses.

To see your current configuration:

```sh
git config --get-all credential.helper
```

See [gitcredentials(7)](https://git-scm.com/docs/gitcredentials) for more information.

### Explicit password / Personal Access Token

For CI environments or when a credential helper is not available, you can provide a username and
password (typically a Personal Access Token) directly.

**CLI:**

```sh
herostratus add https://github.com/owner/private-repo.git \
    --remote-username x-access-token \
    --https-password ghp_your_token_here
```

When using a PAT, the username typically doesn't matter, and it's common to use `x-access-token` as
the username.

**config.toml:**

```toml
[repositories.private-repo]
url = "https://github.com/owner/private-repo.git"
remote_username = "x-access-token"
https_password = "ghp_your_token_here"
```

Both `remote_username` and `https_password` must be set together.
