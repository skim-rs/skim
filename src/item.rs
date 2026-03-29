//! Item representation and management.
//!
//! This module provides the core item types used by skim, including ranked items,
//! item pools for efficient storage, and ranking criteria for sorting matches.
use std::cmp::min;
use std::default::Default;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "cli")]
use clap::ValueEnum;
#[cfg(feature = "cli")]
use clap::builder::PossibleValue;

use crate::spinlock::{SpinLock, SpinLockGuard};
use crate::{MatchRange, Rank, SkimItem};
use tokio::sync::Notify;

//------------------------------------------------------------------------------

/// Builder for creating rank values based on configurable criteria
#[derive(Debug)]
pub struct RankBuilder {
    criterion: Vec<RankCriteria>,
}

impl Default for RankBuilder {
    fn default() -> Self {
        Self {
            criterion: vec![RankCriteria::Score, RankCriteria::Begin, RankCriteria::End],
        }
    }
}

impl RankBuilder {
    /// Creates a new rank builder with the given criteria
    #[must_use]
    pub fn new(mut criterion: Vec<RankCriteria>) -> Self {
        if !criterion.contains(&RankCriteria::Score) && !criterion.contains(&RankCriteria::NegScore) {
            criterion.insert(0, RankCriteria::Score);
        }

        criterion.dedup();
        Self { criterion }
    }

    /// Returns the tiebreak criteria slice.
    #[must_use]
    pub fn criteria(&self) -> &[RankCriteria] {
        &self.criterion
    }

    /// Computes the byte offset of the first character after the last path separator
    /// (`/` or `\`) in `text`.  Returns `0` when no separator is present.
    fn path_name_offset(text: &str) -> i32 {
        text.rfind(['/', '\\'])
            .map_or(0, |pos| i32::try_from(pos).unwrap_or(i32::MAX).saturating_add(1))
    }

    /// Builds a `Rank` from raw match measurements.
    ///
    /// The values are stored as-is; the tiebreak ordering and sign-flipping are
    /// applied lazily by [`Rank::sort_key`] at comparison time.
    /// The `index` will be overridden later
    #[must_use]
    pub fn build_rank(&self, score: i32, begin: usize, end: usize, item_text: &str) -> Rank {
        Rank {
            score,
            begin: i32::try_from(begin).unwrap_or(i32::MAX),
            end: i32::try_from(end).unwrap_or(i32::MAX),
            length: i32::try_from(item_text.len()).unwrap_or(i32::MAX),
            index: Default::default(),
            path_name_offset: Self::path_name_offset(item_text),
        }
    }
}

impl Rank {
    /// Computes the ordered sort key for this rank given a slice of tiebreak criteria.
    ///
    /// Each criterion maps to one slot in the returned `[i32; 5]` array. Values are
    /// sign-flipped where necessary so that the array compares lexicographically with
    /// the "best" match sorting first (ascending order).
    #[must_use]
    pub fn sort_key(&self, criteria: &[RankCriteria]) -> [i32; 5] {
        let mut key = [0i32; 5];
        for (priority, criterion) in criteria.iter().take(5).enumerate() {
            key[priority] = match criterion {
                RankCriteria::Score => -self.score,
                RankCriteria::NegScore => self.score,
                RankCriteria::Begin => self.begin,
                RankCriteria::NegBegin => -self.begin,
                RankCriteria::End => self.end,
                RankCriteria::NegEnd => -self.end,
                RankCriteria::Length => self.length,
                RankCriteria::NegLength => -self.length,
                RankCriteria::Index => self.index,
                RankCriteria::NegIndex => -self.index,
                // PathName: prefer matches that fall within the filename portion (i.e. at or
                // after the last path separator).  `path_name_offset - begin` is <= 0 when the
                // match starts inside the filename, and positive when it starts in a directory
                // component.  Lower values sort first, so filename matches rank higher.
                RankCriteria::PathName => self.path_name_offset - self.begin,
                RankCriteria::NegPathName => self.begin - self.path_name_offset,
            };
        }
        key
    }
}

