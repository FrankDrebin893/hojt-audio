//! The allocator of the `no_alloc` tests sees what the audio thread allocates, frees and
//! resizes, and lets the rest through

use suite_common::host::{assert_allocates_nothing, NoAlloc};

#[global_allocator]
static ALLOC: NoAlloc = NoAlloc;

#[test]
fn test_nothing_allocated_passes() {
    let mut numbers = [0.0f32; 64];
    let sum = assert_allocates_nothing("Adding up", || {
        numbers.iter_mut().enumerate().for_each(|(i, n)| *n = i as f32);
        numbers.iter().sum::<f32>()
    });
    assert_eq!(sum, 2016.0);
}

#[test]
#[should_panic(expected = "allocated")]
fn test_an_allocation_fails() {
    assert_allocates_nothing("Making a buffer", || std::hint::black_box(vec![0u8; 16]).len());
}

#[test]
#[should_panic(expected = "freed")]
fn test_freeing_fails() {
    let buffer = std::hint::black_box(vec![0u8; 16]);
    assert_allocates_nothing("Dropping a buffer", || drop(buffer));
}

#[test]
#[should_panic(expected = "resized")]
fn test_growing_fails() {
    let mut buffer = std::hint::black_box(Vec::<u8>::with_capacity(4));
    assert_allocates_nothing("Growing a buffer", || buffer.extend_from_slice(&[0; 64]));
}

#[test]
fn test_allocating_after_a_failure_is_allowed_again() {
    let failed = std::panic::catch_unwind(|| {
        assert_allocates_nothing("Making a buffer", || std::hint::black_box(vec![0u8; 16]).len())
    });
    assert!(failed.is_err());
    assert_eq!(std::hint::black_box(vec![1u8; 16]).len(), 16);
    assert_allocates_nothing("Nothing", || {});
}
