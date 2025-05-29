# Skim Ratatui Migration Strategy

## Overview

This document outlines the comprehensive migration plan from skim-tuikit to ratatui for skim v2.0.0. This is a **major breaking change** that will modernize the UI foundation while maintaining core functionality.

## Current Architecture Analysis

### TUIKit Usage Patterns

**Core Components:**
- `Selection` (`skim/src/selection.rs`) - Item list with scrolling, highlighting, multi-selection
- `Query` (`skim/src/query.rs`) - Input field with syntax highlighting  
- `Header` (`skim/src/header.rs`) - Title/header information
- `Status` (`skim/src/model/status.rs`) - Status line with counts
- `Previewer` (`skim/src/previewer.rs`) - File preview pane

**Current Architecture:**
- **Widget Trait**: All components implement `Widget<Event>` and `Draw` traits
- **Event System**: Built-in event bubbling with `on_event(event, rect)` handlers
- **Layout**: Flex-box-like system using `HSplit`/`VSplit` with `basis`, `grow`, `shrink`
- **Drawing**: Direct canvas operations with `put_cell()`, `print_with_attr()`
- **State**: Widget-owned state with direct mutation

### Key TUIKit APIs Being Used

```rust
// Widget trait
impl Widget<Event> for Selection {
    fn on_event(&mut self, event: Event, area: Rect) -> EventResult
}

// Drawing trait  
impl Draw for Selection {
    fn draw(&mut self, canvas: &mut dyn Canvas) -> Result<()>
}

// Layout system
let layout = HSplit::default()
    .split(win_main)
    .split(win_preview);

// Terminal management
let term = Term::with_height_opt(height)?;
term.draw(&screen)?;
term.present()?;
```

## Ratatui Target Architecture

### Core Differences

| Aspect | TUIKit | Ratatui |
|--------|--------|---------|
| **Rendering** | Imperative canvas drawing | Declarative buffer rendering |
| **State** | Widget-owned | External state management |
| **Events** | Built-in bubbling | Manual dispatch |
| **Layout** | Flex-box-like | Constraint-based |
| **Redraws** | Dirty regions | Full frame rebuild |

### Recommended Architecture: Modified Elm (TEA)

```rust
// Application state
struct SkimState {
    query: QueryState,
    selection: SelectionState,
    header: HeaderState,
    status: StatusState,
    previewer: PreviewerState,
    // ... other state
}

// Events/Messages
enum SkimMessage {
    UpdateQuery(String),
    SelectItem(usize),
    TogglePreview,
    // ... other events
}

// Update function
fn update(state: &mut SkimState, msg: SkimMessage) -> Option<SkimMessage>

// View function
fn view(state: &SkimState, frame: &mut Frame)
```

## Migration Strategy

### Phase 1: API Mapping

#### Widget System Migration

**Current (TUIKit):**
```rust
impl Widget<Event> for Selection {
    fn on_event(&mut self, event: Event, area: Rect) -> EventResult {
        // Handle events, mutate self
    }
}

impl Draw for Selection {
    fn draw(&mut self, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.put_cell(row, col, cell);
    }
}
```

**Target (Ratatui):**
```rust
// Stateless widget
impl Widget for SelectionWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Render based on passed state
    }
}

// Or stateful widget
impl StatefulWidget for SelectionWidget {
    type State = SelectionState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Render with mutable state access
    }
}

// Event handling (separate)
fn handle_selection_event(state: &mut SelectionState, event: Event) -> Option<SkimMessage>
```

#### Layout System Migration

**Current (TUIKit):**
```rust
let main_split = HSplit::default()
    .basis(80)
    .grow(1)
    .shrink(1)
    .split(left_panel)
    .split(right_panel);
```

**Target (Ratatui):**
```rust
let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Percentage(80),
        Constraint::Min(20)
    ])
    .split(area);

frame.render_widget(left_panel, chunks[0]);
frame.render_widget(right_panel, chunks[1]);
```

#### Terminal Management Migration

**Current (TUIKit):**
```rust
let mut term = Term::with_height_opt(height)?;
term.draw(&screen)?;
term.present()?;
```

**Target (Ratatui + Crossterm):**
```rust
let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
terminal.draw(|frame| {
    view(&state, frame);
})?;
```

### Phase 2: Component-by-Component Migration

#### 1. Query Component
- **Current**: Direct text input with cursor management
- **Target**: Use ratatui `Paragraph` widget with custom cursor rendering
- **Key Challenge**: Syntax highlighting for search patterns

#### 2. Selection Component  
- **Current**: Custom scrollable list with multi-selection
- **Target**: Use ratatui `List` widget with custom state management
- **Key Challenge**: Maintaining scroll position and selection state

#### 3. Header Component
- **Current**: Simple text display
- **Target**: Use ratatui `Paragraph` widget
- **Key Challenge**: Minimal - straightforward migration

