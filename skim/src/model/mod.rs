pub(crate) mod options;
mod status;

use options::InfoDisplay;
use status::{ClearStrategy, Direction, Status};

use std::cmp::max;
use std::env;

use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};
use std::time::Instant;

use chrono::Duration as TimerDuration;
use defer_drop::DeferDrop;
use regex::Regex;
use skim_tuikit::prelude::{Event as TermEvent, *};
use timer::{Guard as TimerGuard, Timer};

use crate::engine::factory::{AndOrEngineFactory, ExactOrFuzzyEngineFactory, RegexEngineFactory};
use crate::event::{Event, EventHandler, EventReceiver, EventSender};
use crate::global::current_run_num;
use crate::header::Header;
use crate::helper::item::DefaultSkimItem;
use crate::input::parse_action_arg;
use crate::item::{ItemPool, MatchedItem, RankBuilder};
use crate::matcher::{Matcher, MatcherControl};
use crate::options::SkimOptions;
use crate::output::SkimOutput;
use crate::previewer::{PreviewSource, Previewer};
use crate::query::Query;
use crate::reader::{Reader, ReaderControl};
use crate::selection::Selection;
use crate::spinlock::SpinLock;
use crate::theme::ColorTheme;
use crate::util::{InjectContext, depends_on_items, inject_command, margin_string_to_size, parse_margin};
use crate::{FuzzyAlgorithm, MatchEngineFactory, MatchRange, SkimItem};

const REFRESH_DURATION: i64 = 100;

static RE_PREVIEW_OFFSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\+([0-9]+|\{-?[0-9]+\})(-[0-9]+|-/[1-9][0-9]*)?$").unwrap());

struct ModelEnv {
    pub cmd: String,
    pub query: String,
    pub cmd_query: String,
    pub clear_selection: ClearStrategy,
    pub in_query_mode: bool,
}

pub struct Model {
    reader: Reader,
    query: Query,
    selection: Selection,
    num_options: usize,
    select_1: bool,
    exit_0: bool,
    sync: bool,

    use_regex: bool,
    regex_matcher: Matcher,
    matcher: Matcher,

    term: Arc<Term>,

    item_pool: Arc<DeferDrop<ItemPool>>,

    rx: EventReceiver,
    tx: EventSender,

    fuzzy_algorithm: FuzzyAlgorithm,
    reader_timer: Instant,
    matcher_timer: Instant,
    reader_control: Option<ReaderControl>,
    matcher_control: Option<MatcherControl>,

    header: Header,

    preview_hidden: bool,
    previewer: Option<Previewer>,
    preview_direction: Direction,
    preview_size: Size,

    margin_top: Size,
    margin_right: Size,
    margin_bottom: Size,
    margin_left: Size,

    layout: String,
    delimiter: Regex,
    info: InfoDisplay,
    no_clear_if_empty: bool,
    theme: Arc<ColorTheme>,

    // Minimum query length to show results
    min_query_length: Option<usize>,

    // timer thread for scheduled events
    timer: Timer,
    hb_timer_guard: Option<TimerGuard>,

    // for AppendAndSelect action
    rank_builder: Arc<RankBuilder>,
}

