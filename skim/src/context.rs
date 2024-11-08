use std::{cell::RefCell, rc::Rc};

use util::read_file_lines;

use crate::prelude::*;

pub struct SkimContext {
    pub cmd_collector: Rc<RefCell<SkimItemReader>>,
    pub query_history: Vec<String>,
    pub cmd_history: Vec<String>,
}

impl SkimContext {
    pub fn init_histories(&mut self, opts: &SkimOptions) {
        if let Some(histfile) = &opts.history {
            self.query_history.extend(read_file_lines(histfile).unwrap_or_default());
        }

        if let Some(cmd_histfile) = &opts.cmd_history {
            self.cmd_history
                .extend(read_file_lines(cmd_histfile).unwrap_or_default());
        }
    }
}

impl Default for SkimContext {
    fn default() -> Self {
        Self {
            cmd_collector: Rc::new(RefCell::new(SkimItemReader::new(Default::default()))),
            query_history: vec![],
            cmd_history: vec![],
        }
    }
}
