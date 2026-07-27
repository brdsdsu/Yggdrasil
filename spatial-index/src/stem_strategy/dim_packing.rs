//! FORK DEVIATION (see README.md): packing a stem's split dimension into the low bits of its
//! split value.
//!
//! Upstream re-derives the split dimension from tree depth (`dim = (dim + 1) % K`), so a full
//! leaf whose points all share one value on the cycled axis cannot be split at all. Choosing
//! the axis by widest spread fixes that, but then the choice has to be *recorded*, or queries
//! descend comparing the wrong coordinate and silently return wrong neighbours. A stem in v6
//! is a bare `A` with nowhere to record anything, so the dimension rides in the low bits of
//! the split value itself.
//!
//! # Why this needs no knowledge of the axis type
//!
//! Every axis type this crate supports keeps its fraction in the low-order bits — the mantissa
//! of `f16`/`f32`/`f64`, the fractional part of a `fixed` type, the low digits of an integer.
//! So "clear the low `n` bits and fold in the dimension" is well defined given only the value's
//! *width in bytes*, which [`Axis::VALUE_WIDTH_BYTES`](crate::Axis) already provides. Since
//! `dim_bits` never exceeds 8 for `K <= 256`, the affected bits always live inside the single
//! least-significant byte, so this is a one-byte read/modify/write at a computable offset.
//!
//! # Why the dimension is stored complemented
//!
//! Slots in the stem array that no split has written hold `A::max_value()`, and for every
//! supported axis type that value has **all** its low bits set (`f32::MAX` is `0x7F7FFFFF`,
//! `i32::MAX` is `0x7FFFFFFF`). Stored plainly, those bits would decode to `dim_mask`, which
//! for a non-power-of-two `K` is out of range — `K = 3` would yield dimension 3, indexed into
//! a `[A; 3]` with `get_unchecked`, reading past the end of the array.
//!
//! Storing the complement makes an all-ones pattern decode to dimension **0** for every `K`,
//! so an unwritten slot is harmless by construction rather than by a range check in the hot
//! path. It also means `A::max_value()` keeps its exact bit pattern, which matters because
//! other strategies detect unwritten slots with `Axis::is_max_value`.
//!
//! # Cost
//!
//! The split plane is constrained to a grid of `2^dim_bits(K)` ULP. For `K = 3` that is 4 ULP:
//! at a coordinate of 3.85 m, a shift of 8.6e-7 m, against voxel geometry quantised to 0.05 m.
//! Nothing requires a split plane to coincide with a data point, so the only real constraint
//! is that the *stored* value is the one partitioned against — see `pack_into`.

/// Low bits of a split value reserved to carry its split dimension, for a `K`-dimensional tree.
///
/// `K = 1` yields zero bits: a one-dimensional tree has only one axis to split on, so nothing
/// needs recording and the value is stored exactly.
#[inline(always)]
pub const fn dim_bits(k: usize) -> u32 {
    k.next_power_of_two().ilog2()
}

/// Mask selecting the bits [`dim_bits`] reserves. Zero when `K == 1`.
#[inline(always)]
pub const fn dim_mask(k: usize) -> u8 {
    // `dim_bits` is 8 for K = 256 and this crate's K is far smaller, but shift by 8 on a u8
    // would overflow, so widen before shifting and truncate after.
    (((1u16 << dim_bits(k)) - 1) & 0xFF) as u8
}

/// Byte offset of the least significant byte of a `width`-byte value.
#[inline(always)]
const fn lsb_offset(width: usize) -> usize {
    if cfg!(target_endian = "little") {
        0
    } else {
        width - 1
    }
}

/// Fold `dim` into the low [`dim_bits`] bits of the `width`-byte value at `ptr`.
///
/// # Safety
///
/// `ptr` must be valid for reads and writes of `width` bytes.
#[inline(always)]
pub unsafe fn pack_into(ptr: *mut u8, width: usize, dim: usize, mask: u8) {
    debug_assert!(width > 0);
    let lsb = unsafe { ptr.add(lsb_offset(width)) };
    // Complemented: see the module docs on unwritten slots.
    let encoded = !(dim as u8) & mask;
    unsafe { *lsb = (*lsb & !mask) | encoded };
}

