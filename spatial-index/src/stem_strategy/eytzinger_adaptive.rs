//! FORK DEVIATION (see ADAPTIVE_SPLIT.md): an Eytzinger cursor whose split dimension comes from
//! the stem rather than from the tree depth.
//!
//! [`EytzingerAdaptive`] addresses stems exactly as [`crate::Eytzinger`] does — breadth-first,
//! root at index 1, children of `i` at `2i` and `2i+1`. The only difference is where the split
//! dimension comes from. Upstream advances a counter on the way down:
//!
//! ```text
//! self.dim = (self.dim + 1) % K;      // in traverse()
//! ```
//!
//! which means a full leaf whose points all share one value on the cycled axis has no pivot to
//! split at, and the insert is refused. Choosing the axis by spread instead requires recording
//! the choice, so this cursor reads it back out of the low bits of the split value — see
//! [`crate::stem_strategy::dim_packing`] for the encoding.
//!
//! # Bulk construction
//!
//! Supported. It used to be refused with
//! [`ConstructionError::AdaptiveSplitBulkBuildUnsupported`](crate::kd_tree::ConstructionError),
//! because the bulk builders partitioned around a pivot before writing it and so could not
//! record a dimension in that value without shifting the plane off its own partition.
//! `KdTree::choose_split` now picks a grid-aligned separator *before* the value is stored, so the
//! stored plane and the partition agree. The error variant is retained for compatibility but is
//! no longer produced.
//!
//! Bulk construction also selects the axis by widest spread rather than cycling with depth, which
//! is what this strategy exists to permit. On the benchmark corpus that is worth 8x fewer stem
//! slots than the cycling builder (163,842 against 1,310,722 for 354,885 lattice points), and
//! ~1000x fewer than incremental insertion in mesh order.
//!
//! # Why the dimension is decoded on demand rather than cached
//!
//! [`StemStrategy::dim`] is not generic over the axis type, so it cannot interpret the bytes at
//! `stems_ptr` on its own; it needs the value's width. Caching the decode in `traverse` (which
//! *is* generic over the axis) would avoid that, but `traverse` runs once more than the descent
//! loop reads — the final step computes a child index the loop never visits — so it would decode
//! from a slot that may lie past the end of the array. Decoding in `dim` instead, guarded by the
//! stem count, is both simpler and reads a word the caller has just loaded anyway.

use std::ptr::NonNull;

use super::dim_packing::{dim_mask, pack_into, unpack_from};
use crate::{Axis, StemStrategy};

/// Eytzinger stem cursor that reads each stem's split dimension from the stem itself.
///
/// Prefetch policy is fixed to the default Eytzinger configuration; the adaptive behaviour is
/// orthogonal to prefetch tuning, and leaving it fixed keeps this type free of const parameters
/// that would appear in every dependent's type alias.
#[derive(Clone, Debug)]
pub struct EytzingerAdaptive {
    stem_idx: u32,
    level: i32,
    /// Byte width of one stem, and the number of stems. Both are supplied by
    /// [`StemStrategy::bind_stems`], because neither [`StemStrategy::new`] nor
    /// [`StemStrategy::dim`] is generic over the axis type. Zero width means unbound, in which
    /// case [`StemStrategy::dim`] answers 0 rather than reading anything.
    stem_width: u8,
    stem_count: u32,
    stems_ptr: NonNull<u8>,
}

/// Compact deferred traversal state for [`EytzingerAdaptive`].
///
/// Smaller than the cycling equivalent: the dimension is a property of the stem, so it does not
/// have to be carried across a backtrack.
#[doc(hidden)]
pub struct EytzingerAdaptiveDeferred {
    stem_idx: u32,
    level: u16,
}

unsafe impl Send for EytzingerAdaptive {}
unsafe impl Sync for EytzingerAdaptive {}

impl EytzingerAdaptive {
    /// Fold `dim` into `value` for storage at a stem. Mirrors [`Self::dim`]'s decode.
    #[inline(always)]
    pub fn encode<A: Axis, const K: usize>(mut value: A, dim: usize) -> A {
        debug_assert!(dim < K, "split dimension {dim} out of range for K={K}");
        // `size_of::<A>()` rather than `A::VALUE_WIDTH_BYTES`: this is the width of the value
        // being mutated in place, which is what locates the least significant byte.
        unsafe {
            pack_into(
                (&mut value as *mut A).cast(),
                size_of::<A>(),
                dim,
                dim_mask(K),
            )
        };
        value
    }
}