//------------------------------------------------------------------------------
/// An item that has been matched against a query
#[derive(Clone)]
pub struct MatchedItem {
    /// The underlying skim item
    pub item: Arc<dyn SkimItem>,
    /// Raw match measurements
    pub rank: Rank,
    /// The tiebreak criteria used to derive sort order from `rank`
    pub rank_builder: Arc<RankBuilder>,
    /// Range of characters that matched the pattern
    pub matched_range: Option<MatchRange>,
}

impl std::fmt::Debug for MatchedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatchedItem")
            .field("item", &self.item.text())
            .field("rank", &self.rank)
            .field("sort_key", &self.rank.sort_key(self.rank_builder.criteria()))
            .field("matched_range", &self.matched_range)
            .finish()
    }
}

impl Hash for MatchedItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_i32(self.rank.index);
        self.text().hash(state);
    }
}

impl Deref for MatchedItem {
    type Target = Arc<dyn SkimItem>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl MatchedItem {
    /// Merge two sorted `Vec<MatchedItem>` lists into one, preserving sort order by rank.
    ///
    /// Both input lists must already be sorted by the same tiebreak criteria (ascending).
    /// The merge is O(n+m).
    #[must_use]
    pub fn sorted_merge(existing: Vec<MatchedItem>, incoming: Vec<MatchedItem>) -> Vec<MatchedItem> {
        if existing.is_empty() {
            return incoming;
        }
        if incoming.is_empty() {
            return existing;
        }

        // Fast path: if all existing <= all incoming, we can append without merging.
        #[allow(clippy::missing_panics_doc)]
        if existing.last().unwrap() <= incoming.first().unwrap() {
            let mut out = existing;
            out.extend(incoming);
            return out;
        }

        // Fast path: if all incoming <= all existing, prepend without complex merge.
        #[allow(clippy::missing_panics_doc)]
        if incoming.last().unwrap() <= existing.first().unwrap() {
            let mut out = incoming;
            out.extend(existing);
            return out;
        }

        let mut merged = Vec::with_capacity(existing.len() + incoming.len());
        let mut a = existing.into_iter().peekable();
        let mut b = incoming.into_iter().peekable();

        loop {
            match (a.peek(), b.peek()) {
                (Some(av), Some(bv)) => {
                    if av <= bv {
                        #[allow(clippy::missing_panics_doc)]
                        merged.push(a.next().unwrap());
                    } else {
                        #[allow(clippy::missing_panics_doc)]
                        merged.push(b.next().unwrap());
                    }
                }
                (Some(_), None) => {
                    merged.extend(a);
                    break;
                }
                (None, _) => {
                    merged.extend(b);
                    break;
                }
            }
        }

        merged
    }

    /// Merge `incoming` into an already-sorted `existing` vector in-place.
    ///
    /// This function chooses between two strategies:
    /// - If `incoming` is small (few items), insert them one-by-one using binary
    ///   search to find the insertion point. This is O(m log n) for m incoming
    ///   items and is faster when m << n.
    /// - Otherwise, fall back to the linear two-way merge which is O(n+m).
    ///
    /// `existing` must be sorted according to the same ordering used by
    /// `MatchedItem::cmp`.
    pub fn merge_into_sorted(existing: &mut Vec<MatchedItem>, incoming: Vec<MatchedItem>) {
        // Heuristic threshold: for small incoming batches, prefer binary-insert.
        // This avoids allocating a new vector and copying the entire existing
        // list when we only need to insert a few new items.
        const SMALL_INSERT_THRESHOLD: usize = 256;

        if incoming.is_empty() {
            return;
        }

        if incoming.len() <= SMALL_INSERT_THRESHOLD {
            // Insert each incoming item into the existing sorted vector.
            // For small m this is typically faster than allocating a new
            // buffer and performing a full linear merge.
            for item in incoming {
                let pos = existing.binary_search_by(|e| e.cmp(&item)).unwrap_or_else(|p| p);
                existing.insert(pos, item);
            }
        } else {
            // For larger incoming batches, perform the linear two-way merge
            // which is O(n+m) and avoids the O(n*m) cost of repeated inserts.
            let old = std::mem::take(existing);
            *existing = MatchedItem::sorted_merge(old, incoming);
        }
    }
}

impl MatchedItem {
    /// Downcast the `MatchedItem` to the corresponding `SkimItem` struct
    #[must_use]
    pub fn downcast_item<T: SkimItem>(&self) -> Option<&T> {
        (*self.item).as_any().downcast_ref::<T>()
    }
}

use std::cmp::Ordering as CmpOrd;

impl PartialEq for MatchedItem {
    fn eq(&self, other: &Self) -> bool {
        self.text().eq(&other.text()) && self.rank.index.eq(&other.rank.index)
    }
}

impl std::cmp::Eq for MatchedItem {}

impl PartialOrd for MatchedItem {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrd> {
        Some(self.cmp(other))
    }
}

impl Ord for MatchedItem {
    fn cmp(&self, other: &Self) -> CmpOrd {
        let criteria = self.rank_builder.criteria();
        self.rank.sort_key(criteria).cmp(&other.rank.sort_key(criteria))
    }
}

//------------------------------------------------------------------------------
const ITEM_POOL_CAPACITY: usize = 16384;

/// Thread-safe pool for storing and managing items efficiently
pub struct ItemPool {
    /// Total number of items in the pool
    length: AtomicUsize,
    /// The main pool of items
    pool: SpinLock<Vec<Arc<dyn SkimItem>>>,
    /// Number of items that were taken
    taken: AtomicUsize,

