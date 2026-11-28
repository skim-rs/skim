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
use crate::{MatchEngine, MatchResult, Rank, SkimItem};
use tokio::sync::Notify;

//------------------------------------------------------------------------------

/// Builder for creating rank values based on configurable criteria
#[derive(Debug)]
pub struct RankBuilder {
    criterion: Vec<RankCriteria>,
    tac: bool,
}

impl Default for RankBuilder {
    fn default() -> Self {
        Self {
            criterion: vec![RankCriteria::Score, RankCriteria::Begin, RankCriteria::End],
            tac: false,
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
        Self { criterion, tac: false }
    }

    #[must_use]
    pub(crate) fn tac(mut self, tac: bool) -> Self {
        self.tac = tac;
        self
    }

    /// Returns the tiebreak criteria slice.
    #[must_use]
    pub fn criteria(&self) -> &[RankCriteria] {
        &self.criterion
    }

    fn sort_key(&self, rank: &Rank) -> [i32; 6] {
        let configured = rank.sort_key(&self.criterion);
        let mut key = [0; 6];
        key[..5].copy_from_slice(&configured);

        if self.tac {
            for (priority, criterion) in self.criterion.iter().take(5).enumerate() {
                key[priority] = match criterion {
                    RankCriteria::Index => rank.index.saturating_neg(),
                    RankCriteria::NegIndex => rank.index,
                    _ => key[priority],
                };
            }
        }
        key[5] = if self.tac {
            rank.index.saturating_neg()
        } else {
            rank.index
        };
        key
    }

    /// Computes the **character** index of the first character after the last path
    /// separator (`/` or `\`) in `text`.  Returns `0` when no separator is present.
    ///
    /// This must be a char index, not a byte offset: the `PathName` tiebreak
    /// subtracts it from [`Rank::begin`], which is a char index, so counting bytes
    /// here would mix units and mis-rank any path with a non-ASCII component.
    fn path_name_offset(text: &str) -> i32 {
        text.rfind(['/', '\\']).map_or(0, |pos| {
            i32::try_from(text[..pos].chars().count())
                .unwrap_or(i32::MAX)
                .saturating_add(1)
        })
    }

    /// Order of the empty query results, where all ranks are equal but the
    /// index: `Some(true)` for highest index first, `Some(false)` for input
    /// order, `None` when the order also depends on the item text.
    #[must_use]
    pub(crate) fn empty_query_order(&self) -> Option<bool> {
        let uses_text = self.criterion.iter().any(|c| {
            matches!(
                c,
                RankCriteria::Length | RankCriteria::NegLength | RankCriteria::PathName | RankCriteria::NegPathName
            )
        });
        let key = |index| {
            self.sort_key(&Rank {
                index,
                ..Default::default()
            })
        };
        (!uses_text).then(|| key(1) < key(0))
    }

    /// Builds a `Rank` from raw match measurements.
    ///
    /// The values are stored as-is; the tiebreak ordering and sign-flipping are
    /// applied lazily by [`Rank::sort_key`] at comparison time.
    /// The `index` will be overridden later.
    /// `path_name_offset` is only computed (else `0`) when a `PathName` tiebreak
    /// uses it: it costs a scan of the text for every matched item.
    #[must_use]
    pub fn build_rank(&self, score: i32, begin: usize, end: usize, item_text: &str) -> Rank {
        let uses_path_name = self
            .criterion
            .iter()
            .any(|c| matches!(c, RankCriteria::PathName | RankCriteria::NegPathName));
        Rank {
            score,
            begin: i32::try_from(begin).unwrap_or(i32::MAX),
            end: i32::try_from(end).unwrap_or(i32::MAX),
            length: i32::try_from(item_text.len()).unwrap_or(i32::MAX),
            index: Default::default(),
            path_name_offset: if uses_path_name {
                Self::path_name_offset(item_text)
            } else {
                0
            },
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
/// A match in the result list, without a reference to the input item.
///
/// The last slot in the sort key is the tac-aware input index. It breaks ties
/// between distinct items ranked in the same run. Index and score also take
/// part in comparisons so that entries with equal keys from different runs
/// or criteria still have consistent equality and ordering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct MatchEntry {
    sort_key: [i32; 6],
    /// The zero-based index in the item pool.
    pub index: u32,
    /// The match score.
    pub score: i32,
}

const _: () = assert!(std::mem::size_of::<MatchEntry>() == 32);

impl MatchEntry {
    /// Builds an entry from a rank and the configured tiebreak criteria.
    ///
    /// # Panics
    ///
    /// Panics if the rank has a negative input index.
    #[must_use]
    pub fn new(rank: &Rank, rank_builder: &RankBuilder) -> Self {
        Self {
            sort_key: rank_builder.sort_key(rank),
            index: u32::try_from(rank.index).expect("input index must be nonnegative"),
            score: rank.score,
        }
    }

    /// Merges two lists already sorted by the same criteria in O(n + m) time.
    #[must_use]
    pub fn merge_sorted(mut existing: &[Self], mut incoming: &[Self]) -> Vec<Self> {
        let mut merged = Vec::with_capacity(existing.len() + incoming.len());
        if existing.last().zip(incoming.first()).is_none_or(|(a, b)| a <= b) {
            merged.extend_from_slice(existing);
            merged.extend_from_slice(incoming);
            return merged;
        }
        if incoming.last().zip(existing.first()).is_some_and(|(b, a)| b < a) {
            merged.extend_from_slice(incoming);
            merged.extend_from_slice(existing);
            return merged;
        }

        while let ([a, rest_a @ ..], [b, rest_b @ ..]) = (existing, incoming) {
            if a <= b {
                merged.push(*a);
                existing = rest_a;
            } else {
                merged.push(*b);
                incoming = rest_b;
            }
        }
        merged.extend_from_slice(existing);
        merged.extend_from_slice(incoming);
        merged
    }
}

/// An input item resolved from a match, for display, selection, or output.
#[derive(Clone)]
pub struct MatchedItem {
    /// The underlying skim item.
    pub item: Arc<dyn SkimItem>,
    /// The zero-based input index, not the position in the sorted list.
    pub index: usize,
    /// The match score.
    pub score: i32,
    engine: Option<Arc<dyn MatchEngine>>,
}

impl std::fmt::Debug for MatchedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatchedItem")
            .field("item", &self.item.text())
            .field("index", &self.index)
            .field("score", &self.score)
            .finish_non_exhaustive()
    }
}

impl Hash for MatchedItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
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
    /// Creates a matched item from its input index and match score, without a match engine.
    pub fn new(item: Arc<dyn SkimItem>, index: usize, score: i32) -> Self {
        Self::with_engine(item, index, score, None)
    }

