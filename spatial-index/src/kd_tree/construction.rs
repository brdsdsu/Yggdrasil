use std::ptr::NonNull;

use aligned_vec::{avec, AVec, ConstAlign, CACHELINE_ALIGN};
use nonmax::NonMaxUsize;

use crate::kd_tree::{ConstructionError, KdTreeQueryOps, MutationError, OwnedStemLeafResolution};
use crate::traits::leaf_strategy::{
    BucketLimitType, ConstructibleLeafStrategy, LeafStrategy, Mutability, MutableLeafStrategy,
};
use crate::{Axis, Content, KdTree, StemStrategy};

impl<A, T, SS, LS, const K: usize, const B: usize> KdTree<A, T, SS, LS, K, B>
where
    A: Axis<Coord = A>,
    T: Content,
    SS: StemStrategy,
    LS: MutableLeafStrategy<A, T, SS, K, B>,
{
    /// Adds a point and associated item to the tree.
    ///
    /// If the target leaf is full, it will be split before insertion.
    pub fn add(&mut self, point: &[A; K], item: T) -> Result<(), ConstructionError> {
        // Find the target leaf
        let (stem_strat, parent_stem_idx, is_right_child) = self.find_leaf_with_context(point);
        let leaf_idx = match &self.stem_leaf_resolution {
            OwnedStemLeafResolution::Mapped { leaf_idx_map, .. } => {
                leaf_idx_map[stem_strat.stem_idx()].unwrap().get()
            }
            _ => stem_strat.leaf_idx(),
        };

        if !self.leaves.is_leaf_full(leaf_idx) {
            self.leaves.add_to_leaf(leaf_idx, point, item);
            self.size += 1;
            return Ok(());
        }

        // println!("Leaf {leaf_idx} is full, splitting. {self}");

        // Leaf is full, need to split
        let (pivot_val, split_dim, new_leaf_idx) =
            self.split_leaf(leaf_idx, stem_strat, parent_stem_idx, is_right_child)?;

        // determine which leaf we belong in after the split
        let leaf_idx = if point[split_dim] >= pivot_val {
            new_leaf_idx
        } else {
            leaf_idx
        };

        self.leaves.add_to_leaf(leaf_idx, point, item);
        self.size += 1;
        Ok(())
    }

    /// Find the leaf for a query point, along with context needed for splitting.
    /// Returns: (stem_strategy, parent_stem_idx, is_right_child)
    fn find_leaf_with_context(&self, query: &[A; K]) -> (SS, Option<NonMaxUsize>, bool) {
        let stems_ptr = NonNull::new(self.stems.as_ptr() as *mut u8).unwrap();
        let mut stem_strat: SS = SS::new(stems_ptr);
        stem_strat.bind_stems::<A>(self.stems.as_slice());
        let mut parent_stem_idx: Option<NonMaxUsize> = None;
        let mut is_right_child = false;

        while stem_strat.level() <= self.max_stem_level {
            let stem_idx = stem_strat.stem_idx();

            // Check if this stem points directly to a leaf (only for Mapped)
            if let Some(_leaf_idx) = self.resolve_terminal_stem(stem_idx) {
                return (stem_strat, parent_stem_idx, is_right_child);
            }

            parent_stem_idx = Some(NonMaxUsize::new(stem_idx).unwrap());
            let pivot = unsafe { self.stems.get_unchecked(stem_idx) };
            is_right_child = unsafe { *query.get_unchecked(stem_strat.dim::<K>()) } >= *pivot;
            stem_strat.traverse::<A, K>(is_right_child);
        }

        (stem_strat, parent_stem_idx, is_right_child)
    }

    /// Split a full leaf, moving some points in the existing leaf to a new one.
    /// Updates the stem tree to contain the new pivot value, pointing to the existing and
    /// split-off leaf.
    ///
    /// Returns the dimension along which the split occurred and the value of the pivot, as well
    /// as the new leaf index.
    fn split_leaf(
        &mut self,
        leaf_idx: usize,
        stem_strategy: SS,
        _parent_stem_idx: Option<NonMaxUsize>,
        _is_right_child: bool,
    ) -> Result<(A, usize, usize), ConstructionError> {
        let old_leaf_idx = leaf_idx; // stem_strategy.leaf_idx();
        let split_dim = stem_strategy.dim::<K>();

        // Split the leaf
        let (pivot_val, new_leaf_idx, split_dim) =
            self.leaves.split_leaf(old_leaf_idx, split_dim)?;

        // Get the indices of the children of the stem at which the split occurs
        let (left_child_idx, right_child_idx) = stem_strategy.child_indices::<A>();
        let stem_idx = stem_strategy.stem_idx();

        // Ensure the stem array is large enough
        if self.stems.len() < stem_idx + 1 {
            self.stems.resize(stem_idx + 1, A::max_value());
            crate::huge_pages::maybe_advise_slice_huge_pages(self.stems.as_ptr(), self.stems.len());
        }

        self.stems[stem_idx] = pivot_val;

        // Update the leaf_idx_map to point children to the two leaves
        if let OwnedStemLeafResolution::Mapped { leaf_idx_map, .. } = &mut self.stem_leaf_resolution
        {
            // Ensure the map is large enough
            if leaf_idx_map.len() < right_child_idx + 1 {
                leaf_idx_map.resize(right_child_idx + 1, None);
            }

            // Map left child to old leaf
            leaf_idx_map[left_child_idx] = leaf_idx_map[stem_idx];
            // Clear the root's mapping (it's now an interior node, not a leaf)
            leaf_idx_map[stem_idx] = None;
            // Map right child to new leaf
            leaf_idx_map[right_child_idx] = NonMaxUsize::new(new_leaf_idx);
        }

        // Track actual deepest interior stem level reached by splits.
        // Splitting a leaf at level L converts that terminal stem into an interior pivot.
        self.max_stem_level = self.max_stem_level.max(stem_strategy.level());

        Ok((pivot_val, split_dim, new_leaf_idx))
    }

    /*    /// Transition from Pristine to Mapped state on first split
    #[allow(unused)]
    fn taint_if_pristine(
        &mut self,
        new_stem_idx: usize,
        _left_leaf_idx: usize,
        _right_leaf_idx: usize,
        parent_stem_idx: Option<usize>,
    ) {
        match &self.stem_leaf_resolution {
            OwnedStemLeafResolution::Pristine {
                stems_depth,
                leaf_count,
            } => {
                // Transition to Mapped
                let min_stem_leaf_idx = 1 << *stems_depth;
                let mut leaf_idx_map = vec![None; self.stems.len()];

                // Map all existing leaves using arithmetic
                for i in 0..*leaf_count {
                    let stem_idx = min_stem_leaf_idx + i;
                    if stem_idx < leaf_idx_map.len() {
                        leaf_idx_map[stem_idx - min_stem_leaf_idx] = NonMaxUsize::new(i);
                    }
                }

                // Update mapping for the new stem and leaves
                if let Some(parent_idx) = parent_stem_idx {
                    // Clear parent's mapping (it now has children)
                    if parent_idx >= min_stem_leaf_idx {
                        leaf_idx_map[parent_idx - min_stem_leaf_idx] = None;
                    }
                }

                // New stem points to the two leaves
                if new_stem_idx >= min_stem_leaf_idx {
                    let idx = new_stem_idx - min_stem_leaf_idx;
                    if idx >= leaf_idx_map.len() {
                        leaf_idx_map.resize(idx + 1, None);
                    }
                }

                self.stem_leaf_resolution = OwnedStemLeafResolution::Mapped {
                    min_stem_leaf_idx,
                    leaf_idx_map,
                };
            }
            OwnedStemLeafResolution::Mapped { .. } => {
                // Already mapped, just update the mapping
                // TODO: implement mapping updates
            }
            _ => {
                // Arithmetic/Immutable - should not be calling this
                panic!("Cannot split leaves in immutable tree");
            }
        }
    }*/

    /// Removes a point and associated item from the tree.
    ///
    /// Note: This does not rebalance the tree.
    pub fn remove(&mut self, point: &[A; K], item: T) {
        let leaf_idx = self.get_leaf_idx(point);
        let old_leaf_len = self.leaves.leaf_len(leaf_idx);

        self.leaves.remove_from_leaf(leaf_idx, point, item);
        let new_leaf_len = self.leaves.leaf_len(leaf_idx);
        self.size -= old_leaf_len - new_leaf_len;

        // TODO: attempt to prune leaf if now empty
    }
}

