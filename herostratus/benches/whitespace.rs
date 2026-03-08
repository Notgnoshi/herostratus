use std::hint::black_box;

use gungraun::{library_benchmark, library_benchmark_group, main};
use herostratus::utils::{is_equal_ignoring_whitespace, is_equal_ignoring_whitespace_v2};

fn make_unicode_content(size: usize) -> Vec<u8> {
    // Mix of ASCII and multi-byte unicode: accented Latin, CJK, Cyrillic
    let line = "The quick bröwn føx jumps 世界 мир over the lazy dog\n";
    line.as_bytes().iter().copied().cycle().take(size).collect()
}

fn make_whitespace_modified(content: &[u8]) -> Vec<u8> {
    // Replace single spaces with double spaces
    let mut result = Vec::with_capacity(content.len() * 2);
    for &b in content {
        if b == b' ' {
            result.push(b' ');
            result.push(b' ');
        } else {
            result.push(b);
        }
    }
    result
}

fn equal_small() -> (Vec<u8>, Vec<u8>) {
    let content = make_unicode_content(256);
    let modified = make_whitespace_modified(&content);
    (content, modified)
}

fn equal_medium() -> (Vec<u8>, Vec<u8>) {
    let content = make_unicode_content(4096);
    let modified = make_whitespace_modified(&content);
    (content, modified)
}

fn equal_large() -> (Vec<u8>, Vec<u8>) {
    let content = make_unicode_content(65536);
    let modified = make_whitespace_modified(&content);
    (content, modified)
}

fn not_equal_early() -> (Vec<u8>, Vec<u8>) {
    let content = make_unicode_content(65536);
    let mut modified = make_unicode_content(65536);
    modified[42] = b'X';
    (content, modified)
}

fn not_equal_late() -> (Vec<u8>, Vec<u8>) {
    let content = make_unicode_content(65536);
    let mut modified = make_unicode_content(65536);
    let len = modified.len();
    modified[len - 2] = b'X';
    (content, modified)
}

fn identical() -> (Vec<u8>, Vec<u8>) {
    let content = make_unicode_content(65536);
    (content.clone(), content)
}

#[library_benchmark]
#[bench::equal_small(equal_small())]
#[bench::equal_medium(equal_medium())]
#[bench::equal_large(equal_large())]
#[bench::not_equal_early(not_equal_early())]
#[bench::not_equal_late(not_equal_late())]
#[bench::identical(identical())]
fn bench_is_equal_ignoring_whitespace(pair: (Vec<u8>, Vec<u8>)) -> bool {
    black_box(is_equal_ignoring_whitespace(&pair.0[..], &pair.1[..]))
}

#[library_benchmark]
#[bench::equal_small(equal_small())]
#[bench::equal_medium(equal_medium())]
#[bench::equal_large(equal_large())]
#[bench::not_equal_early(not_equal_early())]
#[bench::not_equal_late(not_equal_late())]
#[bench::identical(identical())]
fn bench_is_equal_ignoring_whitespace_v2(pair: (Vec<u8>, Vec<u8>)) -> bool {
    black_box(is_equal_ignoring_whitespace_v2(&pair.0[..], &pair.1[..]))
}

library_benchmark_group!(
    name = whitespace_skipper;
    compare_by_id = true;
    benchmarks = bench_is_equal_ignoring_whitespace, bench_is_equal_ignoring_whitespace_v2
);

main!(library_benchmark_groups = whitespace_skipper);