#### 4. Status Component
- **Current**: Formatted status line
- **Target**: Use ratatui `Paragraph` widget with spans
- **Key Challenge**: Dynamic content formatting

#### 5. Previewer Component
- **Current**: Text display with ANSI color support
- **Target**: Use ratatui `Paragraph` widget with styled spans
- **Key Challenge**: ANSI color code parsing and conversion

### Phase 3: Layout System Migration

#### Current Layout Structure
```rust
// Complex nested layout with preview positioning
let win_main = match layout {
    "reverse" => VSplit::default()
        .split(query_status)
        .split(query)
        .split(status)  
        .split(header)
        .split(selection),
    _ => VSplit::default()
        .split(header)
        .split(selection)
        .split(status)
        .split(query),
};

let screen = match preview_direction {
    Direction::Right => HSplit::default().split(win_main).split(win_preview),
    Direction::Left => HSplit::default().split(win_preview).split(win_main),
    Direction::Up => VSplit::default().split(win_preview).split(win_main),
    Direction::Down => VSplit::default().split(win_main).split(win_preview),
};
```

#### Target Layout Structure
```rust
fn create_layout(area: Rect, config: &LayoutConfig) -> Vec<Rect> {
    let main_chunks = match config.preview_direction {
        Some(Direction::Right) => Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Percentage(40)])
            .split(area),
        // ... other directions
        None => vec![area],
    };
    
    let main_area = main_chunks[0];
    let preview_area = main_chunks.get(1);
    
    let main_layout = match config.layout {
        "reverse" => Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // header
                Constraint::Min(1),    // selection
                Constraint::Length(1), // status
                Constraint::Length(1), // query
            ])
            .split(main_area),
        _ => Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // header
                Constraint::Min(1),    // selection  
                Constraint::Length(1), // status
                Constraint::Length(1), // query
            ])
            .split(main_area),
    };
    
    main_layout
}
```

### Phase 4: Event System Migration

#### Current Event Flow
```rust
// Terminal events -> Input translation -> Widget dispatch
let event = term.poll_event()?;
let action = self.input.translate_event(event);
let result = widget.on_event(action, area);
```

#### Target Event Flow  
```rust
// Terminal events -> Input translation -> State updates -> Redraw
let event = crossterm::event::read()?;
let action = translate_event(event);
let message = handle_action(&mut state, action);

terminal.draw(|frame| {
    view(&state, frame);
})?;
```

## Implementation Plan

### Dependencies Update
```toml
[dependencies]
# Remove
# skim-tuikit = { path = "../skim-tuikit" }

# Add
ratatui = "0.27"
crossterm = "0.27"
```

### File Structure Changes
```
skim/src/
├── ui/                    # New UI module
│   ├── mod.rs            # UI state and main view function
│   ├── components/       # Ratatui widget implementations
│   │   ├── selection.rs
│   │   ├── query.rs  
│   │   ├── header.rs
│   │   ├── status.rs
│   │   └── previewer.rs
│   ├── layout.rs         # Layout management
│   ├── events.rs         # Event handling
│   └── terminal.rs       # Terminal abstraction
├── model/
│   └── mod.rs           # Updated to use new UI system
└── input.rs             # Updated for crossterm events
```

## Breaking Changes

### Public API Changes
- **Remove**: `pub use skim_tuikit as tuikit;`
- **Remove**: All tuikit re-exports
- **Change**: Event types and handling
- **Change**: Custom widget APIs

### Library Integration Changes
- Applications using `skim::tuikit` will break
- Custom widget implementations need complete rewrite  
- Event handling patterns require updates

## Testing Strategy

### Unit Tests
- Test each component's rendering output
- Test state management functions
- Test event handling logic

### Integration Tests
- Test complete UI flows
- Test layout calculations
- Test event propagation

### E2E Tests
- Existing e2e tests should continue to work
- Test interactive scenarios
- Test performance characteristics

## Migration Timeline

**Week 1-2**: Research and setup (✓ Completed)
**Week 3-4**: Core infrastructure and terminal abstraction
**Week 5-8**: Component migration (Query, Selection, Header, Status, Previewer)
**Week 9-10**: Layout system and main UI loop
**Week 11-12**: Testing, optimization, and documentation
**Week 13**: Release preparation

## Risk Mitigation

### Compatibility Layer
Consider providing a minimal compatibility shim for the most common `skim::tuikit` usage patterns to ease migration for external users.

### Feature Parity
Ensure all current UI features are maintained:
- Partial screen mode support
- ANSI color handling
- Mouse support
- All layout modes and preview directions
- Custom themes and styling

### Performance Validation
- Benchmark rendering performance before/after
- Memory usage comparison
- Responsiveness testing with large datasets

## Success Criteria

- [ ] All current UI functionality preserved
- [ ] Performance matches or exceeds current implementation  
- [ ] All tests pass (unit, integration, e2e)
- [ ] Documentation updated
- [ ] Migration guide provided for external users
- [ ] Clean removal of skim-tuikit dependency