/// FORK DEVIATION (see ADAPTIVE_SPLIT.md): rebalancing a degenerate tree.
///
/// Stems are addressed breadth-first, so a leaf at depth `d` needs `~2^d` slots, and a spatially
/// coherent insertion order — any mesh vertex buffer walked in order — deepens one path instead of
/// broadening the tree. `stems.resize` then allocates *and fills* an array that doubles per level,
/// which is why wall-clock time and resident memory blow up together rather than separately.
///
/// Incremental insertion cannot dig itself out: re-inserting the same points in a different order
/// does not reliably give a shallower tree, and on a spatially concentrated subset it frequently
/// gives a deeper one. Bulk construction can, because it partitions from the whole point set at
/// once. This pairs the two — a cheap shape test on each insert, and a bulk rebuild when it trips.
impl<A, T, SS, LS, const K: usize, const B: usize> KdTree<A, T, SS, LS, K, B>
where
    A: Axis<Coord = A>,
    T: Content,
    SS: StemStrategy,
    LS: MutableLeafStrategy<A, T, SS, K, B> + ConstructibleLeafStrategy<A, T, SS, K, B>,
{
    /// Allocated stem slots per split actually performed, above which [`Self::add_rebalancing`]
    /// rebuilds.
    ///
    /// Set from measurement, and the numbers matter because the obvious value is wrong. A
    /// *bulk-built* tree sits at 2.4–3.9, but an incrementally built one is legitimately less
    /// balanced and gets relatively deeper as it grows: on well-spread input this ratio peaks at
    /// 3.2 / 4.7 / 5.9 / 9.4 for 2k / 6k / 20k / 60k points, roughly logarithmic. A threshold of 8
    /// would therefore rebuild healthy trees past ~50k points.
    ///
    /// Degenerate trees are nowhere near: coherent insertion reaches 304 at only 2,000 points and
    /// 8,101 by 20,000. 64 sits between the two with several times' margin on each side, and bounds
    /// the stem array at `64 x leaves` — about 5.6 MB for the benchmark corpus, against the
    /// 15,203,841 slots an un-rebalanced coherent build reaches.
    ///
    /// The headroom is finite rather than infinite, since the healthy figure grows with `n`. It is
    /// ample for millions of points; a caller inserting billions coherently should re-measure.
    pub const REBALANCE_SLOT_RATIO: usize = 64;

    /// Below this many leaves the ratio means little — a two-leaf tree legitimately sits at 4 — so
    /// no rebuild is considered.
    pub const REBALANCE_MIN_LEAVES: usize = 8;

    /// Allocated stem slots divided by splits performed. See [`Self::REBALANCE_SLOT_RATIO`].
    pub fn stem_slot_ratio(&self) -> f64 {
        let splits = self.leaves.leaf_count().saturating_sub(1);
        if splits == 0 {
            return 1.0;
        }
        self.stems.len() as f64 / splits as f64
    }

    /// Rebuild from the points currently held, discarding the depth the stem array has accumulated.
    ///
    /// Bulk construction is order-independent, so this is not "the same insertions again" — it is
    /// the tree the points would have produced had they all been available up front.
    pub fn rebalance(&mut self) -> Result<(), ConstructionError> {
        let entries: Vec<(T, [A; K])> = self.iter().collect();
        *self = Self::new_from_entries(&entries)?;
        Ok(())
    }

    /// [`Self::add`], preceded by a rebuild when the insert is about to demand a disproportionate
    /// stem array.
    ///
    /// The test is on the slots the *pending* split would require, before they are allocated, so
    /// the pathological array is never created — as opposed to created, measured and discarded,
    /// which would already have paid for it.
    pub fn add_rebalancing(&mut self, point: &[A; K], item: T) -> Result<(), ConstructionError> {
        let leaves = self.leaves.leaf_count();
        if leaves >= Self::REBALANCE_MIN_LEAVES {
            let budget = Self::REBALANCE_SLOT_RATIO.saturating_mul(leaves.saturating_sub(1).max(1));
            if self
                .pending_split_slots(point)
                .is_some_and(|needed| needed > budget)
            {
                // A rebuild that cannot be performed is not an insertion failure. Bulk
                // construction may legitimately refuse a point set — a bucket of near-coincident
                // points has no separating plane on any axis — and `rebalance` leaves the tree
                // untouched when it does. Propagating that would make an index that merely wants
                // reshaping *less* usable than one that never tried, so the optimisation is
                // skipped and the insert proceeds against the tree we already have.
                let _ = self.rebalance();
            }
        }
        self.add(point, item)
    }

    /// Stem slots the split triggered by inserting `point` would require, or `None` when the target
    /// leaf has room and no split is pending.
    fn pending_split_slots(&self, point: &[A; K]) -> Option<usize> {
        let (stem_strat, _, _) = self.find_leaf_with_context(point);
        let leaf_idx = match &self.stem_leaf_resolution {
            OwnedStemLeafResolution::Mapped { leaf_idx_map, .. } => leaf_idx_map
                .get(stem_strat.stem_idx())
                .copied()
                .flatten()?
                .get(),
            _ => stem_strat.leaf_idx(),
        };
        if !self.leaves.is_leaf_full(leaf_idx) {
            return None;
        }
        // Splitting this leaf turns its terminal stem into an interior one, so the array has to
        // reach the further of the two children it gains.
        let (_, right_child_idx) = stem_strat.child_indices::<A>();
        Some(right_child_idx + 1)
    }
}

impl<A, T, SS, LS, const K: usize, const B: usize> KdTree<A, T, SS, LS, K, B>
where
    A: Axis<Coord = A>,
    T: Content + PartialEq,
    SS: StemStrategy,
    LS: LeafStrategy<A, T, SS, K, B>,
{
    /// Replaces the first exact `(point, old_item)` match with `new_item`.
    ///
    /// Returns [`MutationError::EntryNotFound`] if the target leaf contains no
    /// entry whose point and item both match exactly.
    pub fn replace_item(
        &mut self,
        point: &[A; K],
        old_item: T,
        new_item: T,
    ) -> Result<(), MutationError> {
        let leaf_idx = self.get_leaf_idx(point);

        self.leaves
            .replace_item_in_leaf(leaf_idx, point, old_item, new_item)
            .then_some(())
            .ok_or(MutationError::EntryNotFound)
    }
}

// Shared construction implementation (works for both Immutable and Mutable)
impl<A, T, SS, LS, const K: usize, const B: usize> KdTree<A, T, SS, LS, K, B>
where
    A: Axis<Coord = A>,
    T: Content,
    SS: StemStrategy,
    LS: ConstructibleLeafStrategy<A, T, SS, K, B>,
{
    /// Creates a `KdTree` from a slice of points.
    ///
    /// Items are auto-generated from the point index in the input slice using
    /// `T::try_from(index)`. This is the most convenient constructor when the
    /// caller only has points and is happy for item values to mirror their
    /// original position in the source slice.
    ///
    /// Returns [`ConstructionError::AutoGeneratedItemIndexOverflow`] if the
    /// source length cannot be represented by `T`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kiddo::KdTree;
    /// use kiddo::leaf_strategy::FlatVec;
    /// use kiddo::Eytzinger;
    ///
    /// let points = vec![
    ///     [1.0f64, 2.0f64, 3.0f64],
    ///     [4.0f64, 5.0f64, 6.0f64],
    /// ];
    ///
    /// let tree: KdTree<f64, u32, Eytzinger, FlatVec<f64, u32, 3, 32>, 3, 32> =
    ///     KdTree::new_from_slice(&points).unwrap();
    ///
    /// assert_eq!(tree.size(), 2);
    /// assert_eq!(
    ///     tree.iter().collect::<Vec<_>>(),
    ///     vec![(0u32, [1.0, 2.0, 3.0]), (1u32, [4.0, 5.0, 6.0])]
    /// );
    /// ```
    #[cfg_attr(not(feature = "no_inline"), inline)]
    // TODO: Add checked, Result-returning ingress APIs (`new_from_slice`,
    // `new_from_slice_no_items`, and mutable `add`) that reject coordinates
    // equal to `A::max_value()`, and rename the current behavior to
    // `*_unchecked`.
    pub fn new_from_slice(source: &[[A; K]]) -> Result<Self, ConstructionError>
    where
        T: TryFrom<usize>,
    {
        if let Some(max_src_idx) = source.len().checked_sub(1) {
            if T::try_from(max_src_idx).is_err() {
                return Err(ConstructionError::AutoGeneratedItemIndexOverflow {
                    item_count: source.len(),
                    item_type: core::any::type_name::<T>(),
                });
            }
        }

        Self::new_from_source_with(
            source,
            |point: &[A; K], dim| point[dim],
            |src_idx: usize, _point: &[A; K]| {
                T::try_from(src_idx).map_err(|_| {
                    ConstructionError::AutoGeneratedItemIndexOverflow {
                        item_count: source.len(),
                        item_type: core::any::type_name::<T>(),
                    }
                })
            },
        )
    }

    /// Creates a `KdTree` from a generic slice source plus axis/item accessors.
    ///
    /// This is the most general bulk-construction ingress API. Callers provide
    /// one callback to read the coordinate value for a source item and
    /// dimension, and another callback to produce the stored item value.
    ///
    /// Use this when your source data is not already shaped as `&[[A; K]]` or
    /// `&[(T, [A; K])]`, for example when points and IDs live in fields on a
    /// custom struct.
    ///
    /// The `axis_at` accessor is on the hot path during construction. It is
    /// called exactly `n * k` times to materialize leaf storage for a tree
    /// with `n` source items and dimensionality `k`, plus additional calls
    /// during recursive pivot selection and partitioning. In practice, total
    /// accessor usage is roughly `n * k + O(n log(n))`, so expensive accessors
    /// can noticeably slow down construction. When your data is already
    /// available as a `&[[A; K]]`, prefer [`KdTree::new_from_slice`] to avoid
    /// that extra accessor overhead.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kiddo::KdTree;
    /// use kiddo::leaf_strategy::FlatVec;
    /// use kiddo::Eytzinger;
    ///
    /// #[derive(Clone, Copy)]
    /// struct Point3D {
    ///     id: u32,
    ///     x: f32,
    ///     y: f32,
    ///     z: f32,
    ///     w: f32,
    /// }
    ///
    /// let points = [
    ///     Point3D { id: 10, x: 1.0, y: 2.0, z: 3.0, w: 0.5 },
    ///     Point3D { id: 20, x: 4.0, y: 5.0, z: 6.0, w: 0.7 },
    /// ];
    ///
    /// let tree: KdTree<f32, u32, Eytzinger, FlatVec<f32, u32, 3, 32>, 3, 32> =
    ///     KdTree::new_from_source(
    ///         &points,
    ///         |point, dim| match dim {
    ///             0 => point.x,
    ///             1 => point.y,
    ///             2 => point.z,
    ///             _ => unreachable!(),
    ///         },
    ///         |_idx, point| point.id,
    ///     )
    ///     .unwrap();
    ///
    /// assert_eq!(
    ///     tree.iter().collect::<Vec<_>>(),
    ///     vec![(10u32, [1.0, 2.0, 3.0]), (20u32, [4.0, 5.0, 6.0])]
    /// );
    /// ```
    #[cfg_attr(not(feature = "no_inline"), inline)]
    pub fn new_from_source<X, FA, FI>(
        source: &[X],
        axis_at: FA,
        item_at: FI,
    ) -> Result<Self, ConstructionError>
    where
        FA: Fn(&X, usize) -> A,
        FI: Fn(usize, &X) -> T,
    {
        Self::new_from_source_with(source, axis_at, |src_idx, src| Ok(item_at(src_idx, src)))
    }

    /// Creates a `KdTree` from explicit item/point pairs.
    ///
    /// This is the preferred ingress when callers already have items rather
    /// than wanting `new_from_slice` to auto-generate them from source indices.
    ///
    /// Unlike [`KdTree::new_from_slice`], item values are taken directly from
    /// the input rather than derived from position in the slice.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kiddo::KdTree;
    /// use kiddo::leaf_strategy::FlatVec;
    /// use kiddo::Eytzinger;
    ///
    /// let entries = vec![
    ///     (42u32, [0.0f32, 1.0f32]),
    ///     (7u32, [2.0f32, 3.0f32]),
    /// ];
    ///
    /// let tree: KdTree<f32, u32, Eytzinger, FlatVec<f32, u32, 2, 32>, 2, 32> =
    ///     KdTree::new_from_entries(&entries).unwrap();
    ///
    /// assert_eq!(tree.size(), 2);
    /// assert_eq!(tree.iter().collect::<Vec<_>>(), entries);
    /// ```
    #[cfg_attr(not(feature = "no_inline"), inline)]
    pub fn new_from_entries(source: &[(T, [A; K])]) -> Result<Self, ConstructionError> {
        Self::new_from_source(
            source,
            |entry: &(T, [A; K]), dim| entry.1[dim],
            |_src_idx, entry: &(T, [A; K])| entry.0,
        )
    }

    /// Inner constructor shared by all variants. The accessors are invoked
    /// wherever we would normally pull coordinates or items from the source.
    fn new_from_source_with<X, FA, FI>(
        source: &[X],
        axis_at: FA,
        mut item_at: FI,
    ) -> Result<Self, ConstructionError>
    where
        FA: Fn(&X, usize) -> A,
        FI: FnMut(usize, &X) -> Result<T, ConstructionError>,
    {
        let item_count = source.len();
        let leaf_node_count = item_count.div_ceil(B);

        if leaf_node_count < 2 {
            // A single leaf has no stems, so there is no split dimension to record and nothing
            // for an adaptive strategy to disagree with.
            return Self::new_from_source_no_stems_with(source, &axis_at, item_at);
        }

        // FORK DEVIATION (see ADAPTIVE_SPLIT.md): adaptive strategies used to be refused here,
        // because this builder wrote the coordinate it had already partitioned around and folding
        // a dimension into that value afterwards would shift each plane off its own partition.
        // `split_value_for_stem` now chooses a grid-aligned separator *before* the value is
        // stored, so the stored plane and the partition agree and the refusal is gone. The split
        // axis itself still cycles with depth (`construction_dim`); adaptive selection at build
        // time is a separate change.
        let mut stems_depth: usize = leaf_node_count.next_power_of_two().ilog2() as usize;

        // Pad stem tree height to the next block boundary for block-based strategies
        let padding_level_count = if !stems_depth.is_multiple_of(SS::block_size()) {
            let padding_level_count = SS::block_size() - (stems_depth % SS::block_size());
            stems_depth += padding_level_count;
            padding_level_count
        } else {
            0
        };

        // Padding levels will be placed at the root of the tree. Pre-traverse any padding levels
        // so that stem_strat is set to the location where the true root will be
        let mut stem_strat = SS::new_no_ptr();
        for _ in 0..padding_level_count {
            stem_strat.traverse::<A, K>(false);
        }
        let root_stem_strat = stem_strat.clone();

        let soft_leaf_budget = if LS::BUCKET_LIMIT_TYPE == BucketLimitType::Soft {
            1usize << stems_depth
        } else {
            leaf_node_count
        };

        // Traverse to the right-most represented leaf to determine the max used stem index
        let rightmost_leaf_idx = soft_leaf_budget - 1;
        let rightmost_leaf_bit_range = if LS::BUCKET_LIMIT_TYPE == BucketLimitType::Soft {
            0..stems_depth
        } else {
            1..stems_depth
        };
        for bit_idx in rightmost_leaf_bit_range.rev() {
            let is_right = rightmost_leaf_idx & (1 << bit_idx) != 0;
            stem_strat.traverse::<A, K>(is_right);
        }
        let stem_node_count = stem_strat.stem_idx() + 1;

        // rounded up to the nearest multiple of 8 if not a multiple of 8 already
        let stem_node_count_padded = stem_node_count.div_ceil(8) * 8;
        let mut stems = avec![A::max_value(); stem_node_count_padded];

        let mut leaves = LS::new_with_capacity(item_count);
        let mut terminal_stem_indices = Vec::with_capacity(leaf_node_count);
        let mut actual_max_stem_level: i32 = -1;
        let mut max_leaf_len = 0usize;
        let mut sort_index = Vec::from_iter(0..item_count);

        match LS::BUCKET_LIMIT_TYPE {
            BucketLimitType::Hard => Self::populate_recursive_hard(
                &mut stems,
                source,
                &axis_at,
                &mut sort_index,
                root_stem_strat,
                stems_depth as i32 - 1,
                leaf_node_count * B,
                &mut leaves,
                &mut terminal_stem_indices,
                &mut actual_max_stem_level,
                &mut max_leaf_len,
                &mut item_at,
            )?,
            BucketLimitType::Soft => Self::populate_recursive_soft(
                &mut stems,
                source,
                &axis_at,
                &mut sort_index,
                root_stem_strat,
                stems_depth as i32 - 1,
                soft_leaf_budget,
                &mut leaves,
                &mut actual_max_stem_level,
                &mut max_leaf_len,
                &mut item_at,
            )?,
        }

        let initial_max_stem_level = stems_depth as i32 - 1;
        let requires_mapped_resolution = LS::Mutability::is_mutable()
            || LS::BUCKET_LIMIT_TYPE == BucketLimitType::Hard
                && (actual_max_stem_level > initial_max_stem_level
                    || padding_level_count != 0
                    || !Self::terminal_stem_indices_match_arithmetic_layout(
                        &terminal_stem_indices,
                        actual_max_stem_level,
                    ));

        let stem_leaf_resolution = if requires_mapped_resolution {
            Self::mapped_stem_leaf_resolution_from_terminals(&terminal_stem_indices)
        } else {
            LS::Mutability::initial_stem_leaf_resolution::<A, SS, K>(
                stems_depth,
                leaves.leaf_count(),
            )
        };

        crate::leaf_view::assert_leaf_scratch_capacity(max_leaf_len);

        let tree = Self {
            stems,
            leaves,
            stem_leaf_resolution,
            size: item_count,
            max_stem_level: actual_max_stem_level,
            max_leaf_len,
            _phantom: Default::default(),
        };
        tree.maybe_enable_huge_pages();
        Ok(tree)
    }
    fn new_from_source_no_stems_with<X, FA, FI>(
        source: &[X],
        axis_at: &FA,
        mut item_at: FI,
    ) -> Result<Self, ConstructionError>
    where
        FA: Fn(&X, usize) -> A,
        FI: FnMut(usize, &X) -> Result<T, ConstructionError>,
    {
        let item_count = source.len();

        if item_count == 0 {
            return Ok(Self::default());
        }

        let mut leaf_points: [Vec<A>; K] =
            array_init::array_init(|_| Vec::with_capacity(item_count));
        let mut leaf_items: Vec<T> = Vec::with_capacity(item_count);

        for idx in 0..item_count {
            for dim in 0..K {
                leaf_points[dim].push(axis_at(&source[idx], dim));
            }
            leaf_items.push(item_at(idx, &source[idx])?);
        }

        let leaf_points_refs: [&[A]; K] = array_init::array_init(|dim| leaf_points[dim].as_slice());

        let mut leaves = LS::new_with_capacity(item_count);
        leaves.append_leaf(&leaf_points_refs, leaf_items.as_slice());

        let stem_leaf_resolution =
            LS::Mutability::initial_stem_leaf_resolution::<A, SS, K>(0, leaves.leaf_count());

        let max_leaf_len = item_count;
        crate::leaf_view::assert_leaf_scratch_capacity(max_leaf_len);

        let tree = Self {
            stems: avec![A::max_value(); 0],
            leaves,
            stem_leaf_resolution,
            size: item_count,
            max_stem_level: -1,
            max_leaf_len,
            _phantom: Default::default(),
        };
        tree.maybe_enable_huge_pages();
        Ok(tree)
    }
}

