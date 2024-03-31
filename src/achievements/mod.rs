//! The achievements builtin to Herostratus
use eyre::WrapErr;

use crate::achievement::Rule;

pub fn builtin_rules() -> Vec<Box<dyn Rule>> {
    // TODO: Rule factory? How to get Rules to register themselves? Through a static singleton +
    // module ctor? Metaprogramming? Proc Macro?
    Vec::new()
}