impl Model {
    pub fn new(rx: EventReceiver, tx: EventSender, reader: Reader, term: Arc<Term>, options: &SkimOptions) -> Self {
        let default_command = match env::var("SKIM_DEFAULT_COMMAND").as_ref().map(String::as_ref) {
            Ok("") | Err(_) => "find .".to_owned(),
            Ok(val) => val.to_owned(),
        };

        let theme = Arc::new(ColorTheme::init_from_options(options));
        let query = Query::from_options(options)
            .replace_base_cmd_if_not_set(&default_command)
            .theme(theme.clone())
            .build();

        let rank_builder = Arc::new(RankBuilder::new(options.tiebreak.clone()));

        let selection = Selection::with_options(options).theme(theme.clone());
        let regex_engine: Rc<dyn MatchEngineFactory> =
            Rc::new(RegexEngineFactory::builder().rank_builder(rank_builder.clone()).build());
        let regex_matcher = Matcher::builder(regex_engine).build();

        let fuzzy_engine_factory: Rc<dyn MatchEngineFactory> = Rc::new(AndOrEngineFactory::new(
            ExactOrFuzzyEngineFactory::builder()
                .exact_mode(options.exact)
                .rank_builder(rank_builder.clone())
                .build(),
        ));
        let matcher = Matcher::builder(fuzzy_engine_factory).case(options.case).build();

        let item_pool = Arc::new(DeferDrop::new(ItemPool::new().lines_to_reserve(options.header_lines)));
        let header = Header::empty()
            .with_options(options)
            .item_pool(item_pool.clone())
            .theme(theme.clone());

        let margins = parse_margin(&options.margin);
        let (margin_top, margin_right, margin_bottom, margin_left) = margins;

        let mut ret = Model {
            reader,
            query,
            selection,
            num_options: 0,
            select_1: false,
            exit_0: false,
            sync: false,
            use_regex: options.regex,
            regex_matcher,
            matcher,
            term,
            item_pool,

            rx,
            tx,
            reader_timer: Instant::now(),
            matcher_timer: Instant::now(),
            reader_control: None,
            matcher_control: None,
            fuzzy_algorithm: FuzzyAlgorithm::default(),

            header,
            preview_hidden: true,
            previewer: None,
            preview_direction: Direction::Right,
            preview_size: Size::Default,

            margin_top,
            margin_right,
            margin_bottom,
            margin_left,

            layout: "default".to_string(),
            delimiter: Regex::new(r"[\t\n ]+").unwrap(),
            info: InfoDisplay::Default,
            no_clear_if_empty: false,
            theme,
            min_query_length: options.min_query_length,
            timer: Timer::new(),
            hb_timer_guard: None,

            rank_builder,
        };
        ret.parse_options(options);
        ret
    }

    fn parse_options(&mut self, options: &SkimOptions) {
        let Ok(delimiter) = Regex::new(&options.delimiter) else {
            panic!("Could not parse delimiter {} as a valid regex", options.delimiter);
        };
        self.delimiter = delimiter;

        self.layout = options.layout.clone();

        self.info = if options.inline_info {
            InfoDisplay::Inline
        } else if options.no_info {
            InfoDisplay::Hidden
        } else {
            options.info.clone()
        };

        self.use_regex = options.regex;

        self.fuzzy_algorithm = options.algorithm;

        // preview related
        let (preview_direction, preview_size, preview_wrap, preview_shown) =
            Self::parse_preview(options.preview_window.clone());
        self.preview_direction = preview_direction;
        self.preview_size = preview_size;
        self.preview_hidden = !preview_shown;

        let preview_source = if let Some(cmd) = &options.preview {
            PreviewSource::Command(cmd.to_string())
        } else if let Some(ref callback) = options.preview_fn {
            PreviewSource::Callback(callback.clone())
        } else {
            PreviewSource::Empty
        };

        let tx = Arc::new(SpinLock::new(self.tx.clone()));
        if !matches!(preview_source, PreviewSource::Empty) {
            self.previewer = Some(
                Previewer::new(preview_source, move || {
                    let _ = tx.lock().send((Key::Null, Event::EvHeartBeat));
                })
                .wrap(preview_wrap)
                .delimiter(self.delimiter.clone())
                .preview_offset(Self::parse_preview_offset(options.preview_window.clone())),
            );
        }

        self.select_1 = options.select_1;
        self.exit_0 = options.exit_0;

        self.sync = options.sync;
        self.no_clear_if_empty = options.no_clear_if_empty;
    }

    // -> (direction, size, wrap, shown)
    fn parse_preview(preview_option: String) -> (Direction, Size, bool, bool) {
        let options = preview_option.split(':').collect::<Vec<&str>>();

        let mut direction = Direction::Right;
        let mut shown = true;
        let mut wrap = false;
        let mut size = Size::Percent(50);

        for option in options {
            // mistake
            if option.is_empty() {
                continue;
            }

            let first_char = option.chars().next().unwrap_or('A');

            // raw string
            if first_char.is_ascii_digit() {
                size = margin_string_to_size(option);
            } else {
                match option.to_uppercase().as_str() {
                    "UP" => direction = Direction::Up,
                    "DOWN" => direction = Direction::Down,
                    "LEFT" => direction = Direction::Left,
                    "RIGHT" => direction = Direction::Right,
                    "HIDDEN" => shown = false,
                    "WRAP" => wrap = true,
                    _ => {}
                }
            }
        }

        (direction, size, wrap, shown)
    }

