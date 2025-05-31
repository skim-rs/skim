//!
//! ## Tuikit
//! Tuikit is a TUI library for writing terminal UI applications. Highlights:
//!
//! - Thread safe.
//! - Support non-fullscreen mode as well as fullscreen mode.
//! - Support `Alt` keys, mouse events, etc.
//! - Buffering for efficient rendering.
//!
//! Tuikit is modeld after [termbox](https://github.com/nsf/termbox) which views the
//! terminal as a table of fixed-size cells and input being a stream of structured
//! messages.
//!
//! ## Usage
//!
//! In your `Cargo.toml` add the following:
//!
//! ```toml
//! [dependencies]
//! tuikit = "*"
//! ```
//!
//! Here is an example:
//!
//! ```no_run
//! use tuikit::prelude::*;
//! use crossterm::event::{Event, KeyCode};
//! use std::cmp::{min, max};
//!
//! let term: Term<()> = Term::with_height(TermHeight::Percent(30)).unwrap();
//! let mut row = 1;
//! let mut col = 0;
//!
//! let _ = term.clear();
//! let _ = term.present();
//!
//! while let Ok(ev) = term.poll_event() {
//!     let _ = term.clear();
//!
//!     let (width, height) = term.term_size().unwrap();
//!     match ev {
//!         Event::Key(key_event) if key_event.code == KeyCode::Esc => break,
//!         Event::Key(key_event) if key_event.code == KeyCode::Char('q') => break,
//!         Event::Key(key_event) if key_event.code == KeyCode::Up => row = max(row-1, 1),
//!         Event::Key(key_event) if key_event.code == KeyCode::Down => row = min(row+1, height-1),
//!         Event::Key(key_event) if key_event.code == KeyCode::Left => col = max(col, 1)-1,
//!         Event::Key(key_event) if key_event.code == KeyCode::Right => col = min(col+1, width-1),
//!         _ => {}
//!     }
//!
//!     let _ = term.present();
//! }
//! ```
pub mod canvas;
pub mod cell;
mod color;
pub mod draw;
pub mod error;
pub mod event;
pub mod key;
mod macros;
pub mod prelude;
pub mod screen;
use common::spinlock;
mod sys;
pub mod term;
pub mod widget;

#[macro_use]
extern crate log;

use crate::error::TuikitError;

pub type Result<T> = std::result::Result<T, TuikitError>;