/// Recover the dimension folded into the `width`-byte value at `ptr` by [`pack_into`].
///
/// Returns 0 for any slot never written by a split, since `A::max_value()` has all low bits set.
///
/// # Safety
///
/// `ptr` must be valid for reads of `width` bytes.
#[inline(always)]
pub unsafe fn unpack_from(ptr: *const u8, width: usize, mask: u8) -> usize {
    debug_assert!(width > 0);
    let lsb = unsafe { *ptr.add(lsb_offset(width)) };
    (!lsb & mask) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dim_bits_scales_with_k() {
        // K = 1 reserves nothing: one axis needs no recording.
        assert_eq!((dim_bits(1), dim_mask(1)), (0, 0));
        assert_eq!((dim_bits(2), dim_mask(2)), (1, 1));
        assert_eq!((dim_bits(3), dim_mask(3)), (2, 3));
        assert_eq!((dim_bits(4), dim_mask(4)), (2, 3));
        assert_eq!((dim_bits(5), dim_mask(5)), (3, 7));
        assert_eq!((dim_bits(8), dim_mask(8)), (3, 7));
        assert_eq!((dim_bits(9), dim_mask(9)), (4, 15));
    }

    /// Round-trip through the packed representation, for both signs and across zero, since
    /// IEEE 754 bit patterns are not monotonic in value across the sign boundary.
    #[test]
    fn f32_round_trips_every_dimension() {
        const K: usize = 3;
        let mask = dim_mask(K);
        for value in [3.85f32, 0.05, -1.65, 0.0, -0.0, 1e-38, f32::MAX, f32::MIN] {
            for dim in 0..K {
                let mut v = value;
                unsafe {
                    pack_into((&mut v as *mut f32).cast(), 4, dim, mask);
                    let got = unpack_from((&v as *const f32).cast(), 4, mask);
                    assert_eq!(got, dim, "value {value} dim {dim} decoded as {got}");
                }
                // The stored value stays within one grid cell of the original.
                let shift = (v.to_bits() as i64 - value.to_bits() as i64).abs();
                assert!(shift <= mask as i64, "value {value} moved {shift} ULP");
            }
        }
    }

    #[test]
    fn f64_round_trips_every_dimension() {
        const K: usize = 3;
        let mask = dim_mask(K);
        for value in [3.85f64, -1.65, 0.0, f64::MAX] {
            for dim in 0..K {
                let mut v = value;
                unsafe {
                    pack_into((&mut v as *mut f64).cast(), 8, dim, mask);
                    assert_eq!(unpack_from((&v as *const f64).cast(), 8, mask), dim);
                }
            }
        }
    }

    /// The property the complement exists for: a slot holding `max_value()` must decode to a
    /// dimension that is in range for every `K`, because descent indexes a `[A; K]` with
    /// `get_unchecked`.
    #[test]
    fn unwritten_slots_decode_to_zero() {
        for k in 1..=16usize {
            let mask = dim_mask(k);
            for max in [f32::MAX.to_bits(), u32::MAX, 0x7FFF_FFFF] {
                let v = f32::from_bits(max);
                let got = unsafe { unpack_from((&v as *const f32).cast(), 4, mask) };
                assert_eq!(got, 0, "K={k}: all-ones low bits must decode to dim 0");
                assert!(got < k);
            }
        }
    }

    /// `K = 1` must leave the value bit-for-bit unchanged.
    #[test]
    fn single_dimension_stores_exactly() {
        let mask = dim_mask(1);
        let mut v = 3.85f32;
        let before = v.to_bits();
        unsafe {
            pack_into((&mut v as *mut f32).cast(), 4, 0, mask);
            assert_eq!(v.to_bits(), before);
            assert_eq!(unpack_from((&v as *const f32).cast(), 4, mask), 0);
        }
    }
}