    /// Reserved first N lines as header
    reserved_items: SpinLock<Vec<Arc<dyn SkimItem>>>,
    /// Number of lines to reserve as header
    lines_to_reserve: usize,
    /// Reverse the order of items (--tac flag)
    tac: bool,

    /// Notified whenever new items are appended to the pool (async path).
    ///
    /// Listeners (e.g. the TUI event loop) can `await` this to wake up
    /// immediately when items arrive instead of waiting for the next
    /// periodic tick.
    pub items_available: Arc<Notify>,
}

impl Default for ItemPool {
    fn default() -> Self {
        Self {
            length: AtomicUsize::new(0),
            pool: SpinLock::new(Vec::with_capacity(ITEM_POOL_CAPACITY)),
            taken: AtomicUsize::new(0),
            reserved_items: SpinLock::new(Vec::new()),
            lines_to_reserve: 0,
            tac: false,
            items_available: Arc::new(Notify::new()),
        }
    }
}

impl ItemPool {
    /// Creates a new empty item pool
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new item pool from skim options
    #[must_use]
    pub fn from_options(options: &crate::SkimOptions) -> Self {
        Self {
            length: AtomicUsize::new(0),
            pool: SpinLock::new(Vec::with_capacity(ITEM_POOL_CAPACITY)),
            taken: AtomicUsize::new(0),
            reserved_items: SpinLock::new(Vec::new()),
            lines_to_reserve: options.header_lines,
            tac: options.tac,
            items_available: Arc::new(Notify::new()),
        }
    }

    /// Returns the total number of items in the pool
    pub fn len(&self) -> usize {
        self.length.load(Ordering::SeqCst)
    }

    /// Returns true if the pool contains no items
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of items that have not been taken yet
    pub fn num_not_taken(&self) -> usize {
        self.length.load(Ordering::SeqCst) - self.taken.load(Ordering::SeqCst)
    }

    /// Returns the number of items that have been taken
    pub fn num_taken(&self) -> usize {
        self.taken.load(Ordering::SeqCst)
    }

    /// Clears all items from the pool and resets counters
    pub fn clear(&self) {
        let mut items = self.pool.lock();
        items.clear();
        let mut header_items = self.reserved_items.lock();
        header_items.clear();
        self.taken.store(0, Ordering::SeqCst);
        self.length.store(0, Ordering::SeqCst);
    }

    /// Resets the taken counter without clearing items
    pub fn reset(&self) {
        // lock to ensure consistency
        let _items = self.pool.lock();

        self.taken.store(0, Ordering::SeqCst);
    }

