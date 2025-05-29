# Skim TUIKit to Ratatui Migration Analysis

This document provides a comprehensive analysis of how skim currently uses skim-tuikit and outlines the key architectural differences and migration strategy for moving to ratatui.

## Current skim-tuikit Architecture

### Core UI Components

Skim's UI is built around several key components that implement the tuikit `Widget<Event>` and `Draw` traits:

1. **Selection** (`/src/selection.rs`): Manages item list display and selection
2. **Query** (`/src/query.rs`): Handles the search input field
3. **Header** (`/src/header.rs`): Displays header information
4. **Status** (`/src/model/status.rs`): Shows status information (item counts, spinner)
5. **Previewer** (`/src/previewer.rs`): Displays preview content

### Widget Trait Implementation

Each component implements two key traits:

```rust
// From skim-tuikit
pub trait Widget<Message = ()>: Draw {
    fn size_hint(&self) -> (Option<usize>, Option<usize>) { (None, None) }
    fn on_event(&self, event: Event, rect: Rectangle) -> Vec<Message> { Vec::new() }
}

pub trait Draw {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> { Ok(()) }
}
```

### Layout System

Skim uses tuikit's layout system with:
- **Win**: Container with margin, padding, border (like HTML div)
- **HSplit/VSplit**: Horizontal/vertical split containers
- **Size**: Fixed, Percent, or Default sizing
- **Split trait**: Defines basis, grow, shrink factors

### Main Layout Construction (from `/src/model/mod.rs:756-862`)

```rust
fn do_with_widget<R, F>(&'_ self, action: F) -> R {
    // Create individual component windows
    let win_selection = Win::new(&self.selection);
    let win_query = Win::new(&self.query).basis(1).grow(0).shrink(0);
    let win_status = Win::new(status).basis(1).grow(0).shrink(0);
    let win_header = Win::new(&self.header).grow(0).shrink(0);
    
    // Layout based on configuration
    let win_main = match layout {
        "reverse" => VSplit::default()
            .split(win_query_status)
            .split(win_query)
            .split(win_status)
            .split(win_header)
            .split(win_selection),
        // ... other layouts
    };
    
    // Add preview if enabled
    let screen: Box<dyn Widget<Event>> = if !self.preview_hidden {
        match self.preview_direction {
            Direction::Up => Box::new(VSplit::default().split(win_preview).split(win_main)),
            Direction::Right => Box::new(HSplit::default().split(win_main).split(win_preview)),
            // ... other directions
        }
    } else {
        Box::new(win_main)
    };
    
    // Apply margins
    let root = Win::new(screen)
        .margin_top(self.margin_top)
        .margin_right(self.margin_right)
        .margin_bottom(self.margin_bottom)
        .margin_left(self.margin_left);
}
```

### Event System

Events flow through:
1. Terminal input → skim-tuikit `Event` (Key, Resize, etc.)
2. Input mapper translates to skim `Event` enum
3. Components handle events via `EventHandler` trait
4. Widget `on_event` method handles mouse events

### Canvas Drawing

Components draw to a `Canvas` trait:
```rust
pub trait Canvas {
    fn size(&self) -> Result<(usize, usize)>;
    fn clear(&mut self) -> Result<()>;
    fn put_cell(&mut self, row: usize, col: usize, cell: Cell) -> Result<usize>;
    fn print_with_attr(&mut self, row: usize, col: usize, content: &str, attr: Attr) -> Result<usize>;
    fn set_cursor(&mut self, row: usize, col: usize) -> Result<()>;
    fn show_cursor(&mut self, show: bool) -> Result<()>;
}
```

## Key Architectural Differences with Ratatui

### 1. Widget System

**TUIKit (Current)**:
- Widgets implement `Draw` trait for rendering
- `Widget<Message>` trait for event handling and size hints
- Direct canvas drawing with `put_cell`, `print_with_attr`
- Manual layout calculation

**Ratatui (Target)**:
- Widgets implement `Widget` trait that renders to `Buffer`
- StatefulWidget for components with state
- Declarative rendering - widgets describe what to render
- Automatic layout with constraints

### 2. Layout System

**TUIKit**:
```rust
HSplit::default()
    .split(Win::new(&component1).basis(Size::Fixed(10)))
    .split(Win::new(&component2).grow(1))
```

**Ratatui**:
```rust
Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Length(10), Constraint::Min(0)])
    .split(area)
```

### 3. State Management

**TUIKit**:
- Components hold mutable state
- Direct mutation during event handling
- `EventHandler` trait for state updates

**Ratatui**:
- StatefulWidget pattern separates widget from state
- State is passed to widget during rendering
- External state management

### 4. Event Handling

**TUIKit**:
- Widgets handle events directly via `on_event`
- Event bubbling through layout hierarchy
- Mouse events include absolute coordinates

**Ratatui**:
- Application handles events
- Manual dispatch to appropriate handlers
- No built-in event system

## Migration Strategy

### Phase 1: Create Ratatui Widget Wrappers

Create equivalent widgets for each component:

```rust
// New ratatui implementations
pub struct SelectionWidget<'a> {
    selection: &'a Selection,
}

impl<'a> Widget for SelectionWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Convert tuikit drawing calls to ratatui buffer operations
    }
}

pub struct QueryWidget<'a> {
    query: &'a Query,
}

impl<'a> StatefulWidget for QueryWidget<'a> {
    type State = QueryState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Render query input with cursor
    }
}
```

### Phase 2: Layout Migration

Replace tuikit layout with ratatui constraints:

