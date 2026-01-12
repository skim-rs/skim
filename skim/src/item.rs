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
    pub fn new(mut criterion: Vec<RankCriteria>) -> Self {
        if !criterion.contains(&RankCriteria::Score) && !criterion.contains(&RankCriteria::NegScore) {
            criterion.insert(0, RankCriteria::Score);
        }

        criterion.dedup();
        Self { criterion }
    }

    /// score: the greater the better
    pub fn build_rank(&self, score: i32, begin: usize, end: usize, length: usize, index: usize) -> Rank {
        let mut rank = [0; 5];
        let begin = begin as i32;
        let end = end as i32;
        let length = length as i32;
        let index = index as i32;

        for (priority, criteria) in self.criterion.iter().take(5).enumerate() {
            let value = match criteria {
                RankCriteria::Score => -score,
                RankCriteria::NegScore => score,
                RankCriteria::Begin => begin,
                RankCriteria::NegBegin => -begin,
                RankCriteria::End => end,
                RankCriteria::NegEnd => -end,
                RankCriteria::Length => length,
                RankCriteria::NegLength => -length,
                RankCriteria::Index => index,
                RankCriteria::NegIndex => -index,
            };

            rank[priority] = value;
        }

        trace!("ranks: {rank:?}");
        rank
    }
}

//------------------------------------------------------------------------------
/// An item that has been matched against a query
#[derive(Clone)]
pub struct MatchedItem {
    /// The underlying skim item
    pub item: Arc<dyn SkimItem>,
    /// The rank/score of this match
    pub rank: Rank,
    /// Range of characters that matched the pattern
    pub matched_range: Option<MatchRange>,
}

impl std::fmt::Debug for MatchedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatchedItem")
            .field("item", &self.item.text())
            .field("rank", &self.rank)
            .field("matched_range", &self.matched_range)
            .finish()
    }
}

impl Hash for MatchedItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.get_index());
        self.text().hash(state);
    }
}

impl Deref for MatchedItem {
    type Target = Arc<dyn SkimItem>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl MatchedItem {}

use std::cmp::Ordering as CmpOrd;

impl PartialEq for MatchedItem {
    fn eq(&self, other: &Self) -> bool {
        self.text().eq(&other.text()) && self.get_index().eq(&other.get_index())
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
        self.rank.cmp(&other.rank)
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
        }
    }
}

impl ItemPool {
    /// Creates a new empty item pool
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new item pool from skim options
    pub fn from_options(options: &crate::SkimOptions) -> Self {
        Self {
            length: AtomicUsize::new(0),
            pool: SpinLock::new(Vec::with_capacity(ITEM_POOL_CAPACITY)),
            taken: AtomicUsize::new(0),
            reserved_items: SpinLock::new(Vec::new()),
            lines_to_reserve: options.header_lines,
            tac: options.tac,
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

    /// append the items and return the new_size of the pool
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
                for item in remaining.into_iter() {
                    pool.insert(0, item);
                }
            } else {
                pool.extend(remaining);
            }
        } else if self.tac {
            // For --tac, prepend items (newest items go to front)
            for item in items.into_iter() {
                pool.insert(0, item);
            }
        } else {
            pool.extend(items);
        }
        self.length.store(pool.len(), Ordering::SeqCst);
        trace!("item pool, done append {len} items, total: {}", pool.len());
        pool.len()
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
}

#[cfg(feature = "cli")]
impl ValueEnum for RankCriteria {
    fn value_variants<'a>() -> &'a [Self] {
        use RankCriteria::*;
        &[
            Score, NegScore, Begin, NegBegin, End, NegEnd, Length, NegLength, Index, NegIndex,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        use RankCriteria::*;
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
        })
    }
}
