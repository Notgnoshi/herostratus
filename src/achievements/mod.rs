//! The achievements builtin to Herostratus
mod h001_fixup;
mod h002_shortest_subject_line;

use crate::achievement::Rule;

pub fn builtin_rules() -> Vec<Box<dyn Rule>> {
    // TODO: Rule factory? How to get Rules to register themselves? Through a static singleton +
    // module ctor? Metaprogramming? Proc Macro?
    vec![
        Box::new(h001_fixup::Fixup) as Box<dyn Rule>,
        Box::new(h002_shortest_subject_line::ShortestSubjectLine::default()) as Box<dyn Rule>,
    ]
}
