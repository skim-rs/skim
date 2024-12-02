//! header of the items
use crate::theme::ColorTheme;
use crate::theme::DEFAULT_THEME;
use crate::SkimOptions;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::text::ToLine;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::cmp::max;
use std::sync::Arc;

#[derive(Clone)]
pub struct Header {
    header: Vec<String>,
    tabstop: usize,
    reverse: bool,
    theme: Arc<ColorTheme>,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            header: Vec::default(),
            tabstop: 8,
            reverse: false,
            theme: Arc::new(*DEFAULT_THEME),
        }
    }
}

impl Header {
    pub fn theme(mut self, theme: Arc<ColorTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_options(mut self, options: &SkimOptions) -> Self {
        self.tabstop = max(1, options.tabstop);

        if options.layout.starts_with("reverse") {
            self.reverse = true;
        }

        if let Some(header) = options.header.clone() {
            self.header = header.split("\n").map(|s| s.to_string()).collect();
        }

        self
    }

    fn height(&self) -> u16 {
        u16::try_from(self.header.len()).expect("Header len did not fit into a u16. Really ?")
    }

    fn adjust_row(&self, index: usize, screen_height: usize) -> usize {
        if self.reverse {
            index
        } else {
            screen_height - index - 1
        }
    }
}

impl Widget for &Header {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 3 {
            panic!("screen width is too small to fit the header");
        }

        if area.height < self.height() {
            panic!("screen height is too small to fit the header");
        }

        let header_with_lines = self.header.iter().map(|l| l.to_line()).collect::<Vec<Line>>();
        Paragraph::new(Text::from(header_with_lines)).render(area, buf)
    }
}
