# Caching
**Status:** In consideration

## Goals
* Enable tracking achievements over time, by accelerating re-processing the same branch at a later
  date
* **Question:** Should the caching still provide acceleration when new achievements are added?

## Approaches

No matter what, the caching should be done per-remote. That is, there should not be a distinct cache
file for each remote.

### Cache file
The cache should probably be a sqlite database (perhaps the same db that achievements are stored
in?). Or maybe it should be separate, for simplicity, and ease of blowing away the cache?

You should be able to pass `--clear-cache`, `--cache <CACHE>`, `--no-cache` CLI options.

The cache should be loaded into memory, so that processing a repository minimizes file I/O.

Alternatively, we have a directory structure like
```
aa/
    00/
        aa001122334455667788.txt
```
Where the text file contains the committer date, the rules that were processed on this commit, and
maybe the repository remote?

### How to identify a repository?
* By remote URL?
    * How to pick the right remote?
    * How to handle repositories that might not have remotes?
* By filesystem path (if we're not pointed at a remote, but a work tree or bare repo instead)?
    * Maybe the initial commit could be the ID?
    * Disallow moving / renaming local checkouts?

### Store each processed rule
Load the processed rule ID's into an ordered set. This should be done regardless of the commit
caching strategy.

### Strategy 1: Cache the last commit processed

Probably works best in a linear history. May result in unnecessary re-processing. Easiest and
simplest to implement.

### Strategy 2: Store each processed commit
Load the processed commits in an ordered set. Order could be hash, or committer date.