impl StemStrategy for EytzingerAdaptive {
    const ROOT_IDX: usize = 1;

    /// The point of this strategy: `dim` recovers the axis from the stem, so construction is free
    /// to choose it from the data.
    const ADAPTIVE_SPLIT_DIM: bool = true;

    type DeferredState = EytzingerAdaptiveDeferred;
    type StackContext<A> = crate::kd_tree::query_stack::QueryStackContext<A, Self::DeferredState>;
    type Stack<A> = crate::kd_tree::query_stack::QueryStack<A, Self>;

    fn new(stems_ptr: NonNull<u8>) -> Self {
        Self {
            stem_idx: Self::ROOT_IDX as u32,
            level: 0,
            stem_width: 0,
            stem_count: 0,
            stems_ptr,
        }
    }

    #[inline(always)]
    fn bind_stems<A: Axis>(&mut self, stems: &[A]) {
        self.stem_width = A::VALUE_WIDTH_BYTES as u8;
        self.stem_count = stems.len() as u32;
    }

    #[inline(always)]
    fn stem_idx(&self) -> usize {
        self.stem_idx as usize
    }

    fn deferred_state(&self) -> Self::DeferredState {
        EytzingerAdaptiveDeferred {
            stem_idx: self.stem_idx,
            level: self.level as u16,
        }
    }

    fn rehydrate_deferred_state(&mut self, state: Self::DeferredState) {
        self.stem_idx = state.stem_idx;
        self.level = state.level as i32;
    }

    #[inline(always)]
    fn leaf_idx(&self) -> usize {
        let mask = 1u32.wrapping_shl(self.level as u32);
        (self.stem_idx & !mask) as usize
    }

    /// The split dimension recorded at the current stem.
    ///
    /// Answers 0 for a stem that does not exist, or before [`StemStrategy::bind_stems`] — both
    /// are in range for any `K`, so a caller indexing a `[A; K]` with the result stays inside
    /// the array even on a path that never reads the stem itself.
    #[inline(always)]
    fn dim<const K: usize>(&self) -> usize {
        if self.stem_width == 0 || self.stem_idx >= self.stem_count {
            return 0;
        }
        let width = self.stem_width as usize;
        let dim = unsafe {
            unpack_from(
                self.stems_ptr
                    .as_ptr()
                    .add(self.stem_idx as usize * width)
                    .cast_const(),
                width,
                dim_mask(K),
                K,
            )
        };
        dim
    }

    /// Construction picks the dimension rather than reading one, so this stays the cycling
    /// convention — pure arithmetic on `level`, which is safe on the dangling pointer that
    /// [`StemStrategy::new_no_ptr`] hands the bulk builder.
    #[inline(always)]
    fn construction_dim<const K: usize>(&self) -> usize {
        (self.level as usize) % K
    }

    #[inline(always)]
    fn encode_split<A: Axis, const K: usize>(value: A, dim: usize) -> A {
        Self::encode::<A, K>(value, dim)
    }

    #[inline(always)]
    fn level(&self) -> i32 {
        self.level
    }

    #[inline]
    fn traverse<A: Axis, const K: usize>(&mut self, is_right_child: bool) {
        self.stem_idx = crate::EytzingerFlexPf::<0, 1>::step_pure::<A>(
            self.stem_idx,
            is_right_child,
            self.stems_ptr,
        );
        self.level = self.level.wrapping_add(1);
        // No dimension bookkeeping: compare with upstream's `(dim + 1) % K`.
    }

    fn branch<A: Axis<Coord = A>, const K: usize>(&mut self) -> Self {
        self.stem_idx = self.stem_idx.wrapping_shl(1);
        let right = self.stem_idx | 1;
        self.level = self.level.wrapping_add(1);

        Self {
            stem_idx: right,
            ..self.clone()
        }
    }

    fn child_indices<A: Axis<Coord = A>>(&self) -> (usize, usize) {
        let left = (self.stem_idx << 1) as usize;
        (left, left | 1)
    }

    fn get_stem_node_count_from_leaf_node_count(leaf_node_count: usize) -> usize {
        if leaf_node_count < 2 {
            0
        } else {
            leaf_node_count.next_power_of_two()
        }
    }

    fn stem_node_padding_factor() -> usize {
        1
    }
}