    pub(crate) fn with_engine(
        item: Arc<dyn SkimItem>,
        index: usize,
        score: i32,
        engine: Option<Arc<dyn MatchEngine>>,
    ) -> Self {
        Self {
            item,
            index,
            score,
            engine,
        }
    }

    /// Recomputes the full rank with the match engine for this item.
    /// The rank uses this item's stored index and score. Use [`Self::match_result`]
    /// if you also need the matched range.
    ///
    /// Returns `None` if there is no match engine (for example, for items made with
    /// [`Self::new`]), the item no longer matches, or its index does not fit in a rank.
    #[must_use]
    pub fn rank(&self) -> Option<Rank> {
        Some(Rank {
            index: i32::try_from(self.index).ok()?,
            score: self.score,
            ..self.engine.as_ref()?.match_rank(self.item.as_ref())?
        })
    }

    /// Recomputes the full rank and matched range with the match engine for this item.
    /// The rank uses this item's stored index and score.
    ///
    /// Returns `None` under the same conditions as [`Self::rank`].
    #[must_use]
    pub fn match_result(&self) -> Option<MatchResult> {
        let index = i32::try_from(self.index).ok()?;
        let mut result = self.engine.as_ref()?.match_item(self.item.as_ref())?;
        result.rank.index = index;
        result.rank.score = self.score;
        Some(result)
    }

    /// Downcast the matched item to its concrete input type.
    #[must_use]
    pub fn downcast_item<T: SkimItem>(&self) -> Option<&T> {
        (*self.item).as_any().downcast_ref::<T>()
    }
}

impl PartialEq for MatchedItem {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.text() == other.text()
    }
}

impl Eq for MatchedItem {}

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

    /// Clears the pool and resets its counters. Returns the old non-header items in input order.
    pub fn clear(&self) -> Vec<Arc<dyn SkimItem>> {
        let mut items = self.pool.lock();
        let old = std::mem::take(&mut *items);
        let mut header_items = self.reserved_items.lock();
        let old_headers = std::mem::take(&mut *header_items);
        self.taken.store(0, Ordering::SeqCst);
        self.length.store(0, Ordering::SeqCst);
        drop(header_items);
        drop(items);
        drop(old_headers);
        old
    }

    /// Resets the taken counter without clearing items
    pub fn reset(&self) {
        // lock to ensure consistency
        let _items = self.pool.lock();

        self.taken.store(0, Ordering::SeqCst);
    }

    /// Appends items and returns the new size of the pool.
    ///
    /// # Panics
    ///
    /// Panics if the pool would contain more than `i32::MAX` non-header items.
    pub fn append(&self, mut items: Vec<Arc<dyn SkimItem>>) -> usize {
        let len = items.len();
        trace!("item pool, append {len} items");
        let mut pool = self.pool.lock();
        let mut header_items = self.reserved_items.lock();

        let to_reserve = min(self.lines_to_reserve - header_items.len(), len);
        assert!(
            len - to_reserve <= i32::MAX as usize - pool.len(),
            "item pool exceeds i32::MAX items"
        );
        if to_reserve > 0 {
            // Split items: first part goes to header, rest to main pool
            let remaining = items.split_off(to_reserve);

            // Header items are always in input order, regardless of tac
            header_items.extend(items);

            pool.extend(remaining);
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
        let mut items = guard[taken..].to_vec();
        if self.tac {
            items.reverse();
        }
        drop(guard); // Explicitly release lock
        items
    }

    /// Returns the item at `index` (its position in the input)
    pub fn get(&self, index: usize) -> Option<Arc<dyn SkimItem>> {
        self.pool.lock().get(index).cloned()
    }

    /// Borrows all items with the pool locked. The callback must not access the pool again.
    pub fn with_items<R>(&self, f: impl FnOnce(&[Arc<dyn SkimItem>]) -> R) -> R {
        f(&self.pool.lock())
    }

    /// Frees the spare capacity of the pool (it grows by doubling while items
    /// stream in). Call it once all items are read.
    pub fn shrink_to_fit(&self) {
        self.pool.lock().shrink_to_fit();
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

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
#[path = "item_tests.rs"]
mod tests;
