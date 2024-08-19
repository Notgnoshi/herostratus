//! The achievements builtin to Herostratus
// TODO: Figure out an easier / better way to organize rules
mod h001_fixup;

use crate::achievement::Rule;

pub fn builtin_rules() -> Vec<Box<dyn Rule>> {
    // TODO: Rule factory? How to get Rules to register themselves? Through a static singleton +
    // module ctor? Metaprogramming? Proc Macro?
    vec![Box::new(h001_fixup::Fixup) as Box<dyn Rule>]
}
