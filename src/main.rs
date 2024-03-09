use clap::Parser;

#[derive(Debug, Parser)]
#[clap(about, verbatim_doc_comment, version)]
struct CliArgs {
    /// A path to a work tree or bare repository, or a clone URL
    repository: String,
}

fn main() {
    let args = CliArgs::parse();
}
