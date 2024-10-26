use std::{cell::RefCell, rc::Rc};

use crate::prelude::*;

pub struct SkimContext {
    pub cmd_collector: Rc<RefCell<SkimItemReader>>,
    pub query_history: Vec<String>,
    pub cmd_history: Vec<String>,
}

impl Default for SkimContext {
    fn default() -> Self {
        return Self {
            cmd_collector: Rc::new(RefCell::new(SkimItemReader::new(Default::default()))),
            query_history: vec![],
            cmd_history: vec![]
        }
    }
}
