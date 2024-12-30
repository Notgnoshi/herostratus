pub mod clone;
pub mod rev;

pub fn git2_to_gix(from: &git2::Repository) -> gix::Repository {
    gix::discover(from.path()).expect("Failed to discover gix::Repository from a git2::Repository")
}

pub fn gix_to_git2(from: &gix::Repository) -> git2::Repository {
    git2::Repository::discover(from.path())
        .expect("Failed to discover git2::Repository from a gix::Repository")
}
