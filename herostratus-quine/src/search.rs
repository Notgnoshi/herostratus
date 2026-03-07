use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use sha1::Digest;

use crate::commit::{CommitTemplate, encode_hex};

/// Result of a successful quine search.
pub struct SearchResult {
    /// The raw 20-byte SHA-1 hash.
    pub raw_hash: [u8; 20],
    /// The matched hex prefix string.
    pub hex_prefix: String,
}

/// Search for a commit by brute-forcing SHA-1 prefix matches.
///
/// Splits the candidate space across `num_threads` threads. Returns the first match found, or
/// `None` if the entire space is exhausted (should not happen for prefix_len <= ~10).
///
/// In quine mode (`target` is `None`), the hash must start with the candidate value itself. In
/// fortune-teller mode (`target` is `Some((value, len))`), the hash must start with the fixed
/// target prefix.
pub fn search(
    template: &CommitTemplate,
    num_threads: usize,
    target: Option<(u64, u32)>,
) -> Option<SearchResult> {
    let prefix_len = template.prefix_len;
    let total: u64 = 1u64 << (prefix_len * 4);
    let base_hasher = template.prefix_hasher();
    let suffix = &template.suffix;
    let found = AtomicBool::new(false);

    tracing::info!(
        prefix_len,
        total,
        num_threads,
        "Starting search over {total} candidates with {num_threads} threads"
    );

    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})",
            )
            .expect("valid progress bar template")
            .progress_chars("#>-"),
    );
    pb.enable_steady_tick(Duration::from_millis(100));

    let result: Option<(u64, [u8; 20])> = std::thread::scope(|s| {
        let chunk = total / num_threads as u64;
        let mut handles = Vec::with_capacity(num_threads);

        for thread_id in 0..num_threads {
            let start = chunk * thread_id as u64;
            let end = if thread_id == num_threads - 1 {
                total
            } else {
                start + chunk
            };
            let base_hasher = base_hasher.clone();
            let found = &found;
            let pb = &pb;

            handles.push(s.spawn(move || {
                search_range(
                    base_hasher,
                    suffix,
                    prefix_len,
                    start,
                    end,
                    found,
                    pb,
                    target,
                )
            }));
        }

        for handle in handles {
            if let Some(result) = handle.join().expect("worker thread panicked") {
                return Some(result);
            }
        }
        None
    });

    pb.finish_and_clear();

    result.map(|(candidate, raw_hash)| {
        let mut hex_buf = vec![0u8; prefix_len as usize];
        encode_hex(candidate, &mut hex_buf);
        SearchResult {
            raw_hash,
            hex_prefix: String::from_utf8(hex_buf).expect("hex is valid UTF-8"),
        }
    })
}

const BATCH_SIZE: u64 = 1 << 20;

/// Search a range of candidates for a SHA-1 prefix match.
///
/// Returns `Some((candidate, hash))` on match, `None` if the range is exhausted.
#[allow(clippy::too_many_arguments)]
fn search_range(
    base_hasher: sha1::Sha1,
    suffix: &[u8],
    prefix_len: u32,
    start: u64,
    end: u64,
    found: &AtomicBool,
    pb: &ProgressBar,
    target: Option<(u64, u32)>,
) -> Option<(u64, [u8; 20])> {
    let mut hex_buf = vec![0u8; prefix_len as usize];
    let mut since_last_update: u64 = 0;

    for candidate in start..end {
        since_last_update += 1;
        if since_last_update >= BATCH_SIZE {
            pb.inc(since_last_update);
            since_last_update = 0;
            if found.load(Ordering::Relaxed) {
                return None;
            }
        }

        encode_hex(candidate, &mut hex_buf);

        let mut hasher = base_hasher.clone();
        hasher.update(&hex_buf);
        hasher.update(suffix);
        let hash = hasher.finalize();

        let (match_val, match_len) = target.unwrap_or((candidate, prefix_len));
        if matches_prefix(&hash.into(), match_val, match_len) {
            found.store(true, Ordering::Relaxed);
            pb.inc(since_last_update);
            return Some((candidate, hash.into()));
        }
    }
    pb.inc(since_last_update);
    None
}

