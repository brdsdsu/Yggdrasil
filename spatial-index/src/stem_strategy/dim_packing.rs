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
//! # Why the decode is range-clamped
//!
//! Slots in the stem array that no split has written hold `A::max_value()`, and its low bits
//! depend on the axis family: for floats that value is **infinity**, whose mantissa is all
//! zeros, while for integer and fixed-point axes it is `MAX`, whose low bits are all ones. So
//! no single encoding makes every filler decode in range — plain encoding is safe against the
//! float filler and unsafe against the integer one, and the reverse holds for a complemented
//! encoding.
//!
//! Since `K` need not be a power of two, an out-of-range decode is not harmless: `K = 3` would
//! yield dimension 3, indexed into a `[A; 3]` with `get_unchecked`, reading past the end of the
//! array. [`unpack_from`] therefore clamps into range, which costs one comparison and holds for
//! any axis type and any filler. The filler itself is deliberately left alone: other strategies
//! detect unwritten slots with `Axis::is_max_value`, which needs its exact bit pattern.
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
    unsafe { *lsb = (*lsb & !mask) | (dim as u8 & mask) };
}

/// Recover the dimension folded into the `width`-byte value at `ptr` by [`pack_into`].
///
/// Clamped into `0..k`, so a slot no split has written — whose low bits are whatever
/// `A::max_value()` happens to carry — can never produce an index outside a `[A; k]`. See the
/// module docs.
///
/// # Safety
///
/// `ptr` must be valid for reads of `width` bytes.
#[inline(always)]
pub unsafe fn unpack_from(ptr: *const u8, width: usize, mask: u8, k: usize) -> usize {
    debug_assert!(width > 0 && k > 0);
    let lsb = unsafe { *ptr.add(lsb_offset(width)) };
    ((lsb & mask) as usize).min(k - 1)
}

/// Find a value carrying `dim` in its low bits that separates `lower` from `upper`, i.e. one
/// satisfying `lower < separator <= upper`.
///
/// A leaf is split by sorting on the chosen axis and cutting between two adjacent distinct
/// values; `lower` is the largest value that must stay left and `upper` the smallest that must
/// go right. The stored split value has to be the one the partition is performed against, so it
/// must be grid-aligned *and* still separate that pair — hence choosing it here rather than
/// rounding a pivot afterwards, which would move points across the plane relative to where they
/// were placed.
///
/// Returns `None` when the pair is too close together for a grid point to fall between them,
/// i.e. when they differ by less than one grid cell. On coordinates quantised to any real-world
/// lattice this does not arise: 4 ULP at a coordinate of 3.85 is 8.6e-7.
///
/// Candidates are checked by **value comparison** rather than derived by arithmetic on bit
/// patterns. IEEE 754 orders negative floats in reverse of their bit patterns, so "round the bits
/// down" moves the value down for positive coordinates and up for negative ones; verifying each
/// candidate sidesteps that asymmetry entirely instead of encoding it in sign-dependent branches.
#[inline]
pub fn grid_separator<A: PartialOrd + Copy>(
    lower: A,
    upper: A,
    width: usize,
    dim: usize,
    mask: u8,
) -> Option<A> {
    debug_assert!(lower < upper, "caller must bracket two distinct values");

    if mask == 0 {
        // K == 1: nothing is packed, so the upper value itself is already "aligned".
        return Some(upper);
    }

    let cell = mask as i32 + 1;
    let mut candidate = upper;
    unsafe { pack_into((&mut candidate as *mut A).cast(), width, dim, mask) };

    // Aligning `upper` moves it by less than one cell, in a direction that depends on its sign.
    // Stepping one cell either way therefore covers every grid point that could lie in the
    // interval, and the bounds check is what decides which is valid.
    for step in [0i32, -cell, cell] {
        let mut probe = candidate;
        if step != 0 {
            unsafe { offset_bits((&mut probe as *mut A).cast(), width, step) };
            // Stepping must not disturb the packed bits.
            debug_assert_eq!(
                unsafe { unpack_from((&probe as *const A).cast(), width, mask, dim + 1) },
                dim
            );
        }
        if probe > lower && probe <= upper {
            return Some(probe);
        }
    }

    None
}

