use std::io::{self, Stdout};
use std::sync::Arc;

use crossterm::{
    event::{self, Event as CrosstermEvent},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

/// Terminal wrapper that provides skim-tuikit compatible interface
pub struct SkimTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    original_hook: Option<Arc<Box<dyn Fn(&std::panic::PanicInfo<'_>) + Sync + Send + 'static>>>,
    partial_screen: bool,
}

#[derive(Debug, Clone)]
pub enum TermHeight {
    Percent(u16),
    Fixed(u16),
}

#[derive(Debug)]
pub struct TermOptions {
    pub height: Option<TermHeight>,
    pub enable_mouse: bool,
    pub alternate_screen: bool,
}

impl Default for TermOptions {
    fn default() -> Self {
        Self {
            height: None,
            enable_mouse: true,  // Enable mouse support by default
            alternate_screen: true,  // Use alternate screen by default
        }
    }
}

impl SkimTerminal {
    /// Create a new terminal with default options
    pub fn new() -> io::Result<Self> {
        Self::with_options(TermOptions::default())
    }
    
    /// Create a new terminal with specified height
    pub fn with_height_opt(height: Option<TermHeight>) -> io::Result<Self> {
        let options = TermOptions {
            height,
            ..Default::default()
        };
        Self::with_options(options)
    }
    
    /// Create a new terminal with options
    pub fn with_options(options: TermOptions) -> io::Result<Self> {
        // Try to enable raw mode, which will fail if no TTY is available
        if let Err(e) = terminal::enable_raw_mode() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Terminal not available - cannot enable raw mode: {}", e),
            ));
        }
        
        let mut stdout = io::stdout();
        
        // Enter alternate screen if requested
        if options.alternate_screen {
            stdout.execute(EnterAlternateScreen)?;
        }
        
        // Enable mouse events if requested
        if options.enable_mouse {
            crossterm::execute!(stdout, crossterm::event::EnableMouseCapture)?;
        }
        
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        
        // Handle partial screen mode
        let partial_screen = options.height.is_some();
        
        // Setup panic hook for cleanup
        let original_hook = std::panic::take_hook();
        let hook_arc = Arc::new(original_hook);
        let hook_clone = Arc::clone(&hook_arc);
        std::panic::set_hook(Box::new(move |panic| {
            let _ = restore_terminal();
            hook_clone(panic);
        }));
        
        Ok(Self {
            terminal,
            original_hook: Some(hook_arc),
            partial_screen,
        })
    }
    
    /// Draw the UI
    pub fn draw<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }
    
    /// Present/flush the terminal (ratatui handles this automatically)
    pub fn present(&mut self) -> io::Result<()> {
        // In ratatui, drawing automatically presents
        Ok(())
    }
    
    /// Resize the terminal (handled automatically by ratatui)
    pub fn resize(&mut self, _width: u16, _height: u16) -> io::Result<()> {
        // Ratatui handles terminal resizing automatically
        Ok(())
    }
    
    /// Poll for events with timeout
    pub fn poll_event(&mut self) -> io::Result<Option<CrosstermEvent>> {
        if event::poll(std::time::Duration::from_millis(100))? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }
    
    /// Check if an event is available
    pub fn has_event(&self) -> io::Result<bool> {
        event::poll(std::time::Duration::from_millis(0))
    }
    
    /// Get terminal size
    pub fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }
    
    /// Pause the terminal (for external commands)
    pub fn pause(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()?;
        crossterm::execute!(
            io::stdout(),
            LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )?;
        Ok(())
    }
    
    /// Restart the terminal after pause
    pub fn restart(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        crossterm::execute!(
            io::stdout(),
            EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        Ok(())
    }
    
    /// Clean shutdown
    pub fn shutdown(&mut self) -> io::Result<()> {
        restore_terminal()
    }
}

impl Drop for SkimTerminal {
    fn drop(&mut self) {
        let _ = restore_terminal();
        
        // Restore original panic hook
        if let Some(_hook_arc) = self.original_hook.take() {
            // We can't easily extract from Arc, so we'll just use a simple default
            std::panic::set_hook(Box::new(|info| {
                eprintln!("panic occurred: {}", info);
            }));
        }
    }
}

/// Restore terminal to normal state
fn restore_terminal() -> io::Result<()> {
    terminal::disable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    Ok(())
}

// Re-export common types for compatibility
pub use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent};
pub use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
};