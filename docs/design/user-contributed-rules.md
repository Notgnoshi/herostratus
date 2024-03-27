# User contributed rules
**Status:** In consideration

The method for consuming user-provided rules depends on the language used in the primary
implementation. I'm leaning towards either Rust or Python. Python would be far easier to implement
user-contributed rules, but my language preference is Rust.

* Plugins:
    * If in Rust, this could be dylibs. This gets challenging because of the lack of a stable ABI.
      There are many approaches to providing dylib plugins, which is a pretty interesting topic for
      me personally, but would be quite a lot of work.
    * If in Python, this could be similar to
      <https://jorisroovers.com/gitlint/latest/rules/user_defined_rules/> where you import a
      `herostratus.rules.AchievementRule` interface, and then implement it.
* Consume executables that take the commits from `stdin`, and write the achievements (in JSON?) to
  `stdout`
    * Require the scripts consume a stream of commits? Or a single commit? Probably a single commit,
      so that herostratus can provide them the contents of `git show`.
    * There should be a way to tell herostratus that it shouldn't provide the full diff output to
      the script