/// Add `delta` to the raw integer representation of the `width`-byte value at `ptr`.
///
/// Operates on the whole word rather than the least significant byte so that a step can carry
/// across byte boundaries. `delta` is a multiple of the grid cell, so the packed low bits are
/// preserved.
///
/// # Safety
///
/// `ptr` must be valid for reads and writes of `width` bytes, and `width` must be 2, 4 or 8.
#[inline]
unsafe fn offset_bits(ptr: *mut u8, width: usize, delta: i32) {
    unsafe {
        match width {
            2 => {
                let p = ptr.cast::<u16>();
                *p = (*p).wrapping_add(delta as u16);
            }
            4 => {
                let p = ptr.cast::<u32>();
                *p = (*p).wrapping_add(delta as u32);
            }
            8 => {
                let p = ptr.cast::<u64>();
                *p = (*p).wrapping_add(delta as i64 as u64);
            }
            _ => debug_assert!(false, "unsupported axis width {width}"),
        }
    }
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
                    let got = unpack_from((&v as *const f32).cast(), 4, mask, K);
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
                    assert_eq!(unpack_from((&v as *const f64).cast(), 8, mask, K), dim);
                }
            }
        }
    }

    /// The property the clamp exists for: whatever an unwritten slot holds must decode inside
    /// `0..K`, because descent indexes a `[A; K]` with `get_unchecked`.
    #[test]
    fn unwritten_slots_decode_in_range() {
        for k in 1..=16usize {
            let mask = dim_mask(k);
            // Both fillers actually in use, plus the extremes of the low-bit space. `max_value()`
            // is infinity for float axes (mantissa all zeros) and MAX for integer and
            // fixed-point axes (low bits all ones), so no single encoding puts every filler in
            // range on its own - the clamp is what does.
            for bits in [
                f32::INFINITY.to_bits(),
                f32::NEG_INFINITY.to_bits(),
                f32::MAX.to_bits(),
                u32::MAX,
                0x7FFF_FFFF,
                0,
            ] {
                let v = f32::from_bits(bits);
                let got = unsafe { unpack_from((&v as *const f32).cast(), 4, mask, k) };
                assert!(got < k, "K={k}: bits {bits:#010X} decoded dimension {got}");
            }
        }
    }

    /// The property the split depends on: whatever `grid_separator` returns must both carry the
    /// requested dimension and genuinely separate the bracketing pair. Swept across signs and
    /// magnitudes, including pairs that straddle zero, where bit-pattern order reverses.
    #[test]
    fn grid_separator_separates_and_carries_the_dim() {
        const K: usize = 3;
        let mask = dim_mask(K);
        let pairs = [
            (0.0f32, 0.05),
            (3.80, 3.85),
            (-1.65, -1.60),
            (-0.05, 0.05),
            (-1e-30, 1e-30),
            (-3.85, -3.80),
            (1e20, 1.1e20),
            (f32::MIN, f32::MAX),
            (-0.0, 0.05),
        ];
        for (lower, upper) in pairs {
            for dim in 0..K {
                let s = grid_separator(lower, upper, 4, dim, mask)
                    .unwrap_or_else(|| panic!("no separator for ({lower}, {upper}) dim {dim}"));
                assert!(
                    s > lower && s <= upper,
                    "({lower}, {upper}) dim {dim}: separator {s} outside the interval"
                );
                let got = unsafe { unpack_from((&s as *const f32).cast(), 4, mask, K) };
                assert_eq!(got, dim, "({lower}, {upper}): separator {s} carries dim {got}");
            }
        }
    }

    /// A pair closer than one grid cell has no aligned value between them for at least some
    /// dimensions. The split must decline rather than return something that fails to separate.
    #[test]
    fn grid_separator_declines_when_the_gap_is_too_small() {
        const K: usize = 3;
        let mask = dim_mask(K);
        let lower = 1.0f32;
        let upper = f32::from_bits(lower.to_bits() + 1); // one ULP apart, quarter of a cell
        let outcomes: Vec<_> = (0..K)
            .map(|d| grid_separator(lower, upper, 4, d, mask))
            .collect();
        assert!(
            outcomes.iter().any(|o| o.is_none()),
            "expected at least one dimension to have no aligned separator, got {outcomes:?}"
        );
        // Whatever it does return must still be correct.
        for s in outcomes.into_iter().flatten() {
            assert!(s > lower && s <= upper);
        }
    }

    /// Exhaustive over a dense range of bit patterns: for every bracketing pair at least one grid
    /// cell wide, a separator must exist for every dimension.
    #[test]
    fn grid_separator_always_succeeds_beyond_one_cell() {
        const K: usize = 3;
        let mask = dim_mask(K);
        let base = 3.85f32.to_bits();
        for gap in (mask as u32 + 1)..64 {
            for offset in 0..8u32 {
                let lower = f32::from_bits(base + offset);
                let upper = f32::from_bits(base + offset + gap);
                for dim in 0..K {
                    let s = grid_separator(lower, upper, 4, dim, mask).unwrap_or_else(|| {
                        panic!("gap {gap} offset {offset} dim {dim}: no separator")
                    });
                    assert!(s > lower && s <= upper, "gap {gap} dim {dim}: {s} out of range");
                    assert_eq!(unsafe { unpack_from((&s as *const f32).cast(), 4, mask, K) }, dim);
                }
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
            assert_eq!(unpack_from((&v as *const f32).cast(), 4, mask, 1), 0);
        }
    }
}
