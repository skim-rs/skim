//! Convenience re-exports of commonly used types.
//!
//! This module provides a convenient way to import all the commonly used
//! skim types and traits with a single `use skim::prelude::*;` statement.

pub use crate::engine::{
    factory::*,
    fuzzy::{FuzzyAlgorithm, FuzzyEngine},
};
pub use crate::fuzzy_matcher::skim::SkimMatcherV2;
pub use crate::helper::item_reader::{SkimItemReader, SkimItemReaderOption};
pub use crate::helper::selector::DefaultSkimSelector;
pub use crate::options::{SkimOptions, SkimOptionsBuilder};
pub use crate::output::SkimOutput;
pub use crate::reader::CommandCollector;
pub use crate::tui::{Event, PreviewCallback, event::Action};
pub use crate::*;
pub use kanal::{Receiver, Sender, bounded, unbounded};
pub use std::borrow::Cow;
pub use std::cell::RefCell;
pub use std::rc::Rc;
pub use std::sync::Arc;
pub use std::sync::atomic::{AtomicUsize, Ordering};