    // -> string
    fn parse_preview_offset(preview_window: String) -> String {
        for token in preview_window.split(':').rev() {
            if RE_PREVIEW_OFFSET.is_match(token) {
                return token.to_string();
            }
        }

        String::new()
    }

    fn act_heart_beat(&mut self, env: &mut ModelEnv) {
        // Check if query meets minimum length requirement
        if let Some(min_length) = self.min_query_length {
            // In normal mode, check query length
            // In interactive mode, check cmd_query length
            let query_to_check = if env.in_query_mode { &env.query } else { &env.cmd_query };

            if query_to_check.chars().count() < min_length {
                // Clear selection if query is too short
                self.selection.clear();
                // Exit early to prevent showing any results
                return;
            }
        }

        // save the processed items
        let matcher_stopped = self
            .matcher_control
            .as_ref()
            .map(|ctrl| ctrl.stopped())
            .unwrap_or(false);

        if matcher_stopped {
            let reader_stopped = self.reader_control.as_ref().map(ReaderControl::is_done).unwrap_or(true);
            let ctrl = self.matcher_control.take().unwrap();
            let lock = ctrl.into_items();
            let mut items = lock.lock();
            let matched = std::mem::take(&mut *items);

            match env.clear_selection {
                ClearStrategy::DontClear => {}
                ClearStrategy::Clear => {
                    self.selection.clear();
                    env.clear_selection = ClearStrategy::DontClear;
                }
                ClearStrategy::ClearIfNotNull => {
                    if (!self.no_clear_if_empty && reader_stopped) || !matched.is_empty() {
                        self.selection.clear();
                        env.clear_selection = ClearStrategy::DontClear;
                    }
                }
            };
            self.num_options += matched.len();
            self.selection.append_sorted_items(matched);
        }

        let items_consumed = self.item_pool.num_not_taken() == 0;
        let reader_stopped = self.reader_control.as_ref().map(|c| c.is_done()).unwrap_or(true);
        let processed = reader_stopped && items_consumed;

        // run matcher if matcher had been stopped and reader had new items.
        if !processed && self.matcher_control.is_none() {
            self.restart_matcher();
        }

        // send next heart beat if matcher is still running or there are items not been processed.
        if self.matcher_control.is_some() || !processed {
            let tx = self.tx.clone();
            let hb_timer_guard =
                self.timer
                    .schedule_with_delay(TimerDuration::milliseconds(REFRESH_DURATION), move || {
                        let _ = tx.send((Key::Null, Event::EvHeartBeat));
                    });
            self.hb_timer_guard.replace(hb_timer_guard);
        }
    }

    fn act_rotate_mode(&mut self, env: &mut ModelEnv) {
        self.use_regex = !self.use_regex;

        // restart matcher
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
        }

