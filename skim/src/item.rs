//! An item is line of text that read from `find` command or stdin together with
//! the internal states, such as selected or not
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
#[derive(Clone)]
pub struct MatchedItem {
    pub item: Arc<dyn SkimItem>,
    pub rank: Rank,
    pub matched_range: Option<MatchRange>, // range of chars that matched the pattern
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

pub struct ItemPool {
    length: AtomicUsize,
    pool: SpinLock<Vec<Arc<dyn SkimItem>>>,
    /// number of items that was `take`n
    taken: AtomicUsize,

    /// reverse first N lines as header
    reserved_items: SpinLock<Vec<Arc<dyn SkimItem>>>,
    lines_to_reserve: usize,
}

impl Default for ItemPool {
    fn default() -> Self {
        Self {
            length: AtomicUsize::new(0),
            pool: SpinLock::new(Vec::with_capacity(ITEM_POOL_CAPACITY)),
            taken: AtomicUsize::new(0),
            reserved_items: SpinLock::new(Vec::new()),
            lines_to_reserve: 0,
        }
    }
}

impl ItemPool {
    pub fn new() -> Self {
        Self {
            length: AtomicUsize::new(0),
            pool: SpinLock::new(Vec::with_capacity(ITEM_POOL_CAPACITY)),
            taken: AtomicUsize::new(0),
            reserved_items: SpinLock::new(Vec::new()),
            lines_to_reserve: 0,
        }
    }

    pub fn lines_to_reserve(mut self, lines_to_reserve: usize) -> Self {
        self.lines_to_reserve = lines_to_reserve;
        self
    }

    pub fn len(&self) -> usize {
        self.length.load(Ordering::SeqCst)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn num_not_taken(&self) -> usize {
        self.length.load(Ordering::SeqCst) - self.taken.load(Ordering::SeqCst)
    }

    pub fn num_taken(&self) -> usize {
        self.taken.load(Ordering::SeqCst)
    }

    pub fn clear(&self) {
        let mut items = self.pool.lock();
        items.clear();
        let mut header_items = self.reserved_items.lock();
        header_items.clear();
        self.taken.store(0, Ordering::SeqCst);
        self.length.store(0, Ordering::SeqCst);
    }

    pub fn reset(&self) {
        // lock to ensure consistency
        debug!("reset");
        let _items = self.pool.lock();
        debug!("locked");
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
            // Reserve capacity to reduce reallocations
            header_items.reserve(to_reserve);
            header_items.extend_from_slice(&items[..to_reserve]);
            // Reserve capacity for remaining items
            pool.reserve(items.len() - to_reserve);
            pool.extend_from_slice(&items[to_reserve..]);
        } else {
            // Reserve capacity before appending
            pool.reserve(items.len());
            pool.append(&mut items);
        }
        self.length.store(pool.len(), Ordering::SeqCst);
        trace!("item pool, done append {len} items, total: {}", pool.len());
        pool.len()
    }

    pub fn take(&self) -> ItemPoolGuard<'_, Arc<dyn SkimItem>> {
        let guard = self.pool.lock();
        let taken = self.taken.swap(guard.len(), Ordering::SeqCst);
        ItemPoolGuard { guard, start: taken }
    }

    pub fn reserved(&self) -> ItemPoolGuard<'_, Arc<dyn SkimItem>> {
        let guard = self.reserved_items.lock();
        ItemPoolGuard { guard, start: 0 }
    }
}

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
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RankCriteria {
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
