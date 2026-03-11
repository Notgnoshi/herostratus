use std::mem::Discriminant;

use crate::achievement::{AchievementKind, Grant, Meta};
use crate::config::RulesConfig;
use crate::observer::{CommitContext, Observation};
use crate::rules::rule::Rule;
use crate::rules::rule_plugin::RuleFactory;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct H013Config {
    /// Minimum hex token length to consider a match (inclusive).
    pub min_matched_chars: usize,
    /// Maximum hex token length to consider a match (inclusive).
    pub max_matched_chars: usize,
}

impl Default for H013Config {
    fn default() -> Self {
        Self {
            min_matched_chars: 7,
            max_matched_chars: 19,
        }
    }
}

const META: Meta = Meta {
    id: 13,
    human_id: "fortune-teller",
    name: "Fortune Teller",
    description: "A commit message that predicts a future commit's hash",
    kind: AchievementKind::PerUser { recurrent: true },
};

struct Token {
    token: String,
    source_oid: String,
    author_name: String,
    author_email: String,
    /// Index into `visited_oids`. Commits at indices `0..future_oid_cutoff` are descendants
    /// (future commits) of this token's source commit. The source commit itself is at
    /// `visited_oids[future_oid_cutoff]`, excluded from the search.
    future_oid_cutoff: usize,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct CachedToken {
    token: String,
    source_oid: String,
    author_name: String,
    author_email: String,
}

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct FortuneTellerCache {
    tokens: Vec<CachedToken>,
}

/// Grant an achievement when a commit message contains a hex string that turns out to be a prefix
/// of a future (descendant) commit's hash.
///
/// Since commits are processed newest-first, when we reach commit A every already-processed commit
/// is a descendant of A -- exactly the set of "future" commits.
pub struct FortuneTeller {
    min_matched_chars: usize,
    max_matched_chars: usize,
    visited_oids: Vec<gix::ObjectId>,
    tokens: Vec<Token>,
}

impl Default for FortuneTeller {
    fn default() -> Self {
        let config = H013Config::default();
        Self {
            min_matched_chars: config.min_matched_chars,
            max_matched_chars: config.max_matched_chars,
            visited_oids: Vec::new(),
            tokens: Vec::new(),
        }
    }
}

fn fortune_teller_factory(config: &RulesConfig) -> Box<dyn crate::rules::rule_plugin::RulePlugin> {
    let h013 = config
        .h13_fortune_teller
        .as_ref()
        .cloned()
        .unwrap_or_default();
    Box::new(FortuneTeller {
        min_matched_chars: h013.min_matched_chars,
        max_matched_chars: h013.max_matched_chars,
        ..Default::default()
    })
}
inventory::submit!(RuleFactory::new(fortune_teller_factory));

impl Rule for FortuneTeller {
    type Cache = FortuneTellerCache;

    fn meta(&self) -> &Meta {
        &META
    }

    fn consumes(&self) -> &'static [Discriminant<Observation>] {
        &[Observation::HEX_TOKENS]
    }

    fn commit_start(&mut self, ctx: &CommitContext) -> eyre::Result<()> {
        self.visited_oids.push(ctx.oid);
        Ok(())
    }

    fn process(&mut self, ctx: &CommitContext, obs: &Observation) -> eyre::Result<Option<Grant>> {
        let Observation::HexTokens { tokens } = obs else {
            return Ok(None);
        };

        for token in tokens {
            if token.len() < self.min_matched_chars || token.len() > self.max_matched_chars {
                continue;
            }
            self.tokens.push(Token {
                token: token.clone(),
                source_oid: ctx.oid.to_string(),
                author_name: ctx.author_name.clone(),
                author_email: ctx.author_email.clone(),
                future_oid_cutoff: self.visited_oids.len() - 1,
            });
        }

        Ok(None)
    }

    fn finalize(&mut self) -> eyre::Result<Option<Grant>> {
        let visited_len = self.visited_oids.len();

        for token in &self.tokens {
            let cutoff = token.future_oid_cutoff.min(visited_len);
            let future_oids = &self.visited_oids[..cutoff];

            for oid in future_oids {
                let hex = oid.to_string();
                if hex.starts_with(&token.token) {
                    return Ok(Some(Grant {
                        commit: gix::ObjectId::from_hex(token.source_oid.as_bytes())
                            .unwrap_or_else(|_| gix::ObjectId::null(gix::hash::Kind::Sha1)),
                        author_name: token.author_name.clone(),
                        author_email: token.author_email.clone(),
                    }));
                }
            }
        }

        Ok(None)
    }