        env.clear_selection = ClearStrategy::Clear;
        self.item_pool.reset();
        self.num_options = 0;
        self.restart_matcher();
    }

    fn handle_select1_or_exit0(&mut self) {
        if !self.select_1 && !self.exit_0 && !self.sync {
            return;
        }

        let items_consumed = self.item_pool.num_not_taken() == 0;
        let reader_stopped = self.reader_control.as_ref().map(|c| c.is_done()).unwrap_or(true);
        let matcher_stopped = self.matcher_control.as_ref().map(|ctrl| ctrl.stopped()).unwrap_or(true);

        let processed = reader_stopped && items_consumed && matcher_stopped;
        let num_matched = self.selection.get_num_options();
        if processed {
            if num_matched == 1 && self.select_1 {
                debug!("select-1 triggered, accept");
                let _ = self.tx.send((Key::Null, Event::EvActAccept(None)));
            } else if num_matched == 0 && self.exit_0 {
                debug!("exit-0 triggered, accept");
                let _ = self.tx.send((Key::Null, Event::EvActAbort));
            } else {
                // no longer need need to handle select-1, exit-1, sync, etc.
                self.select_1 = false;
                self.exit_0 = false;
                self.sync = false;
                let _ = self.term.restart();
            }
        }
    }

    fn on_cmd_query_change(&mut self, env: &mut ModelEnv) {
        // stop matcher
        if let Some(ctrl) = self.reader_control.take() {
            ctrl.kill();
        }
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
        }

        env.clear_selection = ClearStrategy::ClearIfNotNull;
        self.item_pool.clear();
        self.num_options = 0;

        // restart reader
        self.reader_control.replace(self.reader.run(&env.cmd));

        // Check if query meets minimum length requirement before restarting matcher
        // In interactive mode, the command query is used as the search query
        if let Some(min_length) = self.min_query_length {
            if env.cmd_query.chars().count() < min_length {
                // Clear selection if query is too short
                self.selection.clear();
                // Don't restart matcher if query is too short
                return;
            }
        }

        self.restart_matcher();
        self.reader_timer = Instant::now();
    }

    fn on_query_change(&mut self, env: &mut ModelEnv) {
        // restart matcher
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
        }

        // Always clear selection when query changes
        env.clear_selection = ClearStrategy::Clear;
        self.item_pool.reset();
        self.num_options = 0;

        // Check if query meets minimum length requirement
        if let Some(min_length) = self.min_query_length {
            if env.query.chars().count() < min_length {
                // Clear selection if query is too short
                self.selection.clear();
                // Don't restart matcher if query is too short
                return;
            }
        }

        self.restart_matcher();
    }

    fn act_execute(&mut self, cmd: &str) {
        let item = self.selection.get_current_item();
        if depends_on_items(cmd) && item.is_none() {
            debug!("act_execute: command refers to items and there is no item for now");
            debug!("command to execute: [{}]", cmd);
            return;
        }

        let _ = self.term.pause();
        self.act_execute_silent(cmd);
        let _ = self.term.restart();
    }

    fn act_execute_silent(&mut self, cmd: &str) {
        let current_index = self.selection.get_current_item_idx();
        let current_item = self.selection.get_current_item();
        if depends_on_items(cmd) && current_item.is_none() {
            debug!("act_execute_silent: command refers to items and there is no item for now");
            debug!("command to execute: [{}]", cmd);
            return;
        }

        let current_selection = current_item.as_ref().map(|item| item.output()).unwrap_or_default();
        let query = self.query.get_fz_query();
        let cmd_query = self.query.get_cmd_query();

        let (indices, selections) = self.selection.get_selected_indices_and_items();
        let tmp: Vec<String> = selections.into_iter().map(|item| item.text().to_string()).collect();
        let selected_texts: Vec<&str> = tmp.iter().map(|cow| cow.as_ref()).collect();

        let context = InjectContext {
            current_index,
            delimiter: &self.delimiter,
            current_selection: &current_selection,
            selections: &selected_texts,
            indices: &indices,
            query: &query,
            cmd_query: &cmd_query,
        };

        let cmd = inject_command(cmd, context).to_string();
        let shell = env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
        let _ = Command::new(shell).arg("-c").arg(cmd).status();
    }

    fn act_reload(&mut self, cmd_opt: Option<String>) {
        let cmd = match cmd_opt {
            Some(s) => s,
            None => self.query.get_cmd(),
        };
        debug!("command to execute: [{}]", cmd);
        let mut env = ModelEnv {
            cmd: cmd.to_string(),
            cmd_query: self.query.get_cmd_query(),
            query: self.query.get_fz_query(),
            clear_selection: ClearStrategy::ClearIfNotNull,
            in_query_mode: self.query.in_query_mode(),
        };

        self.selection.clear();
        self.on_cmd_query_change(&mut env);
    }

    #[allow(clippy::trivial_regex)]
    fn act_append_and_select(&mut self, env: &mut ModelEnv) {
        let query = self.query.get_fz_query();
        if query.is_empty() {
            return;
        }

        let item_len = query.len();
        let item_idx = self.item_pool.len();
        let query_item = DefaultSkimItem::new(query, true, &[], &[], &self.delimiter, item_idx);
        let item: Arc<dyn SkimItem> = Arc::new(query_item);
        let new_len = self.item_pool.append(vec![item.clone()]);
        trace!(
            "appended and selected item with internal id {} and matched as id {}",
            item_idx,
            max(new_len, 1) - 1
        );
        let matched_item = MatchedItem {
            item,
            rank: self.rank_builder.build_rank(0, 0, 0, item_len, item_idx),
            matched_range: Some(MatchRange::ByteRange(0, 0)),
            item_idx: (max(new_len, 1) - 1) as u32,
        };

        self.selection.act_select_matched(current_run_num(), matched_item);

        self.act_heart_beat(env);
    }

    pub fn start(&mut self) -> Option<SkimOutput> {
        let mut env = ModelEnv {
            cmd: self.query.get_cmd(),
            query: self.query.get_fz_query(),
            cmd_query: self.query.get_cmd_query(),
            in_query_mode: self.query.in_query_mode(),
            clear_selection: ClearStrategy::DontClear,
        };

        self.reader_control = Some(self.reader.run(&env.cmd));

        // In the event loop, there might need
        let mut next_event = Some((Key::Null, Event::EvHeartBeat));
        loop {
            let (key, ev) = next_event.take().or_else(|| self.rx.recv().ok())?;

            debug!("handle event: {:?}", ev);

            match ev {
                Event::EvHeartBeat => {
                    // consume following HeartBeat event
                    next_event = self.consume_additional_event(&Event::EvHeartBeat);
                    self.act_heart_beat(&mut env);
                    self.handle_select1_or_exit0();
                }

                Event::EvActIfNonMatched(ref arg_str) => {
                    let matched =
                        self.num_options + self.matcher_control.as_ref().map(|c| c.get_num_matched()).unwrap_or(0);
                    if matched == 0 {
                        next_event = parse_action_arg(arg_str).map(|ev| (key, ev));
                        continue;
                    }
                }

                Event::EvActIfQueryEmpty(ref arg_str) => {
                    if env.query.is_empty() {
                        next_event = parse_action_arg(arg_str).map(|ev| (key, ev));
                        continue;
                    }
                }

                Event::EvActIfQueryNotEmpty(ref arg_str) => {
                    if !env.query.is_empty() {
                        next_event = parse_action_arg(arg_str).map(|ev| (key, ev));
                        continue;
                    }
                }

                Event::EvActTogglePreview => {
                    self.preview_hidden = !self.preview_hidden;
                }

                Event::EvActRotateMode => {
                    self.act_rotate_mode(&mut env);
                }

                Event::EvActAccept(accept_key) => {
                    if let Some(ctrl) = self.reader_control.take() {
                        ctrl.kill();
                    }
                    if let Some(ctrl) = self.matcher_control.take() {
                        ctrl.kill();
                    }

                    return Some(SkimOutput {
                        is_abort: false,
                        final_event: Event::EvActAccept(accept_key),
                        final_key: key,
                        query: self.query.get_fz_query(),
                        cmd: self.query.get_cmd_query(),
                        selected_items: self.selection.get_selected_indices_and_items().1,
                    });
                }

                Event::EvActAbort => {
                    if let Some(ctrl) = self.reader_control.take() {
                        ctrl.kill();
                    }
                    if let Some(ctrl) = self.matcher_control.take() {
                        ctrl.kill();
                    }

                    return Some(SkimOutput {
                        is_abort: true,
                        final_event: ev.clone(),
                        final_key: key,
                        query: self.query.get_fz_query(),
                        cmd: self.query.get_cmd_query(),
                        selected_items: self.selection.get_selected_indices_and_items().1,
                    });
                }

                Event::EvActDeleteCharEOF => {
                    if env.in_query_mode && env.query.is_empty() || !env.in_query_mode && env.cmd_query.is_empty() {
                        next_event = Some((key, Event::EvActAbort));
                        continue;
                    }
                }

                Event::EvActExecute(ref cmd) => {
                    self.act_execute(cmd);
                }

                Event::EvActExecuteSilent(ref cmd) => {
                    self.act_execute_silent(cmd);
                }

                Event::EvActReload(ref cmd) => {
                    self.act_reload(cmd.clone());
                }

                Event::EvActAppendAndSelect => {
                    self.act_append_and_select(&mut env);
                }

                Event::EvInputKey(key) => {
                    // dispatch key(normally the mouse keys) to sub-widgets
                    self.do_with_widget(|root| {
                        let (width, height) = self.term.term_size().unwrap();
                        let rect = Rectangle {
                            top: 0,
                            left: 0,
                            width,
                            height,
                        };
                        let messages = root.on_event(TermEvent::Key(key), rect);
                        for message in messages {
                            let _ = self.tx.send((key, message));
                        }
                    })
                }

                Event::EvActRefreshCmd => {
                    self.on_cmd_query_change(&mut env);
                }

                Event::EvActRefreshPreview => {
                    self.draw_preview(&env, true);
                }

                _ => {}
            }

            // dispatch events to sub-components

            self.header.handle(&ev);

            self.query.handle(&ev);
            env.cmd_query = self.query.get_cmd_query();

            let new_query = self.query.get_fz_query();
            let new_cmd = self.query.get_cmd();

            // re-run reader & matcher if needed;
            if new_cmd != env.cmd {
                env.cmd = new_cmd;
                self.on_cmd_query_change(&mut env);
            } else if new_query != env.query {
                env.query = new_query;
                self.on_query_change(&mut env);
            }

            self.selection.handle(&ev);

            if let Some(previewer) = self.previewer.as_mut() {
                previewer.handle(&ev);
            }

            self.draw_preview(&env, false);

            let _ = self.do_with_widget(|root| self.term.draw(&root));
            let _ = self.term.present();
        }
    }

    fn draw_preview(&mut self, env: &ModelEnv, force: bool) {
        if self.preview_hidden {
            return;
        }

        // re-draw
        let item_index = self.selection.get_current_item_idx();
        let item = self.selection.get_current_item();
        if let Some(previewer) = self.previewer.as_mut() {
            let selections = &self.selection;
            let get_selected_items = || selections.get_selected_indices_and_items();
            previewer.on_item_change(
                item_index,
                item,
                env.query.to_string(),
                env.cmd_query.to_string(),
                selections.get_num_of_selected_exclude_current(),
                get_selected_items,
                force,
            );
        }
    }

    fn consume_additional_event(&self, target_event: &Event) -> Option<(Key, Event)> {
        // consume additional HeartBeat event
        let mut rx_try_iter = self.rx.try_iter().peekable();
        while let Some((_key, ev)) = rx_try_iter.peek() {
            if *ev == *target_event {
                let _ = rx_try_iter.next();
            } else {
                break;
            }
        }
        // once the event is peeked, it is removed from the pipe, thus need to be saved.
        rx_try_iter.next()
    }

    fn restart_matcher(&mut self) {
        self.matcher_timer = Instant::now();
        let query = self.query.get_fz_query();
        let cmd_query = self.query.get_cmd_query();
        let in_query_mode = self.query.in_query_mode();

        // Check if query meets minimum length requirement before doing anything
        if let Some(min_length) = self.min_query_length {
            // Check the appropriate query based on mode
            let query_to_check = if in_query_mode { &query } else { &cmd_query };

            if query_to_check.chars().count() < min_length {
                // Don't run matcher if query is too short
                // Also kill any existing matcher
                if let Some(ctrl) = self.matcher_control.take() {
                    ctrl.kill();
                }
                return;
            }
        }

        // kill existing matcher if exits
        if let Some(ctrl) = self.matcher_control.take() {
            ctrl.kill();
        }

        // if there are new items, move them to item pool
        let processed = self.reader_control.as_ref().map(|c| c.is_done()).unwrap_or(true);
        if !processed {
            // take out new items and put them into items
            let new_items = self.reader_control.as_ref().map(|c| c.take()).unwrap();
            let _ = self.item_pool.append(new_items);
        };

        // send heart beat (so that heartbeat/refresh is triggered)
        let _ = self.tx.send((Key::Null, Event::EvHeartBeat));

        let matcher = if self.use_regex {
            &self.regex_matcher
        } else {
            &self.matcher
        };

        let tx = self.tx.clone();
        let new_matcher_control = matcher.run(&query, self.item_pool.clone(), move |_| {
            // notify refresh immediately
            let _ = tx.send((Key::Null, Event::EvHeartBeat));
        });

        self.matcher_control.replace(new_matcher_control);
    }

    /// construct the widget tree
    fn do_with_widget<R, F>(&'_ self, action: F) -> R
    where
        F: Fn(Box<dyn Widget<Event> + '_>) -> R,
    {
        let total = self.item_pool.len();
        let matched = self.num_options + self.matcher_control.as_ref().map(|c| c.get_num_matched()).unwrap_or(0);
        let matcher_running = self.item_pool.num_not_taken() != 0 || matched != self.num_options;
        let processed = self
            .matcher_control
            .as_ref()
            .map(|c| c.get_num_processed())
            .unwrap_or(total);

        let status = Status {
            total,
            matched,
            processed,
            matcher_running,
            multi_selection: self.selection.is_multi_selection(),
            selected: self.selection.get_num_selected(),
            current_item_idx: self.selection.get_current_item_idx(),
            hscroll_offset: self.selection.get_hscroll_offset(),
            reading: !self.reader_control.as_ref().map(|c| c.is_done()).unwrap_or(true),
            time_since_read: self.reader_timer.elapsed(),
            time_since_match: self.matcher_timer.elapsed(),
            matcher_mode: if self.use_regex {
                "RE".to_string()
            } else {
                "".to_string()
            },
            theme: self.theme.clone(),
            info: self.info.clone(),
        };
        let status_inline = status.clone();

        let win_selection = Win::new(&self.selection);
        let win_query = Win::new(&self.query)
            .basis(if self.info == InfoDisplay::Default { 1 } else { 0 })
            .grow(0)
            .shrink(0);
        let win_status = Win::new(status)
            .basis(if self.info == InfoDisplay::Default { 1 } else { 0 })
            .grow(0)
            .shrink(0);
        let win_header = Win::new(&self.header).grow(0).shrink(0);
        let win_query_status = HSplit::default()
            .basis(if self.info == InfoDisplay::Default { 0 } else { 1 })
            .grow(0)
            .shrink(0)
            .split(Win::new(&self.query).grow(0).shrink(0))
            .split(Win::new(status_inline).grow(1).shrink(0));

        let layout = &self.layout as &str;
        let win_main = match layout {
            "reverse" => VSplit::default()
                .split(win_query_status)
                .split(win_query)
                .split(win_status)
                .split(win_header)
                .split(win_selection),
            "reverse-list" => VSplit::default()
                .split(win_selection)
                .split(win_header)
                .split(win_status)
                .split(win_query)
                .split(win_query_status),
            _ => VSplit::default()
                .split(win_selection)
                .split(win_header)
                .split(win_status)
                .split(win_query)
                .split(win_query_status),
        };

        let screen: Box<dyn Widget<Event>> = if !self.preview_hidden && self.previewer.is_some() {
            let previewer = self.previewer.as_ref().unwrap();
            let win = Win::new(previewer)
                .basis(self.preview_size)
                .grow(0)
                .shrink(0)
                .border_attr(self.theme.border());

            let win_preview = match self.preview_direction {
                Direction::Up => win.border_bottom(true),
                Direction::Right => win.border_left(true),
                Direction::Down => win.border_top(true),
                Direction::Left => win.border_right(true),
            };

            match self.preview_direction {
                Direction::Up => Box::new(VSplit::default().split(win_preview).split(win_main)),
                Direction::Right => Box::new(HSplit::default().split(win_main).split(win_preview)),
                Direction::Down => Box::new(VSplit::default().split(win_main).split(win_preview)),
                Direction::Left => Box::new(HSplit::default().split(win_preview).split(win_main)),
            }
        } else {
            Box::new(win_main)
        };

        let root = Win::new(screen)
            .margin_top(self.margin_top)
            .margin_right(self.margin_right)
            .margin_bottom(self.margin_bottom)
            .margin_left(self.margin_left);

        action(Box::new(root))
    }
}