    /// append the items and return the `new_size` of the pool
    pub fn append(&self, mut items: Vec<Arc<dyn SkimItem>>) -> usize {
        let len = items.len();
        trace!("item pool, append {len} items");
        let mut pool = self.pool.lock();
        let mut header_items = self.reserved_items.lock();

        let to_reserve = self.lines_to_reserve - header_items.len();
        if to_reserve > 0 {
            let to_reserve = min(to_reserve, items.len());
            // Split items: first part goes to header, rest to main pool
            let remaining = items.split_off(to_reserve);

            // Header items are always in input order, regardless of tac
            header_items.extend(items);

            if self.tac {
                // For --tac, prepend non-header items (newest items go to front)
                for item in remaining {
                    pool.insert(0, item);
                }
            } else {
                pool.extend(remaining);
            }
        } else if self.tac {
            // For --tac, prepend items (newest items go to front)
            for item in items {
                pool.insert(0, item);
            }
        } else {
            pool.extend(items);
        }
        self.length.store(pool.len(), Ordering::SeqCst);
        trace!("item pool, done append {len} items, total: {}", pool.len());
        let new_len = pool.len();
        drop(pool);
        drop(header_items);
        // Wake any listener that is waiting for new items (e.g. the event loop
        // or the filter-mode loop) so it can restart the matcher immediately
        // instead of waiting for the next periodic tick.
        self.items_available.notify_one();

        new_len
    }

    /// Takes items from the pool, copying new items since last take and releasing lock immediately
    pub fn take(&self) -> Vec<Arc<dyn SkimItem>> {
        let guard = self.pool.lock();
        let taken = self.taken.swap(guard.len(), Ordering::SeqCst);
        // Copy the new items out so we can release the lock immediately
        let items = guard[taken..].to_vec();
        drop(guard); // Explicitly release lock
        items
    }

    /// Returns a copy of the reserved header items
    pub fn reserved(&self) -> Vec<Arc<dyn SkimItem>> {
        let guard = self.reserved_items.lock();
        guard.clone()
    }
}

/// Guard for accessing a slice of items from the pool
pub struct ItemPoolGuard<'a, T: Sized + 'a> {
    guard: SpinLockGuard<'a, Vec<T>>,
    start: usize,
}

impl<T: Sized> Deref for ItemPoolGuard<'_, T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.guard[self.start..]
    }
}

//------------------------------------------------------------------------------
/// Criteria for ranking and sorting matched items
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RankCriteria {
    /// Sort by match score (lower is better)
    Score,
    /// Sort by match score (higher is better)
    NegScore,
    /// Sort by beginning position of match
    Begin,
    /// Sort by beginning position of match (reversed)
    NegBegin,
    /// Sort by ending position of match
    End,
    /// Sort by ending position of match (reversed)
    NegEnd,
    /// Sort by item length
    Length,
    /// Sort by item length (reversed)
    NegLength,
    /// Sort by item index
    Index,
    /// Sort by item index (reversed)
    NegIndex,
    /// Give a bonus to matches that are after the last path separator (`/` or `\`)
    PathName,
    /// Give a bonus to matches that are after the last path separator (reversed)
    NegPathName,
}

#[cfg(feature = "cli")]
impl ValueEnum for RankCriteria {
    fn value_variants<'a>() -> &'a [Self] {
        use RankCriteria::{
            Begin, End, Index, Length, NegBegin, NegEnd, NegIndex, NegLength, NegPathName, NegScore, PathName, Score,
        };
        &[
            Score,
            NegScore,
            Begin,
            NegBegin,
            End,
            NegEnd,
            Length,
            NegLength,
            Index,
            NegIndex,
            PathName,
            NegPathName,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        use RankCriteria::{
            Begin, End, Index, Length, NegBegin, NegEnd, NegIndex, NegLength, NegPathName, NegScore, PathName, Score,
        };
        Some(match self {
            Score => PossibleValue::new("score"),
            Begin => PossibleValue::new("begin"),
            End => PossibleValue::new("end"),
            NegScore => PossibleValue::new("-score"),
            NegBegin => PossibleValue::new("-begin"),
            NegEnd => PossibleValue::new("-end"),
            Length => PossibleValue::new("length"),
            NegLength => PossibleValue::new("-length"),
            Index => PossibleValue::new("index"),
            NegIndex => PossibleValue::new("-index"),
            PathName => PossibleValue::new("pathname"),
            NegPathName => PossibleValue::new("-pathname"),
        })
    }
}