    fn init_cache(&mut self, cache: Self::Cache) {
        for ct in cache.tokens {
            self.tokens.push(Token {
                token: ct.token,
                source_oid: ct.source_oid,
                author_name: ct.author_name,
                author_email: ct.author_email,
                future_oid_cutoff: usize::MAX,
            });
        }
    }

    fn fini_cache(&self) -> Self::Cache {
        let tokens = self
            .tokens
            .iter()
            .filter(|t| {
                // Remove tokens that are prefixes of any visited oid -- they are references to
                // known commits (cherry-picks, reverts, etc.)
                !self
                    .visited_oids
                    .iter()
                    .any(|oid| oid.to_string().starts_with(&t.token))
            })
            .map(|t| CachedToken {
                token: t.token.clone(),
                source_oid: t.source_oid.clone(),
                author_name: t.author_name.clone(),
                author_email: t.author_email.clone(),
            })
            .collect::<Vec<_>>();

        let size_bytes: usize = tokens
            .iter()
            .map(|t| {
                t.token.len() + t.source_oid.len() + t.author_name.len() + t.author_email.len()
            })
            .sum();
        tracing::info!(
            "fortune-teller cache: {} orphan tokens, ~{} bytes",
            tokens.len(),
            size_bytes,
        );

        FortuneTellerCache { tokens }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn ctx_with_oid(name: &str, hex: &str) -> CommitContext {
        let padded = format!("{:0<40}", hex);
        let mut ctx = CommitContext::test(name);
        ctx.oid = gix::ObjectId::from_hex(padded.as_bytes()).unwrap();
        ctx
    }

    #[test]
    fn within_run_match_grants() {
        let mut rule = FortuneTeller {
            min_matched_chars: 7,
            max_matched_chars: 19,
            ..Default::default()
        };

        // Process newest commit first (the "future" commit)
        let future_ctx = ctx_with_oid("Future", "abcdef1234");
        rule.commit_start(&future_ctx).unwrap();

        // Process older commit whose message contains the future commit's hash prefix
        let old_ctx = ctx_with_oid("Alice", "1111111111");
        rule.commit_start(&old_ctx).unwrap();
        let obs = Observation::HexTokens {
            tokens: vec!["abcdef1".to_string()],
        };
        rule.process(&old_ctx, &obs).unwrap();

        let grant = rule.finalize().unwrap();
        assert!(grant.is_some());
        let grant = grant.unwrap();
        assert_eq!(grant.author_name, "Alice");
        assert_eq!(grant.author_email, "alice@example.com");
    }

    #[test]
    fn self_match_excluded() {
        let mut rule = FortuneTeller {
            min_matched_chars: 7,
            max_matched_chars: 19,
            ..Default::default()
        };

        // Only one commit -- its own token can't match itself
        let ctx = ctx_with_oid("Alice", "abcdef1234");
        rule.commit_start(&ctx).unwrap();
        let obs = Observation::HexTokens {
            tokens: vec!["abcdef1".to_string()],
        };
        rule.process(&ctx, &obs).unwrap();

        let grant = rule.finalize().unwrap();
        assert!(grant.is_none(), "Self-match should be excluded");
    }

    #[test]
    fn below_threshold_ignored() {
        let mut rule = FortuneTeller {
            min_matched_chars: 7,
            max_matched_chars: 19,
            ..Default::default()
        };

        let future_ctx = ctx_with_oid("Future", "abcdef1234");
        rule.commit_start(&future_ctx).unwrap();

        let old_ctx = ctx_with_oid("Alice", "1111111111");
        rule.commit_start(&old_ctx).unwrap();
        // Token is only 5 chars, below min_matched_chars of 7
        let obs = Observation::HexTokens {
            tokens: vec!["abcde".to_string()],
        };
        rule.process(&old_ctx, &obs).unwrap();

        let grant = rule.finalize().unwrap();
        assert!(grant.is_none(), "Below-threshold token should be ignored");
    }

    #[test]
    fn above_max_threshold_ignored() {
        let mut rule = FortuneTeller {
            min_matched_chars: 7,
            max_matched_chars: 10,
            ..Default::default()
        };

        let future_ctx = ctx_with_oid("Future", "abcdef1234567890ab");
        rule.commit_start(&future_ctx).unwrap();

        let old_ctx = ctx_with_oid("Alice", "1111111111");
        rule.commit_start(&old_ctx).unwrap();
        // Token is 15 chars, above max_matched_chars of 10
        let obs = Observation::HexTokens {
            tokens: vec!["abcdef123456789".to_string()],
        };
        rule.process(&old_ctx, &obs).unwrap();

        let grant = rule.finalize().unwrap();
        assert!(
            grant.is_none(),
            "Above-max-threshold token should be ignored"
        );
    }

    #[test]
    fn no_match_returns_none() {
        let mut rule = FortuneTeller {
            min_matched_chars: 7,
            max_matched_chars: 19,
            ..Default::default()
        };

        let future_ctx = ctx_with_oid("Future", "abcdef1234");
        rule.commit_start(&future_ctx).unwrap();

        let old_ctx = ctx_with_oid("Alice", "1111111111");
        rule.commit_start(&old_ctx).unwrap();
        let obs = Observation::HexTokens {
            tokens: vec!["9999999".to_string()],
        };
        rule.process(&old_ctx, &obs).unwrap();

        let grant = rule.finalize().unwrap();
        assert!(grant.is_none());
    }

    #[test]
    fn cached_token_matches_visited_oid() {
        let mut rule = FortuneTeller {
            min_matched_chars: 7,
            max_matched_chars: 19,
            ..Default::default()
        };

        // Load a cached token from a previous run
        let cache = FortuneTellerCache {
            tokens: vec![CachedToken {
                token: "abcdef1".to_string(),
                source_oid: format!("{:0<40}", "2222222222"),
                author_name: "Alice".to_string(),
                author_email: "alice@example.com".to_string(),
            }],
        };
        rule.init_cache(cache);

        // In this run, we process a commit whose hash matches the cached token
        let future_ctx = ctx_with_oid("Future", "abcdef1234");
        rule.commit_start(&future_ctx).unwrap();

        let grant = rule.finalize().unwrap();
        assert!(grant.is_some());
        let grant = grant.unwrap();
        assert_eq!(grant.author_name, "Alice");
        assert_eq!(grant.author_email, "alice@example.com");
    }

    #[test]
    fn fini_cache_filters_matched_tokens() {
        let mut rule = FortuneTeller {
            min_matched_chars: 7,
            max_matched_chars: 19,
            ..Default::default()
        };

        // A visited oid that starts with "abcdef1"
        let ctx = ctx_with_oid("Future", "abcdef1234");
        rule.commit_start(&ctx).unwrap();

        // A token that is a prefix of the visited oid -- should be filtered out
        let old_ctx = ctx_with_oid("Alice", "1111111111");
        rule.commit_start(&old_ctx).unwrap();
        let obs = Observation::HexTokens {
            tokens: vec!["abcdef1".to_string()],
        };
        rule.process(&old_ctx, &obs).unwrap();

        let cache = rule.fini_cache();
        assert!(
            cache.tokens.is_empty(),
            "Token matching a visited oid should be filtered from cache"
        );
    }

    #[test]
    fn fini_cache_preserves_unmatched_tokens() {
        let mut rule = FortuneTeller {
            min_matched_chars: 7,
            max_matched_chars: 19,
            ..Default::default()
        };

        let ctx = ctx_with_oid("Future", "1111111111");
        rule.commit_start(&ctx).unwrap();

        let old_ctx = ctx_with_oid("Alice", "2222222222");
        rule.commit_start(&old_ctx).unwrap();
        let obs = Observation::HexTokens {
            tokens: vec!["9999999".to_string()],
        };
        rule.process(&old_ctx, &obs).unwrap();

        let cache = rule.fini_cache();
        assert_eq!(cache.tokens.len(), 1);
        assert_eq!(cache.tokens[0].token, "9999999");
    }

    #[test]
    fn cache_round_trip() {
        let mut rule1 = FortuneTeller {
            min_matched_chars: 7,
            max_matched_chars: 19,
            ..Default::default()
        };

        let ctx = ctx_with_oid("Alice", "1111111111");
        rule1.commit_start(&ctx).unwrap();
        let obs = Observation::HexTokens {
            tokens: vec!["9999999".to_string()],
        };
        rule1.process(&ctx, &obs).unwrap();

        let cache = rule1.fini_cache();

        let mut rule2 = FortuneTeller {
            min_matched_chars: 7,
            max_matched_chars: 19,
            ..Default::default()
        };
        rule2.init_cache(cache);

        // Cached tokens get future_oid_cutoff = usize::MAX
        assert_eq!(rule2.tokens.len(), 1);
        assert_eq!(rule2.tokens[0].future_oid_cutoff, usize::MAX);
        assert_eq!(rule2.tokens[0].token, "9999999");
    }
}