/// Check if the first N hex characters of a SHA-1 hash match the candidate value.
///
/// For even prefix_len, this compares full bytes. For odd prefix_len, the last nibble is
/// masked.
fn matches_prefix(hash: &[u8; 20], candidate: u64, prefix_len: u32) -> bool {
    let full_bytes = (prefix_len / 2) as usize;
    let odd = !prefix_len.is_multiple_of(2);

    // Compare full bytes. Byte i of the hash covers hex digits 2*i and 2*i+1.
    for (i, &hash_byte) in hash.iter().enumerate().take(full_bytes) {
        let shift = (prefix_len as usize - 2 * (i + 1)) * 4;
        let expected = ((candidate >> shift) & 0xff) as u8;
        if hash_byte != expected {
            return false;
        }
    }

    // Compare the remaining nibble for odd prefix lengths
    if odd {
        let expected = (candidate & 0xf) as u8;
        let actual = (hash[full_bytes] >> 4) & 0xf;
        if actual != expected {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_prefix_even() {
        // Hash starts with 0xab 0xcd ...
        let mut hash = [0u8; 20];
        hash[0] = 0xab;
        hash[1] = 0xcd;

        assert!(matches_prefix(&hash, 0xabcd, 4));
        assert!(!matches_prefix(&hash, 0xabce, 4));
        assert!(matches_prefix(&hash, 0xab, 2));
    }

    #[test]
    fn test_matches_prefix_odd() {
        let mut hash = [0u8; 20];
        hash[0] = 0xab;
        hash[1] = 0xc0; // upper nibble is 'c'

        assert!(matches_prefix(&hash, 0xabc, 3));
        assert!(!matches_prefix(&hash, 0xabd, 3));
    }

    #[test]
    fn test_matches_prefix_single_char() {
        let mut hash = [0u8; 20];
        hash[0] = 0xf0;
        assert!(matches_prefix(&hash, 0xf, 1));

        hash[0] = 0xe0;
        assert!(!matches_prefix(&hash, 0xf, 1));
    }

    #[test]
    fn test_search_prefix_4() {
        // A small prefix_len=4 search should complete quickly and find a match.
        let template = CommitTemplate::new(4, None, "Test User", "test@example.com", 1000000);
        let result = search(&template, 1, None);
        assert!(result.is_some(), "should find a match with prefix_len=4");

        let result = result.unwrap();
        // Verify the hex prefix matches the hash
        let hash_hex: String = result.raw_hash.iter().map(|b| format!("{b:02x}")).collect();
        assert!(
            hash_hex.starts_with(&result.hex_prefix),
            "hash {hash_hex} should start with prefix {}",
            result.hex_prefix
        );
    }

    #[test]
    fn test_search_prefix_4_multithreaded() {
        let template = CommitTemplate::new(4, None, "Test User", "test@example.com", 1000000);
        let result = search(&template, 4, None);
        assert!(
            result.is_some(),
            "should find a match with prefix_len=4 using 4 threads"
        );

        let result = result.unwrap();
        let hash_hex: String = result.raw_hash.iter().map(|b| format!("{b:02x}")).collect();
        assert!(
            hash_hex.starts_with(&result.hex_prefix),
            "hash {hash_hex} should start with prefix {}",
            result.hex_prefix
        );
    }

    #[test]
    fn test_search_with_target() {
        // Fortune-teller mode: nonce field is 4 chars, target is a specific 4-char prefix.
        // We pick a target and let the search find a nonce that produces a matching hash.
        let parent = "c".repeat(40);
        let template =
            CommitTemplate::new_fortune_teller(4, &parent, "Test User", "test@example.com", 1000);
        // Target: hash must start with "000" (3 hex chars = 12 bits, ~1 in 4096)
        let target = Some((0x000, 3));
        let result = search(&template, 1, target);
        assert!(
            result.is_some(),
            "should find a match for a 3-char target prefix"
        );

        let result = result.unwrap();
        let hash_hex: String = result.raw_hash.iter().map(|b| format!("{b:02x}")).collect();
        assert!(
            hash_hex.starts_with("000"),
            "hash {hash_hex} should start with target prefix 000"
        );
    }
}
