use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use generic_array::GenericArray;
use sha1::{Digest, Sha1};

pub fn spawn_worker_thread(
    w: usize,
    worker_start: u128,
    worker_end: u128,
    prefix_length: u8,
    raw_commit: String,
    is_running: Arc<AtomicBool>,
) -> std::thread::JoinHandle<Option<u128>> {
    std::thread::Builder::new()
        .name(format!("quine-{w}"))
        .spawn(move || {
            worker(
                w,
                worker_start,
                worker_end,
                prefix_length,
                raw_commit,
                is_running,
            )
        })
        .expect("Failed to spawn worker thread")
}

pub fn join_all<T>(
    mut handles: Vec<std::thread::JoinHandle<Option<T>>>,
    is_running: Arc<AtomicBool>,
) -> Vec<T> {
    let mut results = Vec::new();

    while !handles.is_empty() {
        let (finished, unfinished): (Vec<_>, Vec<_>) =
            handles.into_iter().partition(|h| h.is_finished());

        for handle in finished {
            // terminate after the first result
            if let Some(result) = handle.join().expect("Worker thread panicked") {
                results.push(result);
                is_running.store(false, Ordering::SeqCst);
            }
        }

        handles = unfinished;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    results
}

fn worker(
    worker: usize,
    worker_start: u128,
    worker_end: u128,
    prefix_length: u8,
    mut raw_commit: String,
    is_running: Arc<AtomicBool>,
) -> Option<u128> {
    tracing::debug!("Worker {worker} processing chunk {worker_start:#x}..={worker_end:#x}");

    let placeholder = "X".repeat(prefix_length as usize);
    let offset = raw_commit
        .find(&placeholder)
        .expect("Failed to find XXXXX placeholder pattern");

    let mut hasher = Sha1::new();
    let mut output_buffer: [u8; 20] = [0; 20];
    let output = GenericArray::from_mut_slice(&mut output_buffer);
    let prefix_length_bytes: usize = prefix_length as usize / 2; // prefix_length is in nibbles
    let hex = "0123456789abcdef";
    for prefix in worker_start..=worker_end {
        if !is_running.load(Ordering::SeqCst) {
            break;
        }

        // TODO: Give 10% progress reports

        let prefix_bytes: [u8; 16] = prefix.to_le_bytes();
        // hash printed as c63cf7 corresponds to byte array [0xc6, 0x3c, 0xf7]
        for hash_idx in 0..prefix_length as usize {
            let byte_idx = hash_idx / 2;
            let prefix_nibble = if hash_idx % 2 == 0 {
                // high nibble of prefix
                prefix_bytes[byte_idx] >> 4
            } else {
                // low nibble of prefix
                prefix_bytes[byte_idx] & 0x0F
            };
            let prefix_char = hex.as_bytes()[prefix_nibble as usize];

            unsafe {
                raw_commit.as_bytes_mut()[hash_idx + offset] = prefix_char;
            }
        }
        // TODO: Do I need to add a UUID to each commit for more randomness? Or do I fall back on
        // the whitespace trick from https://github.com/not-an-aardvark/lucky-commit ?
        hasher.update(raw_commit.as_bytes());

        // Finalize and reset the hasher, using preallocated output memory
        hasher.finalize_into_reset(output);

        // let oid = git2::Oid::from_bytes(output).unwrap();
        // tracing::debug!("Worker {worker} attempting prefix {prefix:#x} found full hash {oid} from {raw_commit:?} {prefix_bytes:x?}");

        // TODO: This doesn't account for nibble order. E.g., prefix 0x95A matches hash 5A090B, but
        // it works as long as the prefix_length is even?
        if output.as_slice()[..=prefix_length_bytes] == prefix_bytes[..=prefix_length_bytes] {
            // hack for pretty-printing
            let oid = git2::Oid::from_bytes(output).unwrap();
            tracing::info!("Worker {worker} found prefix {prefix:#x} for full hash {oid}");
            return Some(prefix);
        }
    }

    None
}
