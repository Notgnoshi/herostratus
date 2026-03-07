use std::mem::Discriminant;

use gix::bstr::ByteSlice;

use crate::observer::observation::Observation;
use crate::observer::observer::Observer;
use crate::observer::observer_factory::ObserverFactory;

/// Minimum hex token length the observer will emit.
const MIN_TOKEN_LEN: usize = 5;

/// Maximum hex token length the observer will emit. Tokens at or above this length are almost
/// certainly full hash references to existing commits and are discarded.
const MAX_TOKEN_LEN: usize = 20;

/// Extract contiguous hex-digit tokens from `text`, lowercased, with length in [MIN_TOKEN_LEN,
/// MAX_TOKEN_LEN).
///
/// Respects word boundaries: a hex run is only emitted when both the preceding and following
/// characters are non-alphanumeric (or string start/end). This prevents extracting hex substrings
/// from normal words while still allowing tokens delimited by punctuation like `[deadbeef]`.
fn extract_hex_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    // Was the character immediately before the current hex run alphanumeric?
    // Initialized to false so that string-start counts as a valid boundary.
    let mut preceded_by_alnum = false;

    for ch in text.chars() {
        if ch.is_ascii_hexdigit() {
            current.push(ch.to_ascii_lowercase());
        } else {
            if !current.is_empty() {
                let valid_len = current.len() >= MIN_TOKEN_LEN && current.len() < MAX_TOKEN_LEN;
                let valid_boundary = !preceded_by_alnum && !ch.is_alphanumeric();
                if valid_len && valid_boundary {
                    tokens.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            }
            preceded_by_alnum = ch.is_alphanumeric();
        }
    }

    // Flush trailing token (end-of-string is a valid boundary)
    if !preceded_by_alnum && current.len() >= MIN_TOKEN_LEN && current.len() < MAX_TOKEN_LEN {
        tokens.push(current);
    }

    tokens
}

/// Emits [Observation::HexTokens] when the commit message contains hex-digit tokens suitable for
/// fortune-teller matching.
#[derive(Default)]
pub struct HexTokensObserver;

inventory::submit!(ObserverFactory::new::<HexTokensObserver>());

impl Observer for HexTokensObserver {
    fn emits(&self) -> Discriminant<Observation> {
        Observation::HEX_TOKENS
    }

    fn on_commit(
        &mut self,
        commit: &gix::Commit,
        _repo: &gix::Repository,
    ) -> eyre::Result<Option<Observation>> {
        let msg = commit.message_raw()?;
        let text = msg.to_str_lossy();
        let tokens = extract_hex_tokens(&text);

        if tokens.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Observation::HexTokens { tokens }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_message_no_tokens() {
        let tokens = extract_hex_tokens("Just a normal commit message");
        assert!(tokens.is_empty());
    }

    #[test]
    fn single_hex_token_lowercased() {
        let tokens = extract_hex_tokens("See commit ABCDE12 for details");
        assert_eq!(tokens, vec!["abcde12"]);
    }

    #[test]
    fn multiple_tokens_extracted() {
        let tokens = extract_hex_tokens("Compare abcdef0 and 1234567");
        assert_eq!(tokens, vec!["abcdef0", "1234567"]);
    }

    #[test]
    fn tokens_below_min_ignored() {
        let tokens = extract_hex_tokens("Short abcd token");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokens_at_max_ignored() {
        // 20 or more hex chars should be ignored
        let tokens = extract_hex_tokens("Hash: abcdef0123456789abcd done");
        assert!(tokens.is_empty());
    }

    #[test]
    fn trailing_hex_flushed() {
        let tokens = extract_hex_tokens("Ends with abcde12");
        assert_eq!(tokens, vec!["abcde12"]);
    }

    #[test]
    fn mixed_case_lowercased() {
        let tokens = extract_hex_tokens("Token AbCdEf1");
        assert_eq!(tokens, vec!["abcdef1"]);
    }

    #[test]
    fn boundary_length_five() {
        let tokens = extract_hex_tokens("Exactly abcde five");
        assert_eq!(tokens, vec!["abcde"]);
    }

    #[test]
    fn boundary_length_nineteen() {
        // 19 hex chars -- should be included
        let tokens = extract_hex_tokens("Hash: abcdef0123456789abc done");
        assert_eq!(tokens, vec!["abcdef0123456789abc"]);
    }

    #[test]
    fn hex_embedded_in_word_rejected() {
        // "Seedeadbeef" -- hex chars embedded in a larger word
        let tokens = extract_hex_tokens("Seedeadbeef123 is not a hash");
        assert!(tokens.is_empty());
    }

    #[test]
    fn hex_preceded_by_alpha_rejected() {
        let tokens = extract_hex_tokens("xabcdef1 end");
        assert!(tokens.is_empty());
    }

    #[test]
    fn hex_followed_by_alpha_rejected() {
        let tokens = extract_hex_tokens("start abcdef1z");
        assert!(tokens.is_empty());
    }

    #[test]
    fn hex_in_brackets() {
        let tokens = extract_hex_tokens("fix [deadbeef1] issue");
        assert_eq!(tokens, vec!["deadbeef1"]);
    }

    #[test]
    fn hex_in_parens() {
        let tokens = extract_hex_tokens("revert (abcdef1)");
        assert_eq!(tokens, vec!["abcdef1"]);
    }

    #[test]
    fn hex_after_colon() {
        let tokens = extract_hex_tokens("commit:abcdef1 done");
        assert_eq!(tokens, vec!["abcdef1"]);
    }

    #[test]
    fn hex_at_string_start() {
        let tokens = extract_hex_tokens("abcdef1 is the hash");
        assert_eq!(tokens, vec!["abcdef1"]);
    }

    #[test]
    fn hex_at_string_end() {
        let tokens = extract_hex_tokens("the hash is abcdef1");
        assert_eq!(tokens, vec!["abcdef1"]);
    }

    #[test]
    fn hex_0x_prefix_rejected() {
        // "0xdeadbeef" -- the 'x' breaks the boundary and makes "deadbeef" preceded by alnum
        let tokens = extract_hex_tokens("value 0xdeadbeef here");
        assert!(tokens.is_empty());
    }
}