impl<A, SS, LS, const K: usize, const B: usize> KdTree<A, (), SS, LS, K, B>
where
    A: Axis<Coord = A>,
    SS: StemStrategy,
    LS: ConstructibleLeafStrategy<A, (), SS, K, B>,
{
    /// Creates a `KdTree` with no stored item values (`T = ()`).
    ///
    /// Leaf item slices will have the correct length but contain only `()`.
    /// LLVM can generally optimize the `Vec<()>` storage away.
    ///
    /// This is useful when the points themselves are the only data you need to
    /// store and query.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use kiddo::KdTree;
    /// use kiddo::leaf_strategy::FlatVec;
    /// use kiddo::Eytzinger;
    ///
    /// let points = vec![[1.0f64, 2.0f64], [3.0f64, 4.0f64]];
    ///
    /// let tree: KdTree<f64, (), Eytzinger, FlatVec<f64, (), 2, 32>, 2, 32> =
    ///     KdTree::new_from_slice_no_items(&points).unwrap();
    ///
    /// assert_eq!(tree.size(), 2);
    /// assert_eq!(
    ///     tree.iter().collect::<Vec<_>>(),
    ///     vec![((), [1.0, 2.0]), ((), [3.0, 4.0])]
    /// );
    /// ```
    #[cfg_attr(not(feature = "no_inline"), inline)]
    pub fn new_from_slice_no_items(source: &[[A; K]]) -> Result<Self, ConstructionError> {
        Self::new_from_source(
            source,
            |point: &[A; K], dim| point[dim],
            |_src_idx, _point| (),
        )
    }
}

