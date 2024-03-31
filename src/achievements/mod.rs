//! The achievements builtin to Herostratus
// TODO: Figure out an easier / better way to organize rules
mod fixup;

use crate::achievement::Rule;

pub fn builtin_rules() -> Vec<Box<dyn Rule>> {
    // TODO: Rule factory? How to get Rules to register themselves? Through a static singleton +
    // module ctor? Metaprogramming? Proc Macro?
    vec![Box::new(fixup::IMeantToFixThatUpLaterISwear) as Box<dyn Rule>]
}
