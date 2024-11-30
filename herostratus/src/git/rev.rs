use eyre::WrapErr;
use git2::{ObjectType, Oid, Repository, Sort};

pub fn parse(reference: &str, repo: &Repository) -> eyre::Result<Oid> {
    let object = repo
        .revparse_single(reference)
        .wrap_err("Failed to rev-parse")?;
    let oid = object.id();
    tracing::debug!(
        "Resolved {reference:?} to {:?} {oid:?}",
        object.kind().unwrap_or(ObjectType::Any)
    );
    Ok(oid)
}

pub fn walk(
    oid: Oid,
    repo: &Repository,
) -> eyre::Result<impl Iterator<Item = eyre::Result<Oid>> + '_> {
    let mut revwalk = repo.revwalk().wrap_err("Could not walk repository")?;
    revwalk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)?;
    revwalk.push(oid)?;

    Ok(revwalk.map(|r| r.wrap_err("Failed to yield next rev")))
}