```rust
fn create_layout(area: Rect, model: &Model) -> Vec<Rect> {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(0),     // Selection
            Constraint::Length(1), // Status
            Constraint::Length(1), // Query
        ])
        .split(area);
    
    if !model.preview_hidden {
        let preview_chunks = Layout::default()
            .direction(match model.preview_direction {
                Direction::Left | Direction::Right => Direction::Horizontal,
                Direction::Up | Direction::Down => Direction::Vertical,
            })
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(main_chunks[1]);
        // Return modified layout
    }
    
    main_chunks
}
```

### Phase 3: Event System Migration

Replace tuikit event handling:

```rust
impl Model {
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<SkimOutput> {
        // Convert crossterm KeyEvent to skim Event
        let skim_event = self.input.translate_key_event(key);
        
        match skim_event {
            Event::EvActUp(diff) => self.selection.act_move_line_cursor(*diff),
            Event::EvActDown(diff) => self.selection.act_move_line_cursor(-*diff),
            Event::EvActAddChar(ch) => self.query.act_add_char(*ch),
            // ... other events
        }
        
        None
    }
    
    pub fn handle_mouse_event(&mut self, mouse: MouseEvent, layout: &[Rect]) -> Option<SkimOutput> {
        // Determine which component was clicked based on layout
        let component = self.get_component_at_position(mouse.row, mouse.column, layout);
        
        match component {
            Component::Selection => self.selection.handle_mouse(mouse),
            Component::Query => self.query.handle_mouse(mouse),
            // ... other components
        }
        
        None
    }
}
```

### Phase 4: Terminal Integration

Replace tuikit Terminal with crossterm + ratatui:

```rust
pub fn run_with(options: &SkimOptions, source: Option<SkimItemReceiver>) -> Option<SkimOutput> {
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut model = Model::new(options, source);
    
    loop {
        // Render
        terminal.draw(|f| {
            let chunks = create_layout(f.size(), &model);
            
            f.render_widget(SelectionWidget { selection: &model.selection }, chunks[1]);
            f.render_stateful_widget(
                QueryWidget { query: &model.query }, 
                chunks[3], 
                &mut model.query_state
            );
            f.render_widget(HeaderWidget { header: &model.header }, chunks[0]);
            f.render_widget(StatusWidget { status: &model.status }, chunks[2]);
        })?;
        
        // Handle events
        if crossterm::event::poll(Duration::from_millis(100))? {
            match crossterm::event::read()? {
                crossterm::event::Event::Key(key) => {
                    if let Some(output) = model.handle_key_event(key) {
                        return Some(output);
                    }
                }
                crossterm::event::Event::Mouse(mouse) => {
                    let layout = create_layout(terminal.size()?, &model);
                    model.handle_mouse_event(mouse, &layout);
                }
                crossterm::event::Event::Resize(_, _) => {
                    // Handle resize
                }
            }
        }
        
        // Handle heartbeat/background processing
        model.process_background_tasks();
    }
}
```

## Critical Implementation Details

### 1. Canvas to Buffer Conversion

Convert tuikit canvas operations to ratatui buffer operations:

```rust
// TUIKit
canvas.print_with_attr(row, col, text, attr)?;

// Ratatui equivalent
for (i, ch) in text.chars().enumerate() {
    if col + i < area.width as usize {
        buf.get_mut(area.x + col + i, area.y + row)
            .set_char(ch)
            .set_style(convert_attr_to_style(attr));
    }
}
```

### 2. Attribute/Style Conversion

Map tuikit attributes to ratatui styles:

```rust
fn convert_attr_to_style(attr: skim_tuikit::attr::Attr) -> ratatui::style::Style {
    let mut style = Style::default();
    
    if let Some(fg) = attr.fg {
        style = style.fg(convert_color(fg));
    }
    
    if let Some(bg) = attr.bg {
        style = style.bg(convert_color(bg));
    }
    
    if attr.effect.contains(Effect::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    
    // ... other effects
    
    style
}
```

### 3. Mouse Event Handling

Convert absolute mouse coordinates to component-relative:

```rust
fn get_component_at_position(&self, row: u16, col: u16, layout: &[Rect]) -> Component {
    for (i, rect) in layout.iter().enumerate() {
        if row >= rect.y && row < rect.y + rect.height &&
           col >= rect.x && col < rect.x + rect.width {
            return Component::from_index(i);
        }
    }
    Component::None
}
```

### 4. Size Calculation

Convert tuikit size hints to ratatui constraints:

```rust
// TUIKit size hint
fn size_hint(&self) -> (Option<usize>, Option<usize>) {
    (Some(80), Some(24))
}

// Ratatui constraint
fn get_constraints(&self) -> Vec<Constraint> {
    vec![
        Constraint::Length(80),  // width
        Constraint::Length(24),  // height
    ]
}
```

## Migration Benefits

1. **Better Performance**: Ratatui's buffer-based rendering is more efficient
2. **Modern Architecture**: More idiomatic Rust patterns
3. **Active Development**: Ratatui is actively maintained with regular updates
4. **Better Documentation**: More comprehensive docs and examples
5. **Ecosystem**: Better integration with modern terminal libraries

## Migration Risks

1. **Breaking Changes**: Extensive changes to internal architecture
2. **Behavior Differences**: Subtle differences in rendering or event handling
3. **Performance Regressions**: Potential temporary performance impact during transition
4. **Testing Complexity**: Need comprehensive testing of UI behavior
5. **Timeline**: Significant development effort required

## Recommended Approach

1. **Create Feature Branch**: Isolate migration work
2. **Incremental Migration**: Migrate components one at a time
3. **Dual Implementation**: Keep both implementations during transition
4. **Extensive Testing**: Comprehensive UI and integration tests
5. **Feature Parity**: Ensure exact behavioral compatibility
6. **Performance Validation**: Benchmark before/after performance

This migration will modernize skim's UI architecture while maintaining all existing functionality and behavior.