// Shared utility methods for construction (available to both Immutable and Mutable)
impl<A, T, SS, LS, const K: usize, const B: usize> KdTree<A, T, SS, LS, K, B>
where
    A: Axis<Coord = A>,
    T: Content,
    SS: StemStrategy,
    LS: ConstructibleLeafStrategy<A, T, SS, K, B>,
{
    /// The value to store at a stem for the split that puts `sort_index[..pivot]` left and
    /// `sort_index[pivot..]` right.
    ///
    /// Cycling strategies store the boundary coordinate itself, exactly as upstream does, so
    /// their behaviour is bit-identical.
    ///
    /// FORK DEVIATION (see ADAPTIVE_SPLIT.md): an adaptive strategy stores its split dimension
    /// in the low [`dim_bits`](crate::stem_strategy::dim_packing::dim_bits) bits of the split
    /// value, and queries compare against **that stored value**. Encoding the boundary
    /// coordinate in place would move it by up to one grid cell while the partition stays fixed
    /// by index, stranding every point between the old and new plane on the wrong side of its
    /// own split — a silently wrong nearest-neighbour, not an error. So the value is *chosen* to
    /// separate the partition instead of being derived from it: bracket the boundary, ask
    /// [`grid_separator`](crate::stem_strategy::dim_packing::grid_separator) for an aligned value
    /// strictly inside the bracket, and encode into the bits it left free. Because the separator
    /// lies strictly between the two bracketing coordinates, the index partition and the value
    /// partition coincide and no re-partitioning is needed.
    /// Choose the axis, the partition and the value to store for one internal node.
    ///
    /// Cycling strategies have no choice to make: the axis is implied by depth, so this is
    /// `update_pivot` plus the boundary coordinate, exactly as upstream.
    ///
    /// FORK DEVIATION (see ADAPTIVE_SPLIT.md): an adaptive strategy records its axis in the
    /// stored value, so construction is free to pick a different one — and sometimes must. Two
    /// reasons an axis can be unusable: every point in the chunk shares a value on it, or its two
    /// bracketing values sit closer together than one grid cell so no aligned plane fits between
    /// them. The second was expected to be unreachable on quantised data; it is not, because
    /// interpolated mesh vertices are not exactly on the lattice they were derived from.
    ///
    /// Candidates are tried widest-spread first, which both avoids those cases and keeps the tree
    /// shallow — the whole point of bulk construction here is a balanced tree, and `2^depth` stem
    /// addressing makes every wasted level expensive.
    ///
    /// Returns the axis used, the partition boundary, and the value to store — `None` when the
    /// partition puts everything on one side, in which case no stem is written.
    #[allow(clippy::type_complexity)]
    fn choose_split<X, FA>(
        source: &[X],
        axis_at: &FA,
        sort_index: &mut [usize],
        preferred_dim: usize,
        init_pivot: usize,
    ) -> Result<(usize, usize, Option<A>), ConstructionError>
    where
        FA: Fn(&X, usize) -> A,
    {
        if !SS::ADAPTIVE_SPLIT_DIM {
            let pivot = Self::update_pivot(source, axis_at, sort_index, preferred_dim, init_pivot)?;
            let value = (pivot < sort_index.len())
                .then(|| (*axis_at)(&source[sort_index[pivot]], preferred_dim));
            return Ok((preferred_dim, pivot, value));
        }

        // Spread per axis over this chunk, so the widest is tried first: it is the most likely to
        // admit a separator and it keeps the tree shallow. `saturating_dist` gives a real width,
        // which matters — an ordering built from range containment is not a total order and
        // `sort_by` is entitled to reject it.
        let mut order: Vec<usize> = (0..K).collect();
        let spread: Vec<A> = (0..K)
            .map(|dim| {
                let mut lo = A::max_value();
                let mut hi = A::min_value();
                for &idx in sort_index.iter() {
                    let value = (*axis_at)(&source[idx], dim);
                    if A::cmp(value, lo) == std::cmp::Ordering::Less {
                        lo = value;
                    }
                    if A::cmp(hi, value) == std::cmp::Ordering::Less {
                        hi = value;
                    }
                }
                A::saturating_dist(lo, hi)
            })
            .collect();
        order.sort_by(|&a, &b| {
            // Widest first, then the cycled axis's precedence so ties stay deterministic and
            // well-conditioned data keeps behaving as it did.
            A::cmp(spread[b], spread[a]).then_with(|| {
                let rank = |d: usize| (d + K - preferred_dim) % K;
                rank(a).cmp(&rank(b))
            })
        });

        let mut last_error = None;
        for dim in order {
            let pivot = match Self::update_pivot(source, axis_at, sort_index, dim, init_pivot) {
                Ok(pivot) => pivot,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };
            if pivot >= sort_index.len() {
                // Everything on one side: nothing to store, and no axis would change that.
                return Ok((dim, pivot, None));
            }
            match Self::split_value_for_stem(source, axis_at, sort_index, dim, pivot) {
                Ok(value) => return Ok((dim, pivot, Some(value))),
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error.unwrap_or(ConstructionError::UnsplittableBucket {
            split_dim: preferred_dim,
        }))
    }

    fn split_value_for_stem<X, FA>(
        source: &[X],
        axis_at: &FA,
        sort_index: &[usize],
        dim: usize,
        pivot: usize,
    ) -> Result<A, ConstructionError>
    where
        FA: Fn(&X, usize) -> A,
    {
        let upper = (*axis_at)(&source[sort_index[pivot]], dim);
        if !SS::ADAPTIVE_SPLIT_DIM {
            return Ok(upper);
        }

        // `select_nth_unstable_by` only guarantees that everything left of `pivot` is <= the
        // element *at* `pivot`; the element at `pivot - 1` is merely one of them, not necessarily
        // the largest. The separator has to clear the true maximum, or some other left-hand point
        // tests `>= stored` and queries for it descend right while it lives in the left leaf.
        let mut lower = None;
        for &idx in &sort_index[..pivot] {
            let value = (*axis_at)(&source[idx], dim);
            if lower.is_none_or(|current| A::cmp(current, value) == std::cmp::Ordering::Less) {
                lower = Some(value);
            }
        }
        // An empty left partition, or a left maximum that reaches the boundary value, leaves no
        // plane that separates the two sides. `update_pivot` is supposed to have ruled the latter
        // out; checking is cheap and the alternative is a silently wrong tree.
        let lower = match lower {
            Some(lower) if A::cmp(lower, upper) == std::cmp::Ordering::Less => lower,
            _ => return Err(ConstructionError::UnsplittableBucket { split_dim: dim }),
        };

        let separator = crate::stem_strategy::dim_packing::grid_separator(
            lower,
            upper,
            size_of::<A>(),
            dim,
            crate::stem_strategy::dim_packing::dim_mask(K),
        )
        .ok_or(ConstructionError::UnsplittableBucket { split_dim: dim })?;

        Ok(SS::encode_split::<A, K>(separator, dim))
    }

    fn write_leaf_from_sort_index<X, FA, FI>(
        source: &[X],
        axis_at: &FA,
        sort_index: &[usize],
        leaves: &mut LS,
        max_leaf_len: &mut usize,
        item_at: &mut FI,
    ) -> Result<(), ConstructionError>
    where
        FA: Fn(&X, usize) -> A,
        FI: FnMut(usize, &X) -> Result<T, ConstructionError>,
    {
        let leaf_len = sort_index.len();
        *max_leaf_len = (*max_leaf_len).max(leaf_len);

        let mut leaf_points: [Vec<A>; K] = array_init::array_init(|_| Vec::with_capacity(leaf_len));
        let mut leaf_items: Vec<T> = Vec::with_capacity(leaf_len);

        for &src_idx in sort_index {
            for d in 0..K {
                leaf_points[d].push(axis_at(&source[src_idx], d));
            }
            leaf_items.push(item_at(src_idx, &source[src_idx])?);
        }

        let leaf_points_refs: [&[A]; K] = array_init::array_init(|d| leaf_points[d].as_slice());
        leaves.append_leaf(&leaf_points_refs, leaf_items.as_slice());

        Ok(())
    }

    #[inline(always)]
    fn soft_left_leaf_budget(leaf_budget: usize) -> usize {
        debug_assert!(leaf_budget > 1);
        1usize << ((usize::BITS - 1 - (leaf_budget - 1).leading_zeros()) as usize)
    }

    #[inline(always)]
    fn soft_ideal_pivot(chunk_length: usize, left_leaf_budget: usize, leaf_budget: usize) -> usize {
        debug_assert!(leaf_budget > 0);
        debug_assert!(left_leaf_budget < leaf_budget);

        if chunk_length == 0 {
            return 0;
        }

        chunk_length
            .saturating_mul(left_leaf_budget)
            .div_ceil(leaf_budget)
            .clamp(1, chunk_length)
    }

    /// Hard-bucket recursive construction helper.
    #[allow(clippy::too_many_arguments)]
    fn populate_recursive_hard<X, FA, FI>(
        stems: &mut AVec<A, ConstAlign<{ CACHELINE_ALIGN }>>,
        source: &[X],
        axis_at: &FA,
        sort_index: &mut [usize],
        mut stem_ordering: SS,
        max_stem_level: i32,
        capacity: usize,
        leaves: &mut LS,
        terminal_stem_indices: &mut Vec<usize>,
        actual_max_stem_level: &mut i32,
        max_leaf_len: &mut usize,
        item_at: &mut FI,
    ) -> Result<(), ConstructionError>
    where
        FA: Fn(&X, usize) -> A,
        FI: FnMut(usize, &X) -> Result<T, ConstructionError>,
    {
        let chunk_length = sort_index.len();
        let dim = stem_ordering.construction_dim::<K>();

        debug_assert!(
            chunk_length > 0,
            "recursed an empty chunk (stem_idx={}, level={}, chunk_length={}, capacity={})",
            stem_ordering.stem_idx(),
            stem_ordering.level(),
            chunk_length,
            capacity,
        );

        if chunk_length <= B {
            Self::write_leaf_from_sort_index(
                source,
                axis_at,
                sort_index,
                leaves,
                max_leaf_len,
                item_at,
            )?;
            terminal_stem_indices.push(stem_ordering.stem_idx());
            return Ok(());
        }

        let levels_below = max_stem_level - stem_ordering.level();
        let clamped_levels_below = levels_below.max(0) as u32;
        let left_capacity = (2usize.pow(clamped_levels_below) * B).min(capacity);
        let right_capacity = capacity.saturating_sub(left_capacity);

        debug_assert!(
            left_capacity > 0,
            "left_capacity is zero - should never happen (stem_idx={}, level={}, chunk_length={}, capacity={}, right_capacity={})",
            stem_ordering.stem_idx(),
            stem_ordering.level(),
            chunk_length,
            capacity,
            right_capacity
        );

        let stem_index = stem_ordering.stem_idx();
        *actual_max_stem_level = (*actual_max_stem_level).max(stem_ordering.level());

        if stem_index >= stems.len() {
            tracing::warn!(
                %stem_index,
                existing_stem_vec_len = %stems.len(),
                "encountered a stem index beyond the end of the stem vec. Growing the vec to fit"
            );

            stems.resize(stem_index + 1, A::max_value());
        }

        let mut pivot = Self::calc_pivot(
            chunk_length,
            stem_index,
            right_capacity,
            LS::BUCKET_LIMIT_TYPE,
        );

        debug_assert!(
            pivot > 0,
            "construction produced initial pivot=0 (empty-left split candidate): \
            stem_index = {}, level={}, chunk_length = {}, capacity = {}, \
            left_capacity = {}, right_capacity = {}, dim = {}",
            stem_index,
            stem_ordering.level(),
            chunk_length,
            capacity,
            left_capacity,
            right_capacity,
            dim,
        );

        // only bother with this logic if we are putting at least one item in the right-hand child
        let mut dim = dim;
        if pivot < chunk_length {
            let (chosen_dim, chosen_pivot, split_value) =
                Self::choose_split(source, axis_at, sort_index, dim, pivot)?;
            dim = chosen_dim;
            pivot = chosen_pivot;

            debug_assert!(
                pivot > 0,
                "construction produced updated pivot=0 (empty-left split candidate): \
                stem_index = {}, level={}, chunk_length = {}, capacity = {}, \
                left_capacity = {}, right_capacity = {}, dim = {}",
                stem_index,
                stem_ordering.level(),
                chunk_length,
                capacity,
                left_capacity,
                right_capacity,
                dim,
            );

            // if we end up with a pivot of 0, something has gone wrong,
            // unless we only had a slice of len 1 anyway
            // debug_assert!(
            //     pivot > 0 || chunk_length == 1,
            // );

            // if LS::BUCKET_LIMIT_TYPE == BucketLimitType::Hard {
            //     debug_assert!(
            //         right_capacity >= chunk_length.saturating_sub(pivot),
            //         "right_capacity ({right_capacity}) should be greater than chunk_length - pivot ({chunk_length} - {pivot})"
            //     );
            // }

            if let Some(split_value) = split_value {
                debug_assert!(
                    A::Coord::is_max_value(stems[stem_index]),
                    "Wrote to stem #{stem_index:?} for a second time",
                );

                stems[stem_index] = split_value;
            }
        }

        // Capacity is a budget derived from the leaf count, but the actual partition can exceed
        // it: `calc_pivot` deliberately overrides the budget to force a split when everything
        // would otherwise land left (its "splitting finer" branch uses `right_capacity.max(B)`),
        // and the pivot moves again to avoid straddling equal values. Either can hand a child more
        // items than it was allotted, and a child that recurses with a capacity of zero over a
        // chunk larger than one bucket trips the `left_capacity > 0` assertion below.
        //
        // Widening the budget to the partition it has to describe is what keeps the two
        // consistent. It only ever raises a capacity, so it cannot tighten a split.
        let left_capacity = left_capacity.max(pivot);
        let right_capacity = right_capacity.max(chunk_length - pivot);

        let right_stem_ordering = stem_ordering.branch::<A, K>();
        let (lower_sort_index, upper_sort_index) = sort_index.split_at_mut(pivot);

        Self::populate_recursive_hard(
            stems,
            source,
            axis_at,
            lower_sort_index,
            stem_ordering,
            max_stem_level,
            left_capacity,
            leaves,
            terminal_stem_indices,
            actual_max_stem_level,
            max_leaf_len,
            item_at,
        )?;

        if !upper_sort_index.is_empty() {
            Self::populate_recursive_hard(
                stems,
                source,
                axis_at,
                upper_sort_index,
                right_stem_ordering,
                max_stem_level,
                right_capacity,
                leaves,
                terminal_stem_indices,
                actual_max_stem_level,
                max_leaf_len,
                item_at,
            )?;
        }

        Ok(())
    }

    /// Soft-bucket recursive construction helper preserving arithmetic layout.
    #[allow(clippy::too_many_arguments)]
    fn populate_recursive_soft<X, FA, FI>(
        stems: &mut AVec<A, ConstAlign<{ CACHELINE_ALIGN }>>,
        source: &[X],
        axis_at: &FA,
        sort_index: &mut [usize],
        mut stem_ordering: SS,
        max_stem_level: i32,
        leaf_budget: usize,
        leaves: &mut LS,
        actual_max_stem_level: &mut i32,
        max_leaf_len: &mut usize,
        item_at: &mut FI,
    ) -> Result<(), ConstructionError>
    where
        FA: Fn(&X, usize) -> A,
        FI: FnMut(usize, &X) -> Result<T, ConstructionError>,
    {
        if leaf_budget == 0 {
            return Ok(());
        }

        if stem_ordering.level() > max_stem_level {
            Self::write_leaf_from_sort_index(
                source,
                axis_at,
                sort_index,
                leaves,
                max_leaf_len,
                item_at,
            )?;
            return Ok(());
        }

        let chunk_length = sort_index.len();
        let dim = stem_ordering.construction_dim::<K>();
        let stem_index = stem_ordering.stem_idx();
        *actual_max_stem_level = (*actual_max_stem_level).max(stem_ordering.level());

        if stem_index >= stems.len() {
            tracing::warn!(
                %stem_index,
                existing_stem_vec_len = %stems.len(),
                "encountered a stem index beyond the end of the stem vec. Growing the vec to fit"
            );
            stems.resize(stem_index + 1, A::max_value());
        }

        let (left_leaf_budget, right_leaf_budget, pivot, split_value) = if leaf_budget == 1 {
            (1usize, 0usize, chunk_length, None)
        } else {
            let left_leaf_budget = Self::soft_left_leaf_budget(leaf_budget);
            let right_leaf_budget = leaf_budget - left_leaf_budget;
            let mut pivot = Self::soft_ideal_pivot(chunk_length, left_leaf_budget, leaf_budget);
            let mut split_value = None;
            if pivot < chunk_length {
                let (_, chosen_pivot, chosen_value) =
                    Self::choose_split(source, axis_at, sort_index, dim, pivot)?;
                pivot = chosen_pivot;
                split_value = chosen_value;
            }
            (left_leaf_budget, right_leaf_budget, pivot, split_value)
        };

        if let Some(split_value) = split_value {
            debug_assert!(
                A::Coord::is_max_value(stems[stem_index]),
                "Wrote to stem #{stem_index:?} for a second time",
            );
            stems[stem_index] = split_value;
        }

        let right_stem_ordering = stem_ordering.branch::<A, K>();
        let split_idx = pivot.min(chunk_length);
        let (lower_sort_index, upper_sort_index) = sort_index.split_at_mut(split_idx);

        Self::populate_recursive_soft(
            stems,
            source,
            axis_at,
            lower_sort_index,
            stem_ordering,
            max_stem_level,
            left_leaf_budget,
            leaves,
            actual_max_stem_level,
            max_leaf_len,
            item_at,
        )?;

        Self::populate_recursive_soft(
            stems,
            source,
            axis_at,
            upper_sort_index,
            right_stem_ordering,
            max_stem_level,
            right_leaf_budget,
            leaves,
            actual_max_stem_level,
            max_leaf_len,
            item_at,
        )?;

        Ok(())
    }

    // TODO: remove this entirely in favor of just taking ownership of terminal_stem_indices
    //       once confident that the debug_asserts never fire
    fn mapped_stem_leaf_resolution_from_terminals(
        terminal_stem_indices: &[usize],
    ) -> OwnedStemLeafResolution {
        if terminal_stem_indices.is_empty() {
            return OwnedStemLeafResolution::Mapped {
                min_stem_leaf_idx: 0,
                leaf_idx_map: Vec::new(),
            };
        }

        // TODO: this should not be needed. Just use terminal_stem_indices.len()
        let max_terminal_stem_idx = terminal_stem_indices.iter().copied().max().unwrap_or(0);
        // debug_assert!(
        //     max_terminal_stem_idx == terminal_stem_indices.len() - 1,
        //     "Leaf array should be contiguous. Construction invariant failed"
        // );
        if max_terminal_stem_idx > terminal_stem_indices.len() - 1 {
            tracing::warn!("Leaf array should be contiguous. Construction invariant failed");
        };
        let mut leaf_idx_map: Vec<Option<NonMaxUsize>> = vec![None; max_terminal_stem_idx + 1];

        for (leaf_idx, &terminal_stem_idx) in terminal_stem_indices.iter().enumerate() {
            debug_assert!(
                leaf_idx_map[terminal_stem_idx].is_none(),
                "Duplicate terminal stem index in mapped leaf_idx_map construction: stem_idx={} existing_leaf_idx={} new_leaf_idx={}",
                terminal_stem_idx,
                leaf_idx_map[terminal_stem_idx].unwrap(),
                leaf_idx
            );

            leaf_idx_map[terminal_stem_idx] = NonMaxUsize::new(leaf_idx);
        }

        OwnedStemLeafResolution::Mapped {
            min_stem_leaf_idx: 0,
            leaf_idx_map,
        }
    }

    fn terminal_stem_indices_match_arithmetic_layout(
        terminal_stem_indices: &[usize],
        actual_max_stem_level: i32,
    ) -> bool {
        let depth = (actual_max_stem_level + 1) as usize;

        for (leaf_idx, &terminal_stem_idx) in terminal_stem_indices.iter().enumerate() {
            let mut stem_ordering = SS::new_no_ptr();
            for bit_idx in (0..depth).rev() {
                let is_right = leaf_idx & (1 << bit_idx) != 0;
                stem_ordering.traverse::<A, K>(is_right);
            }

            if stem_ordering.stem_idx() != terminal_stem_idx {
                return false;
            }
        }

        true
    }

    fn calc_pivot(
        chunk_length: usize,
        _stem_index: usize,
        right_capacity: usize,
        bucket_limit_type: BucketLimitType,
    ) -> usize {
        let mut result = chunk_length
            .saturating_sub(right_capacity)
            .next_multiple_of(B)
            .min(chunk_length);

        // Treat this as the ideal split target: put at least B items on the left
        // whenever the chunk can support it.
        if chunk_length > 0 {
            let min_ideal_left = B.min(chunk_length);
            if result < min_ideal_left {
                result = min_ideal_left;
            }
        }

        // debug_assert!(
        //     result > 0 || chunk_length >= right_capacity,
        //     "Unexpectedly generated an initial pivot to split a slice at position 0 during construction (chunk length: {chunk_length}, right_capacity: {right_capacity})"
        // );

        let result = if bucket_limit_type == BucketLimitType::Hard
            && result >= chunk_length
            && result > B
        {
            let adjusted_result = chunk_length
                .saturating_sub(right_capacity.max(B))
                .next_multiple_of(B)
                .min(chunk_length);

            tracing::debug!(
                orig_pivot = %result,
                adjusted_pivot = %adjusted_result,
                %chunk_length,
                %right_capacity,
                "initial pivot calc would result in infinite recursion due to everything going in left but left being > B. Splitting finer"
            );

            adjusted_result
        } else {
            result
        };

        debug_assert!(
            result < chunk_length + 1,
            "Unexpectedly generated an initial pivot to split a slice at or beyond its end during construction (chunk length: {chunk_length}, right_capacity: {right_capacity})"
        );

        result
    }

    // #[cfg(not(feature = "unreliable_select_nth_unstable"))]
    #[cfg_attr(not(feature = "no_inline"), inline)]
    fn update_pivot<X, FA>(
        source: &[X],
        axis_at: &FA,
        sort_index: &mut [usize],
        dim: usize,
        init_pivot: usize,
    ) -> Result<usize, ConstructionError>
    where
        FA: Fn(&X, usize) -> A,
    {
        // TODO: this block might be faster by using a quickselect with a fat partition?
        //       we could then run that quickselect and subtract (fat partition length - 1)
        //       from the pivot, avoiding the need for the while loop.

        let mut pivot = init_pivot;

        // ensure the item whose index = pivot is in its correctly sorted position, and any
        // items that are equal to it are adjacent, according to our assumptions about the
        // behaviour of `select_nth_unstable_by` (See examples/check_select_nth_unstable.rs)
        sort_index.select_nth_unstable_by(pivot, |&ia, &ib| {
            A::cmp((*axis_at)(&source[ia], dim), (*axis_at)(&source[ib], dim))
        });

        // FORK DEVIATION (see ADAPTIVE_SPLIT.md): tighten the partition so that *no* left-hand
        // element equals the boundary value, not merely the one at `pivot - 1`.
        //
        // The nudge below walks contiguously, which relies on `select_nth_unstable_by` leaving
        // equal values adjacent to the pivot. It only promises a partition, so an element equal to
        // the boundary can sit anywhere in the left region. A cycling strategy tolerates that: it
        // stores the boundary coordinate verbatim, and such an element merely tests to the right
        // of a plane it sits left of, which the query's branch-and-bound recovers from. An
        // adaptive strategy cannot, because there is then no value at all satisfying
        // `max(left) < s <= min(right)` for it to store.
        //
        // One forward pass moves every non-equal element to the front, so `[0, pivot)` ends up
        // strictly below the boundary and the equal ones join the right region, which only
        // requires `>= boundary`. Driving `pivot` to zero is fine — the rightward-nudge branch
        // below already handles an empty left side.
        if SS::ADAPTIVE_SPLIT_DIM && pivot > 0 {
            let boundary = (*axis_at)(&source[sort_index[pivot]], dim);
            let mut write = 0usize;
            for read in 0..pivot {
                if A::cmp((*axis_at)(&source[sort_index[read]], dim), boundary)
                    != std::cmp::Ordering::Equal
                {
                    sort_index.swap(write, read);
                    write += 1;
                }
            }
            pivot = write;
        }

        // if the pivot straddles two values that are equal, keep nudging it left until they aren't
        while pivot > 0
            && (*axis_at)(&source[sort_index[pivot]], dim)
                == (*axis_at)(&source[sort_index[pivot - 1]], dim)
        {
            pivot -= 1;
        }

        // if we nudged it all the way to the left, reset and try nudging it rightwards from the
        // initial pivot point instead. This requires that the entire slice is sorted, rather than
        // just the left-hand side
        if pivot == 0 {
            pivot = init_pivot;

            sort_index.sort_unstable_by(|&ia, &ib| {
                A::cmp((*axis_at)(&source[ia], dim), (*axis_at)(&source[ib], dim))
            });

            while pivot + 1 < sort_index.len()
                && (*axis_at)(&source[sort_index[pivot]], dim)
                    == (*axis_at)(&source[sort_index[pivot + 1]], dim)
            {
                pivot += 1;
            }

            if pivot + 1 >= sort_index.len() {
                // if we end up here at the end of the slice, then the source slice is unsplittable
                // in this dimension due to all entries having the same value on the given dimension
                tracing::debug!(
                    slice_len = %sort_index.len(),
                    %dim,
                    "Slice unsplittable along dimension"
                );

                if LS::BUCKET_LIMIT_TYPE == BucketLimitType::Hard {
                    return Err(ConstructionError::UnsplittableBucket { split_dim: dim });
                }

                pivot = sort_index.len();
            } else {
                // Avoid empty-left splits: place the boundary after the run.
                pivot += 1;
                tracing::trace!(
                    slice_len = %sort_index.len(),
                    %dim,
                    %init_pivot,
                    shift = %(pivot - init_pivot),
                    %pivot,
                    "pivot shifted right"
                );
            }
        } else if pivot != init_pivot {
            tracing::trace!(
                slice_len = %sort_index.len(),
                %dim,
                %init_pivot,
                shift = %(init_pivot - pivot),
                %pivot,
                "pivot shifted left"
            );
        }

        Ok(pivot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dist::SquaredEuclidean;
    use crate::leaf_strategy::FlatVec;
    use crate::leaf_strategy::VecOfArenas;
    use crate::leaf_strategy::VecOfArrays;
    use crate::traits::kd_tree::KdTreeAccessor;
    use crate::{Eytzinger, EytzingerAdaptive};

    // ---- FORK DEVIATION: adaptive bulk construction (see ADAPTIVE_SPLIT.md) --------------
    //
    // These pin the invariant that makes storing an encoded split value safe:
    //
    //     max(left) < stored <= min(right)
    //
    // Break it and queries descend past points that are physically in the other leaf, with no
    // error raised — so "it built and the sizes look right" is not evidence of correctness. Every
    // test below therefore checks *retrievability*, not shape.

    type AdaptiveTree = KdTree<f32, u32, EytzingerAdaptive, VecOfArrays<f32, u32, 3, 32>, 3, 32>;
    type CyclingTree = KdTree<f32, u32, Eytzinger, VecOfArrays<f32, u32, 3, 32>, 3, 32>;

    /// Lattice geometry with heavy coordinate coincidence, which is what voxel-derived meshes
    /// look like and what makes boundary-duplicate splits the common case rather than an edge
    /// case. `step` is the lattice pitch; `origin` shifts the whole block so sign handling gets
    /// exercised.
    ///
    /// **The dimensions below are chosen so `n.div_ceil(B)` is a power of two**, which is not
    /// cosmetic: bulk construction trips the `left_capacity is zero` debug assertion for any
    /// other leaf count, until the capacity accounting was fixed — see
    /// [`bulk_build_handles_non_power_of_two_leaf_counts`], which now covers exactly that. The
    /// property is kept here anyway so these tests fail for encoding reasons alone rather than
    /// for a capacity reason that would be someone else's bug.
    fn lattice(nx: usize, ny: usize, nz: usize, step: f32, origin: [f32; 3]) -> Vec<[f32; 3]> {
        let mut points = Vec::with_capacity(nx * ny * nz);
        for ix in 0..nx {
            for iy in 0..ny {
                for iz in 0..nz {
                    points.push([
                        clear_dim_bits(origin[0] + ix as f32 * step),
                        clear_dim_bits(origin[1] + iy as f32 * step),
                        clear_dim_bits(origin[2] + iz as f32 * step),
                    ]);
                }
            }
        }
        points
    }

    /// Zero the bits the dimension encoding claims, moving the value by at most one grid cell.
    ///
    /// This is what makes the fixtures adversarial rather than merely realistic, and it is load
    /// bearing. Encoding dimension `d > 0` into such a value can only move it **upwards**, so
    /// every point sharing the boundary coordinate ends up on the far side of a plane that was
    /// encoded in place — which is precisely the bug. Left to whatever low bits `origin + k*step`
    /// happens to produce, the shift may go the other way and the defect hides: verified by
    /// mutation testing, where an un-quantised lattice let the naive implementation pass all of
    /// these tests.
    fn clear_dim_bits(value: f32) -> f32 {
        let mask = crate::stem_strategy::dim_packing::dim_mask(3) as u32;
        f32::from_bits(value.to_bits() & !mask)
    }

    fn brute_force_nearest(points: &[[f32; 3]], query: &[f32; 3]) -> f32 {
        points
            .iter()
            .map(|p| {
                (0..3)
                    .map(|d| (p[d] - query[d]) * (p[d] - query[d]))
                    .sum::<f32>()
            })
            .fold(f32::INFINITY, f32::min)
    }

    /// Querying at a point that is *in* the tree must find it at distance zero. This is the
    /// direct test for a plane that has drifted off its own partition: a stranded point is
    /// invisible to the descent that should reach it, so its own coordinates stop finding it.
    fn assert_every_point_is_retrievable(tree: &AdaptiveTree, points: &[[f32; 3]]) {
        for (idx, point) in points.iter().enumerate() {
            let found = tree
                .query(point)
                .nearest_one::<SquaredEuclidean<f32>>()
                .execute();
            assert_eq!(
                found.distance, 0.0,
                "point {idx} at {point:?} is in the tree but its own coordinates do not \
                 reach it (nearest was item {} at squared distance {}) - a split plane does \
                 not match the partition it was built from",
                found.item, found.distance,
            );
        }
    }

    #[test]
    fn adaptive_bulk_build_retrieves_every_point_on_lattice_geometry() {
        // 8 x 16 x 16 = 2,048 points over a bucket of 32, so 64 leaves and six levels of
        // splits, with at most 16 distinct values per axis: boundary duplicates everywhere.
        let points = lattice(8, 16, 16, 0.05, [1.65, -0.55, 3.85]);
        let tree = AdaptiveTree::new_from_slice(&points).expect("adaptive bulk build is supported");

        assert_eq!(tree.size(), points.len());
        assert_every_point_is_retrievable(&tree, &points);
    }

    #[test]
    fn adaptive_bulk_build_matches_brute_force_for_off_lattice_queries() {
        let points = lattice(8, 8, 16, 0.05, [-0.25, -0.25, -0.25]);
        let tree = AdaptiveTree::new_from_slice(&points).unwrap();

        // Queries deliberately off the lattice and outside it, so the descent has to pick the
        // correct side of planes rather than landing exactly on stored values.
        for i in 0..200u32 {
            let f = i as f32;
            let query = [
                (f * 0.7301).sin() * 0.9,
                (f * 1.1301).cos() * 0.9,
                (f * 0.3701).sin() * 0.9,
            ];
            let found = tree
                .query(&query)
                .nearest_one::<SquaredEuclidean<f32>>()
                .execute();
            let expected = brute_force_nearest(&points, &query);
            assert_eq!(
                found.distance, expected,
                "query {query:?} returned squared distance {} but brute force says {expected}",
                found.distance,
            );
        }
    }

    /// Coordinates spanning zero. IEEE 754 orders negative floats in reverse of their bit
    /// patterns, so any attempt to replace `grid_separator`'s value comparisons with bit
    /// arithmetic moves the separator the wrong way for negatives. This fails if that happens.
    #[test]
    fn adaptive_bulk_build_handles_coordinates_spanning_zero() {
        let points = lattice(8, 16, 16, 0.05, [-0.3, -0.3, -0.3]);
        assert!(
            points.iter().any(|p| p[0] < 0.0) && points.iter().any(|p| p[0] > 0.0),
            "the fixture must straddle zero for this test to mean anything"
        );

        let tree = AdaptiveTree::new_from_slice(&points).unwrap();
        assert_every_point_is_retrievable(&tree, &points);
    }

    /// The whole purpose of the encoding: a stem must report back the axis it was split on.
    /// Skip `encode_split` and the low bits hold whatever the data happened to carry, so the
    /// decoded dimension is wrong and queries compare the wrong coordinate against the plane.
    #[test]
    fn adaptive_bulk_build_records_the_split_dimension_in_every_written_stem() {
        use crate::stem_strategy::dim_packing::{dim_mask, unpack_from};

        let points = lattice(8, 16, 16, 0.05, [1.65, 2.0, 3.85]);
        let tree = AdaptiveTree::new_from_slice(&points).unwrap();

        // The axis is *chosen* by spread, not implied by depth, so there is no per-stem value to
        // predict. What must hold is that the stored values carry real axis information: with the
        // fixture's dimension bits cleared, an unencoded stem decodes as axis 0, so a tree built
        // without encoding shows axis 0 everywhere. A tree that genuinely records its choices
        // uses more than one, because the fixture is deliberately not cubic.
        let mut counts = [0usize; 3];
        for (stem_idx, stored) in tree.stems().iter().enumerate().skip(1) {
            if f32::is_max_value(*stored) {
                continue; // never written
            }
            let decoded = unsafe {
                unpack_from(
                    (stored as *const f32).cast(),
                    size_of::<f32>(),
                    dim_mask(3),
                    3,
                )
            };
            assert!(
                decoded < 3,
                "stem {stem_idx} decodes to axis {decoded}, which is not an axis of a 3-d tree"
            );
            counts[decoded] += 1;
        }

        let written: usize = counts.iter().sum();
        assert!(
            written >= 8,
            "expected the fixture to write a meaningful number of stems, only saw {written}"
        );
        let axes_used = counts.iter().filter(|c| **c > 0).count();
        assert!(
            axes_used >= 2,
            "every written stem decodes to the same axis (counts {counts:?}); the split \
             dimension is not being recorded in the stored value"
        );
    }

    /// Bulk and incremental construction are two different code paths that must agree, since the
    /// rebalance this unlocks swaps one for the other mid-life.
    #[test]
    fn adaptive_bulk_build_answers_identically_to_incremental_add() {
        let points = lattice(8, 8, 16, 0.05, [1.65, -0.55, 3.85]);

        let bulk = AdaptiveTree::new_from_slice(&points).unwrap();
        let mut incremental = AdaptiveTree::default();
        for (idx, point) in points.iter().enumerate() {
            incremental.add(point, idx as u32).unwrap();
        }

        assert_eq!(bulk.size(), incremental.size());
        for i in 0..200u32 {
            let f = i as f32;
            let query = [
                1.65 + (f * 0.7301).sin() * 0.8,
                -0.55 + (f * 1.1301).cos() * 0.8,
                3.85 + (f * 0.3701).sin() * 0.8,
            ];
            let from_bulk = bulk
                .query(&query)
                .nearest_one::<SquaredEuclidean<f32>>()
                .execute();
            let from_incremental = incremental
                .query(&query)
                .nearest_one::<SquaredEuclidean<f32>>()
                .execute();
            assert_eq!(
                from_bulk.distance, from_incremental.distance,
                "bulk and incremental disagree at {query:?}"
            );
        }
    }

    /// Radius queries walk the tree differently from nearest-neighbour, so they can expose a bad
    /// plane that `nearest_one` happens to survive. The returned set must be exactly the set
    /// brute force finds.
    #[test]
    fn adaptive_bulk_build_within_radius_matches_brute_force() {
        let points = lattice(8, 16, 16, 0.05, [1.65, -0.55, 3.85]);
        let tree = AdaptiveTree::new_from_slice(&points).unwrap();

        let radius = 0.12f32;
        for i in 0..40u32 {
            let f = i as f32;
            let query = [
                1.65 + (f * 0.31).sin() * 0.3,
                -0.55 + (f * 0.53).cos() * 0.3,
                3.85 + (f * 0.17).sin() * 0.3,
            ];
            let mut found: Vec<u32> = tree
                .query(&query)
                .within::<SquaredEuclidean<f32>>(radius * radius)
                .execute()
                .into_iter()
                .map(|n| n.item)
                .collect();
            found.sort_unstable();

            let mut expected: Vec<u32> = points
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    (0..3)
                        .map(|d| (p[d] - query[d]) * (p[d] - query[d]))
                        .sum::<f32>()
                        <= radius * radius
                })
                .map(|(idx, _)| idx as u32)
                .collect();
            expected.sort_unstable();

            assert_eq!(found, expected, "radius query at {query:?} returned the wrong set");
        }
    }

    /// The reason bulk construction is wanted at all: it must produce a near-balanced tree, since
    /// stems are addressed breadth-first and depth costs ~2^depth slots. A regression that
    /// reintroduces depth blow-up would still pass every correctness test above.
    #[test]
    fn adaptive_bulk_build_stays_near_balanced() {
        let points = lattice(16, 16, 16, 0.05, [1.65, -0.55, 3.85]);
        let tree = AdaptiveTree::new_from_slice(&points).unwrap();

        let balanced = (points.len().div_ceil(32) as f64).log2().ceil() as i32;
        assert!(
            tree.max_stem_level() <= balanced + 2,
            "bulk build reached depth {} where balanced is {balanced}; stems hold {} slots",
            tree.max_stem_level(),
            tree.stems().len(),
        );
    }


    /// Clustered geometry in a wide extent, emitted cluster by cluster so consecutive insertions
    /// are spatial neighbours. This is the shape of object-claimed mesh geometry and the order a
    /// vertex buffer hands you, and it is what drives depth in an incremental tree.
    fn coherent_clusters(count: usize, clusters: usize) -> Vec<[f32; 3]> {
        const STEP: f32 = 0.05;
        let per_cluster = count.div_ceil(clusters);
        let side = (per_cluster as f64).sqrt().ceil().max(1.0) as usize;

        let mut points = Vec::with_capacity(count);
        for cluster in 0..clusters {
            let f = cluster as f32;
            let origin = [
                (f * 12.9898).sin() * 10.0,
                (f * 78.2330).sin() * 8.5,
                (f * 37.7190).sin() * 1.5,
            ];
            for i in 0..per_cluster {
                if points.len() == count {
                    break;
                }
                points.push([
                    clear_dim_bits(origin[0] + (i % side) as f32 * STEP),
                    clear_dim_bits(origin[1] + (i / side) as f32 * STEP),
                    clear_dim_bits(origin[2]),
                ]);
            }
        }
        points
    }

    /// The point of the whole exercise: inserting in a coherent order must not run the stem array
    /// away. Asserted against plain `add` on the identical points, so the test states a difference
    /// rather than a magic number — it fails if the trigger never fires *or* if it fires without
    /// bounding anything.
    #[test]
    fn add_rebalancing_bounds_the_stem_array_on_coherent_insertion() {
        let points = coherent_clusters(6_000, 150);

        let mut plain = AdaptiveTree::default();
        for (idx, point) in points.iter().enumerate() {
            plain.add(point, idx as u32).expect("plain add");
        }

        let mut healed = AdaptiveTree::default();
        for (idx, point) in points.iter().enumerate() {
            healed
                .add_rebalancing(point, idx as u32)
                .expect("rebalancing add");
        }

        assert_eq!(healed.size(), points.len(), "rebalancing must not lose points");
        assert_every_point_is_retrievable(&healed, &points);

        let balanced_slots = 2 * points.len().div_ceil(32);
        assert!(
            healed.stems().len() < plain.stems().len(),
            "rebalancing gave {} stem slots against plain add's {}, so it bought nothing",
            healed.stems().len(),
            plain.stems().len(),
        );
        assert!(
            healed.stem_slot_ratio() <= AdaptiveTree::REBALANCE_SLOT_RATIO as f64,
            "ratio {} exceeds the threshold it is supposed to enforce (slots {}, balanced would \
             be about {balanced_slots})",
            healed.stem_slot_ratio(),
            healed.stems().len(),
        );
    }

    /// A rebuild is only useful if it is transparent: same points, same answers.
    #[test]
    fn rebalance_preserves_every_point_and_its_answers() {
        let points = coherent_clusters(4_000, 100);
        let mut tree = AdaptiveTree::default();
        for (idx, point) in points.iter().enumerate() {
            tree.add(point, idx as u32).expect("add");
        }

        let before: Vec<f32> = (0..100u32)
            .map(|i| {
                let f = i as f32;
                let query = [(f * 0.31).sin() * 9.0, (f * 0.53).cos() * 8.0, (f * 0.17).sin()];
                tree.query(&query)
                    .nearest_one::<SquaredEuclidean<f32>>()
                    .execute()
                    .distance
            })
            .collect();

        tree.rebalance().expect("rebalance");

        assert_eq!(tree.size(), points.len());
        assert_every_point_is_retrievable(&tree, &points);
        for (i, expected) in before.iter().enumerate() {
            let f = i as f32;
            let query = [(f * 0.31).sin() * 9.0, (f * 0.53).cos() * 8.0, (f * 0.17).sin()];
            let after = tree
                .query(&query)
                .nearest_one::<SquaredEuclidean<f32>>()
                .execute()
                .distance;
            assert_eq!(after, *expected, "rebalance changed the answer at {query:?}");
        }
    }

    /// A healthy tree must not be rebuilt: the trigger has to distinguish shape from size, or it
    /// would rebuild on every split and turn an out-of-memory into a hang.
    #[test]
    fn add_rebalancing_leaves_a_well_spread_tree_alone() {
        // A shuffled point set builds a balanced tree incrementally, so nothing should fire.
        let mut points = coherent_clusters(6_000, 150);
        shuffle_points(&mut points);

        let mut healed = AdaptiveTree::default();
        for (idx, point) in points.iter().enumerate() {
            healed.add_rebalancing(point, idx as u32).expect("add");
        }
        let mut plain = AdaptiveTree::default();
        for (idx, point) in points.iter().enumerate() {
            plain.add(point, idx as u32).expect("add");
        }

        assert_eq!(
            healed.stems().len(),
            plain.stems().len(),
            "a well-spread insertion order triggered a rebuild it did not need"
        );
    }

    /// `add_rebalancing` must never fail where `add` would have succeeded.
    ///
    /// Bulk construction can legitimately refuse a point set, so a rebuild is an optimisation that
    /// may not be available. Propagating that refusal made the office corpus fail to build at all
    /// — an index that merely wanted reshaping became less usable than one that never tried. The
    /// fixture mixes coherent clusters with near-coincident points, which is what pushes bulk
    /// construction into refusing; the corpus build is the broader regression test.
    #[test]
    fn add_rebalancing_never_fails_where_add_succeeds() {
        let mut points = coherent_clusters(3_000, 80);
        // Sprinkle in points a few ULP from an existing one, on every axis at once. A bucket of
        // these has no separating plane, which is the case that makes a rebuild impossible.
        let nudged: Vec<[f32; 3]> = points
            .iter()
            .take(600)
            .map(|p| {
                [
                    f32::from_bits(p[0].to_bits() + 1),
                    f32::from_bits(p[1].to_bits() + 2),
                    f32::from_bits(p[2].to_bits() + 1),
                ]
            })
            .collect();
        points.extend(nudged);

        let mut plain = AdaptiveTree::default();
        let mut healed = AdaptiveTree::default();
        for (idx, point) in points.iter().enumerate() {
            let plain_result = plain.add(point, idx as u32);
            let healed_result = healed.add_rebalancing(point, idx as u32);
            assert!(
                !(plain_result.is_ok() && healed_result.is_err()),
                "insert {idx} at {point:?} succeeded with add but failed with add_rebalancing: \
                 {healed_result:?}"
            );
            if plain_result.is_err() {
                break; // both hit a genuinely unsplittable bucket; nothing left to compare
            }
        }

        assert_eq!(
            healed.size(),
            plain.size(),
            "the two paths accepted different numbers of points"
        );
    }

    fn shuffle_points(points: &mut [[f32; 3]]) {
        let mut state: u64 = 0x2026_0727;
        for i in (1..points.len()).rev() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            points.swap(i, (state >> 33) as usize % (i + 1));
        }
    }

    /// `update_pivot` must leave a partition where **no** left-hand element equals the boundary
    /// value, not merely the one at `pivot - 1`.
    ///
    /// `select_nth_unstable_by` partitions but does not gather equal values next to the pivot, so
    /// the contiguous nudge can leave one behind. For a cycling strategy that is survivable; for
    /// an adaptive one there is then no value satisfying `max(left) < s <= min(right)`, and the
    /// chunk gets refused even though it is perfectly splittable. Removing the tightening makes
    /// the office corpus's 313,522 unclaimed vertices fail to bulk-build at all.
    ///
    /// Driven as a property over heavily duplicated values, because which element ends up where
    /// is an internal detail of the standard library's quickselect and cannot be dictated.
    #[test]
    fn update_pivot_leaves_no_left_element_equal_to_the_boundary() {
        fn next(state: &mut u64) -> u64 {
            *state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *state >> 33
        }

        let axis_at = |p: &[f32; 3], dim: usize| p[dim];
        let mut state: u64 = 0x5EED_2026;
        let mut exercised = 0usize;

        for _ in 0..3_000 {
            // Large enough to get real quickselect rather than a small-slice sort, and few
            // enough distinct values that duplicates straddle the pivot constantly.
            // A chunk of B+1 splits 128/1, so the pivot sits at the very edge. That asymmetry
            // is what puts a full bucket's worth of duplicates on the left of the boundary; a
            // symmetric len/2 pivot never reproduces it.
            let len = 129 + (next(&mut state) % 400) as usize;
            let distinct = 2 + (next(&mut state) % 40) as usize;
            let source: Vec<[f32; 3]> = (0..len)
                .map(|_| [(next(&mut state) % distinct as u64) as f32, 0.0, 0.0])
                .collect();
            let mut sort_index: Vec<usize> = (0..len).collect();

            let init_pivot = 1 + (next(&mut state) % (len as u64 - 1)) as usize;
            let Ok(pivot) =
                AdaptiveTree::update_pivot(&source, &axis_at, &mut sort_index, 0, init_pivot)
            else {
                continue; // legitimately unsplittable on this axis
            };
            if pivot == 0 || pivot >= len {
                continue; // nothing on one side, so there is no boundary to check
            }

            let boundary = source[sort_index[pivot]][0];
            let left_max = sort_index[..pivot]
                .iter()
                .map(|&i| source[i][0])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(
                left_max < boundary,
                "left partition reaches {left_max} but the boundary is {boundary}: an element \
                 equal to the boundary was left on the left-hand side (len {len}, {distinct} \
                 distinct values, pivot {pivot})"
            );
            exercised += 1;
        }

        assert!(
            exercised > 300,
            "only {exercised} of 3000 trials produced a two-sided split; the fixture is not \
             exercising the postcondition"
        );
    }

    /// The left bound handed to `grid_separator` must be the **true maximum** of the left
    /// partition, not merely the element sitting at `pivot - 1`.
    ///
    /// `grid_separator` always returns a value within one cell of `upper`, so on well-spread data
    /// a too-small left bound yields the same separator and nothing looks wrong — which is why
    /// the black-box tests above cannot see this. It matters exactly when some *other* left
    /// element lies within a cell of the boundary: the honest answer is then "no plane separates
    /// these", and a bound read off `pivot - 1` instead reports success with a plane that leaves
    /// that element on the wrong side.
    ///
    /// Driven directly rather than through a fixture, since `select_nth_unstable_by` cannot be
    /// asked to place a particular element at `pivot - 1`.
    #[test]
    fn split_value_requires_the_true_left_maximum() {
        let upper = 2.0f32; // mantissa zero, so the dimension bits are already clear
        let near = f32::from_bits(upper.to_bits() - 1); // one ULP below the boundary

        // Values live on axis 1, which is the axis passed below. Index 1 holds the true left
        // maximum; index 2 — the `pivot - 1` slot — holds something far below it.
        let source = [
            [0.0f32, 1.0, 0.0],
            [0.0, near, 0.0],
            [0.0, 1.5, 0.0],
            [0.0, upper, 0.0],
        ];
        let sort_index = [0usize, 1, 2, 3];
        let axis_at = |p: &[f32; 3], dim: usize| p[dim];

        let result = AdaptiveTree::split_value_for_stem(&source, &axis_at, &sort_index, 1, 3);

        assert!(
            matches!(
                result,
                Err(ConstructionError::UnsplittableBucket { split_dim: 1 })
            ),
            "a left element one ULP below the boundary leaves no separating plane, so this must \
             be refused; got {result:?}, which means the left bound was not the true maximum"
        );

        // Sanity: with that element removed the same split is perfectly fine, so the refusal
        // above is about the near-boundary value and not about the fixture being malformed.
        let sort_index_without_near = [0usize, 2, 3];
        let ok = AdaptiveTree::split_value_for_stem(&source, &axis_at, &sort_index_without_near, 1, 2);
        let separator = ok.expect("a well-spread left partition must produce a separator");
        assert!(
            separator > 1.5 && separator <= upper,
            "separator {separator} must satisfy max(left) < s <= min(right)"
        );
    }

    /// Bulk construction used to mis-account capacity whenever the leaf count was not a power of
    /// two, tripping `left_capacity is zero - should never happen` (16/32/64/128 leaves built fine
    /// while 54 and 94 panicked). It reproduced with the *cycling* strategy, so it was independent
    /// of adaptive split-value encoding, and it was a `debug_assert`, so release builds sailed past
    /// it — which is why bulk builds over the office corpus (324 leaves) looked healthy. Fixed by
    /// widening each child's capacity to the partition it actually receives.
    ///
    #[test]
    fn bulk_build_handles_non_power_of_two_leaf_counts() {
        // 1,728 points over a bucket of 32 is 54 leaves, which used to trip the assertion.
        for (nx, ny, nz) in [(12, 12, 12), (10, 10, 10), (7, 13, 11), (5, 5, 5)] {
            let points = lattice(nx, ny, nz, 0.05, [1.65, -0.55, 3.85]);
            CyclingTree::new_from_slice(&points)
                .unwrap_or_else(|e| panic!("cycling bulk build of {} points failed: {e}", points.len()));
            let adaptive = AdaptiveTree::new_from_slice(&points)
                .unwrap_or_else(|e| panic!("adaptive bulk build of {} points failed: {e}", points.len()));
            assert_every_point_is_retrievable(&adaptive, &points);
        }
    }

    /// Cycling strategies must be untouched by the deviation: they still store the boundary
    /// coordinate verbatim, with no bits stolen.
    #[test]
    fn cycling_bulk_build_stores_unencoded_coordinates() {
        let points = lattice(8, 16, 16, 0.05, [1.65, -0.55, 3.85]);
        let tree = CyclingTree::new_from_slice(&points).unwrap();

        let coordinates: std::collections::HashSet<u32> = points
            .iter()
            .flat_map(|p| p.iter().map(|v| v.to_bits()))
            .collect();
        let mut checked = 0usize;
        for stored in tree.stems().iter() {
            if f32::is_max_value(*stored) {
                continue;
            }
            assert!(
                coordinates.contains(&stored.to_bits()),
                "cycling stem holds {stored}, which is not one of the source coordinates - the \
                 adaptive encoding has leaked into the non-adaptive path"
            );
            checked += 1;
        }
        assert!(checked >= 8, "expected written stems, saw {checked}");
    }

    #[test]
    fn update_pivot_shifts_right_when_left_scan_hits_zero() {
        type TestTree = KdTree<f32, u32, Eytzinger, FlatVec<f32, u32, 2, 32>, 2, 32>;

        let source = [
            [1.0f32, 10.0],
            [1.0, 20.0],
            [1.0, 30.0],
            [2.0, 40.0],
            [3.0, 50.0],
        ];
        let mut sort_index = [0usize, 1, 2, 3, 4];

        let pivot = TestTree::update_pivot(
            &source,
            &|point: &[f32; 2], dim| point[dim],
            &mut sort_index,
            0,
            1,
        )
        .unwrap();

        assert_eq!(pivot, 3);
        assert_eq!(sort_index, [0, 1, 2, 3, 4]);
        assert_eq!(source[sort_index[pivot - 1]][0], 1.0);
        assert_eq!(source[sort_index[pivot]][0], 2.0);
    }

    #[test]
    fn replace_item_updates_flat_vec_tree_without_changing_size() {
        type TestTree = KdTree<f32, u32, Eytzinger, FlatVec<f32, u32, 2, 32>, 2, 32>;

        let entries = [
            (10u32, [1.0f32, 10.0]),
            (11u32, [2.0, 20.0]),
            (12u32, [1.0, 10.0]),
        ];
        let mut tree = TestTree::new_from_entries(&entries).unwrap();

        assert_eq!(tree.size(), 3);
        tree.replace_item(&[1.0, 10.0], 10, 99).unwrap();
        assert_eq!(tree.size(), 3);

        let iterated = tree.iter().collect::<Vec<_>>();
        assert_eq!(iterated[0], (99, [1.0, 10.0]));
        assert_eq!(iterated[1], (11, [2.0, 20.0]));
        assert_eq!(iterated[2], (12, [1.0, 10.0]));
    }

    #[test]
    fn replace_item_returns_entry_not_found_when_exact_match_is_missing() {
        type TestTree = KdTree<f32, u32, Eytzinger, VecOfArrays<f32, u32, 2, 32>, 2, 32>;

        let entries = [(10u32, [1.0f32, 10.0]), (11u32, [2.0, 20.0])];
        let mut tree = TestTree::new_from_entries(&entries).unwrap();

        assert_eq!(
            tree.replace_item(&[1.0, 10.0], 99, 100),
            Err(MutationError::EntryNotFound)
        );
        assert_eq!(
            tree.replace_item(&[9.0, 90.0], 10, 100),
            Err(MutationError::EntryNotFound)
        );
    }

    #[test]
    fn replace_item_updates_vec_of_arenas_tree() {
        type TestTree = KdTree<f64, u32, Eytzinger, VecOfArenas<f64, u32, 2, 32>, 2, 32>;

        let entries = [
            (20u32, [1.0f64, 10.0]),
            (21u32, [2.0, 20.0]),
            (22u32, [3.0, 30.0]),
        ];
        let mut tree = TestTree::new_from_entries(&entries).unwrap();

        tree.replace_item(&[2.0, 20.0], 21, 77).unwrap();

        let iterated = tree.iter().collect::<Vec<_>>();
        assert_eq!(
            iterated,
            vec![(20, [1.0, 10.0]), (77, [2.0, 20.0]), (22, [3.0, 30.0])]
        );
    }

    #[test]
    fn irregular_immutable_soft_layout_preserves_arithmetic_resolution() {
        type TestTree = KdTree<f32, u32, Eytzinger, FlatVec<f32, u32, 2, 2>, 2, 2>;

        let points = vec![
            [3.0, 0.0],
            [1.0, 0.6],
            [1.0, 1.4],
            [3.0, 3.3],
            [3.0, 3.8],
            [0.0, 1.8],
            [3.0, 1.5],
            [3.0, 2.7],
            [1.0, 3.3],
        ];
        let query = [2.9142656, 5.220647];

        let tree = TestTree::new_from_slice(&points).unwrap();
        assert!(tree.stem_leaf_resolution.uses_arithmetic());
        assert_eq!(tree.leaf_count(), 8);
        assert_eq!(tree.max_leaf_len(), 3);
        assert_eq!(
            (0..tree.leaf_count())
                .map(|leaf_idx| {
                    <FlatVec<f32, u32, 2, 2> as LeafStrategy<f32, u32, Eytzinger, 2, 2>>::leaf_len(
                        &tree.leaves,
                        leaf_idx,
                    )
                })
                .collect::<Vec<_>>(),
            vec![2, 0, 1, 1, 3, 0, 2, 0]
        );

        let result = tree
            .query(&query)
            .nearest_one::<SquaredEuclidean<f32>>()
            .execute();
        assert_eq!(result.item, 4);
        assert!((result.distance - 2.025588).abs() < 1.0e-6);
    }

    #[test]
    fn irregular_hard_terminal_layout_is_detected_and_mapped() {
        type TestTree = KdTree<f32, u32, Eytzinger, VecOfArrays<f32, u32, 2, 2>, 2, 2>;

        let terminal_stem_indices = vec![8usize, 10, 3];

        assert!(!TestTree::terminal_stem_indices_match_arithmetic_layout(
            &terminal_stem_indices,
            2,
        ));

        let stem_leaf_resolution =
            TestTree::mapped_stem_leaf_resolution_from_terminals(&terminal_stem_indices);
        assert!(!stem_leaf_resolution.uses_arithmetic());
        assert_eq!(stem_leaf_resolution.resolve_terminal_stem_idx(8, 0), 0);
        assert_eq!(stem_leaf_resolution.resolve_terminal_stem_idx(10, 0), 1);
        assert_eq!(stem_leaf_resolution.resolve_terminal_stem_idx(3, 0), 2);
    }

    #[test]
    fn unsplittable_immutable_hard_bucket_returns_error() {
        type TestTree = KdTree<f32, u32, Eytzinger, VecOfArrays<f32, u32, 2, 2>, 2, 2>;

        let points = vec![[1.0, 1.0], [1.0, 1.0], [1.0, 1.0]];

        assert!(matches!(
            TestTree::new_from_slice(&points),
            Err(ConstructionError::UnsplittableBucket { split_dim: 0 })
        ));
    }
}
