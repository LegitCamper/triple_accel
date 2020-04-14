//! # triple accel
//! Hamming and Levenshtein distance routines that are accelerated using SIMD.
//!
//! This library provides routines for both searching for needle string in some haystack string
//! and calculating the distance between two strings, along with other utility functions.
//!
//! The goal is an easy-to-use abstraction over SIMD edit distance routines, that
//! falls back to non-SIMD routines if the target architecture is not supported.
//! Additionally, all limitations and tradeoffs of edit distance routines should be provided upfront
//! so the user knows exactly what to expect.
//!
//! This library supports strings that are represented with `u8` characters. Unicode is not
//! currently supported.
//!
//! Currently, this library supports the AVX2 instruction set for both x86 and x86-64 machines.
//! This offers 256-bit vectors that allow 32 bytes to be processed together.
//!
//! Quick notation notes that will often appear:
//! * `k` - the number of edits that are allowed
//! * `a` and `b` - any two strings; this is usually used for matching routines
//! * `needle` and `haystack` - any two strings; we want to search for where needle appears in
//! haystack

use std;

mod jewel;
mod hamming;
mod levenshtein;

// re-export
pub use hamming::*;
pub use levenshtein::*;

#[cfg(target_arch = "x86")]
use core::arch::x86::*;

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

// some shared utility stuff below

/// A struct that describes a single matching location.
///
/// This is usually returned as part of searching routines.
#[derive(Debug, PartialEq)]
pub struct Match {
    /// The start index of the match (inclusive).
    pub start: usize,
    /// The end index of the match (exclusive).
    pub end: usize,
    /// Number of edits for the match.
    pub k: u32
}

/// An enum describing possible edit operations.
///
/// This is usually returned as part of the traceback for matching routines.
#[derive(Debug, PartialEq)]
pub enum Edit {
    Match,
    Mismatch,
    AGap,
    BGap
}

/// This creates a vector with the alignment and padding for `u128` values, and then convert it to a vector of `u8` values that is returned.
///
/// This is possible because u8 has looser alignment requirements than `u128`.
/// This vector can be easily converted back to `u128` or `u64` later, for Hamming distance routines.
/// The returned vector can be edited by copying `u8` values into it.
/// However, do not do any operation (like `push`) that may cause the the vector to be reallocated.
///
/// # Arguments
/// * `len` - the length of the resulting array of u8 values
///
/// # Example
/// ```
/// # use triple_accel::*;
///
/// let s = alloc_str(10);
///
/// assert!(s.len() == 10);
/// ```
#[inline]
pub fn alloc_str(len: usize) -> Vec<u8> {
    let words_len = (len >> 4) + (if (len & 15) > 0 {1} else {0});
    let words = vec![0u128; words_len];
    let mut words = std::mem::ManuallyDrop::new(words);

    unsafe {
        Vec::from_raw_parts(words.as_mut_ptr() as *mut u8, len, words_len << 4)
    }
}

/// Directly copy from the a source `u8` slice to a destination `u8` slice.
///
/// Can be used to copy string data after allocating a vector using `alloc_str`.
///
/// # Arguments
/// * `dest` - the destination slice
/// * `src` - the source slice
///
/// # Panics
/// * If the length of `src` is greater than the length of `dest`.
///
/// # Example
/// ```
/// # use triple_accel::*;
///
/// let mut a = vec![0u8; 5];
/// let b = vec![1u8, 2u8, 3u8, 4u8];
///
/// fill_str(&mut a, &b);
///
/// assert!(a == vec![1u8, 2u8, 3u8, 4u8, 0u8]);
/// ```
#[inline]
pub fn fill_str(dest: &mut [u8], src: &[u8]) {
    assert!(dest.len() >= src.len());

    unsafe {
        std::ptr::copy_nonoverlapping(src.as_ptr(), dest.as_mut_ptr(), src.len());
    }
}

#[allow(dead_code)]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn print_x86_avx2(a: &str, b: __m256i){ // debug print AVX2 vectors
    let mut arr = [0u8; 32];
    _mm256_storeu_si256(arr.as_mut_ptr() as *mut __m256i, b);
    print!("{}\t", a);

    for i in 0..32 {
        print!(" {:>3}", arr[i]);
    }

    println!();
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn shift_left_x86_avx2(a: __m256i) -> __m256i {
    _mm256_alignr_epi8(_mm256_permute2x128_si256(a, a, 0b10000001i32), a, 1i32)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn shift_right_x86_avx2(a: __m256i) -> __m256i {
    _mm256_alignr_epi8(a, _mm256_permute2x128_si256(a, a, 0b00001000i32), 15i32)
}

