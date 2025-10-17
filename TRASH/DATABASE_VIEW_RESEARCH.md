# Database Management View - Research & Feasibility Analysis

## Overview
Research for adding a database management button in the sidebar that opens a new view for managing the DuckDB database while charts continue running in the background.

---

## Current UI Structure

### Sidebar Navigation Buttons (src/screen/dashboard/sidebar.rs:119-195)

Current button layout (top to bottom):
```
┌──────────────┐
│   🔍 Search  │ ← ticker_search_button (line 154-166)
│   📐 Layout  │ ← layout_modal_button (line 140-152)
│   🔊 Audio   │ ← audio_btn (line 168-184)
│              │
│   (space)    │ ← space::vertical() (line 190)
│              │
│   ⚙️ Settings │ ← settings_modal_button (line 125-138)
└──────────────┘
```

**Proposed Addition:**
Add database button between Audio and Settings (after the spacer):
```rust
column![
    ticker_search_button,
    layout_modal_button,
    audio_btn,
    space::vertical(),
    database_button,        // ← NEW
    settings_modal_button,
]
```

---

## Menu System Architecture

### 1. Menu Enum (data/src/config/sidebar.rs:66-72)

Current menus:
```rust
pub enum Menu {
    Layout,      // Layout manager modal
    Settings,    // Settings modal
    Audio,       // Audio stream selector
    ThemeEditor, // Theme customization
}
```

**Required Change:**
Add new variant:
```rust
pub enum Menu {
    Layout,
    Settings,
    Audio,
    ThemeEditor,
    Database,    // ← NEW
}
```

### 2. Modal Rendering (src/main.rs:677-1010)

Each menu case in `view_with_modal()` renders a modal overlay:
- `Menu::Settings` → Settings modal (line 686-869)
- `Menu::Layout` → Layout manager (line 870-977)
- `Menu::Audio` → Audio stream selector (line 978-996)
- `Menu::ThemeEditor` → Theme editor (line 997+)

**Required Addition:**
New case for Database menu:
```rust
sidebar::Menu::Database => {
    // Database management view implementation
}
```

---

## Modal System (src/modal.rs)

### Dashboard Modal Function (line 42-66)
```rust
pub fn dashboard_modal<'a, Message>(
    base: impl Into<Element<'a, Message>>,      // Background (charts)
    content: impl Into<Element<'a, Message>>,   // Modal content
    on_blur: Message,                            // Close handler
    padding: padding::Padding,
    align_y: Alignment,
    align_x: Alignment,
) -> Element<'a, Message>
```

**How it works:**
- Creates a `stack![]` with base layer (charts) and overlay layer (modal)
- Base layer remains rendered underneath
- Modal intercepts mouse events but base continues updating
- Charts/WebSockets continue running in background ✓

---

## Icon System (src/style.rs:24-57)

### Available Icons
Currently available icons that could work:
- `Icon::Folder` (line 55) - Could represent database storage
- Custom font glyphs via Unicode: `'\u{EXXX}'`

### Options:
1. **Use existing icon**: `Icon::Folder`
2. **Add new icon**: `Icon::Database` with new Unicode glyph
3. **Use emoji**: Direct Unicode database icon

**Recommendation**: Add proper `Icon::Database` variant for consistency.

---

## Database Management Features to Implement

### Suggested View Layout

```
┌─────────────────────────────────────────────────────────┐
│  Database Manager                              [Close]  │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  📊 DATABASE OVERVIEW                                    │
│  ├─ Size: 28 MB                                         │
│  ├─ Total Trades: 178,289                               │
│  ├─ Total Klines: 747                                   │
│  └─ Tickers: 2                                          │
│                                                          │
│  📁 DATA BY TICKER                                       │
│  ┌────────────────────────────────────────────────────┐ │
│  │ Symbol    Exchange   Trades    First      Last     │ │
│  │ ETHUSDT   Binance    178,289   16:14:22   17:03:39│ │
│  │ ETHUSDT   Aster      0         -          -       │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  🗑️ CLEANUP OPERATIONS                                  │
│  ├─ [Delete trades older than: [7] days] [Run]         │
│  ├─ [Delete klines older than: [30] days] [Run]        │
│  └─ [Vacuum database] [Run]                            │
│                                                          │
│  💾 BACKUP & RESTORE                                     │
│  ├─ [Create backup] [Restore from backup]              │
│  └─ Last backup: Never                                  │
│                                                          │
│  📤 EXPORT                                               │
│  └─ [Export to CSV] [Export to Parquet]                │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

---

## Required Implementation Steps

### 1. **Data Layer** (data crate)
- ✅ Already exists: Query functions (TradesCRUD, KlinesCRUD)
- ✅ Already exists: Cleanup functions (delete_trades_older_than)
- ✅ Already exists: VACUUM operation
- ⚠️ Need to add: Backup/restore functions
- ⚠️ Need to add: CSV/Parquet export

### 2. **UI Component** (new file: src/modal/database_manager.rs)
Create new module similar to:
- `src/modal/layout_manager.rs`
- `src/modal/theme_editor.rs`

Structure:
```rust
pub struct DatabaseManager {
    // State for database stats
    stats: Option<DbStats>,
    // Cleanup settings
    cleanup_days_trades: u32,
    cleanup_days_klines: u32,
    // etc.
}

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
    DeleteOldTrades,
    DeleteOldKlines,
    Vacuum,
    CreateBackup,
    RestoreBackup,
    ExportCsv,
    // etc.
}

pub enum Action {
    // Actions that need to propagate up
}

impl DatabaseManager {
    pub fn new() -> Self { ... }

    pub fn update(&mut self, message: Message) -> (Task<Message>, Option<Action>) { ... }

    pub fn view(&self) -> Element<Message> { ... }
}
```

### 3. **Main Integration** (src/main.rs)

**Add to Flowsurface struct:**
```rust
pub struct Flowsurface {
    // ... existing fields ...
    database_manager: modal::DatabaseManager,
}
```

**Add message variant:**
```rust
pub enum Message {
    // ... existing variants ...
    DatabaseManager(modal::database_manager::Message),
}
```

**Add modal rendering:**
```rust
sidebar::Menu::Database => {
    let (align_x, padding) = match sidebar_pos {
        sidebar::Position::Left => (Alignment::Start, padding::left(44).top(76)),
        sidebar::Position::Right => (Alignment::End, padding::right(44).top(76)),
    };

    dashboard_modal(
        base,
        self.database_manager.view().map(Message::DatabaseManager),
        Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
        padding,
        Alignment::Start,
        align_x,
    )
}
```

### 4. **Button Addition** (src/screen/dashboard/sidebar.rs)

Add between audio_btn and settings:
```rust
let database_button = {
    let is_active = self.is_menu_active(sidebar::Menu::Database);

    button_with_tooltip(
        icon_text(Icon::Database, 14)  // or Icon::Folder
            .width(24)
            .align_x(Alignment::Center),
        Message::ToggleSidebarMenu(Some(sidebar::Menu::Database)),
        None,
        tooltip_position,
        move |theme, status| crate::style::button::transparent(theme, status, is_active),
    )
};

column![
    ticker_search_button,
    layout_modal_button,
    audio_btn,
    space::vertical(),
    database_button,        // ← NEW
    settings_modal_button,
]
```

---

## Background Processing Analysis

### ✅ Charts Will Continue Working

**Evidence from architecture:**
1. **Modal System**: Uses `stack![]` which layers views
   - Base layer (charts) remains in DOM
   - Modal only overlays on top
   - Both layers render in same update cycle

2. **WebSocket Streams**: Independent of UI
   - Handled in `Dashboard::subscription()` (src/screen/dashboard.rs)
   - Events processed in `Dashboard::update()`
   - Data flows regardless of active view
   - Dual-write still happens (database inserts continue)

3. **Similar Examples**:
   - Settings modal already works this way
   - Layout manager already works this way
   - Theme editor already works this way
   - All allow background chart updates

### Update Flow with Database View Open:
```
WebSocket Event
    ↓
Dashboard::update()
    ↓
distribute_fetched_data()
    ↓
├─→ persist_to_database()  ← Still happens!
│
└─→ update_in_memory()     ← Still happens!
    ↓
    render()
    ├─→ Base layer (charts updated)
    └─→ Modal layer (database view)
```

---

## Database Operations to Expose

### Real-time Statistics (Read-only)
- ✅ Trade count per ticker/exchange
- ✅ Kline count per ticker/timeframe
- ✅ Database size
- ✅ Date range of stored data
- ✅ Memory usage

### Cleanup Operations
- ✅ Delete trades older than N days
- ✅ Delete klines older than N days
- ✅ VACUUM (reclaim space)
- ⚠️ Clear all data for specific ticker
- ⚠️ Clear all data for specific exchange

### Backup/Restore
- ⚠️ Create backup (copy .duckdb file)
- ⚠️ List available backups
- ⚠️ Restore from backup
- ⚠️ Auto-backup before cleanup

### Export
- ⚠️ Export trades to CSV
- ⚠️ Export klines to CSV
- ⚠️ Export to Parquet (native DuckDB)

### Import
- ⚠️ Import from CSV
- ⚠️ Import from Binance ZIP archives

Legend: ✅ Already implemented | ⚠️ Needs implementation

---

## Potential Challenges

### 1. **Async Operations**
Database operations can be slow (especially vacuum, export, backup).

**Solution**: Use `Task` for async operations:
```rust
Task::perform(
    async move {
        // Long-running DB operation
        db_manager.vacuum()?;
        Ok(())
    },
    |result| Message::VacuumComplete(result)
)
```

### 2. **Database Lock**
DuckDB has single-writer limitation.

**Solution**:
- Read operations use read-only mode (`-readonly`)
- Write operations coordinate with dual-write system
- Show loading state during operations

### 3. **UI Responsiveness**
Large queries could freeze UI.

**Solution**:
- Use background tasks for queries
- Show loading spinners
- Implement pagination for large result sets

### 4. **Error Handling**
Database operations can fail.

**Solution**:
- Show error toasts
- Graceful degradation
- Clear error messages to user

---

## Effort Estimate

### Time Breakdown:

1. **Data Layer Additions** (4-6 hours)
   - Backup/restore functions
   - Export to CSV/Parquet
   - Additional query helpers

2. **UI Component** (8-12 hours)
   - DatabaseManager modal component
   - Layout and styling
   - Interactive controls
   - Stats display

3. **Integration** (2-4 hours)
   - Add to sidebar
   - Wire up message handling
   - Add icon (if new)
   - Testing

4. **Polish** (2-4 hours)
   - Error handling
   - Loading states
   - Tooltips
   - Documentation

**Total: 16-26 hours** for full implementation

---

## Recommendation

✅ **This is FEASIBLE and STRAIGHTFORWARD**

### Pros:
- Clean architecture supports this well
- Modal system already proven with 4 existing modals
- Charts continue in background (verified)
- Database query layer already exists
- Follows existing patterns

### Cons:
- Need to implement backup/restore/export features
- Async operations need careful handling
- Some database functions not yet implemented

### Minimal Version (4-6 hours):
Just show read-only statistics:
- Database overview
- Trade/kline counts
- Size information
- No cleanup/export features yet

### Full Version (16-26 hours):
Complete database management interface with all features.

---

## Next Steps

If you want to proceed:

1. **Choose icon**: Add `Icon::Database` or use `Icon::Folder`?
2. **Decide scope**: Minimal (stats only) or full (with operations)?
3. **Implementation order**:
   - Add Menu::Database variant
   - Create DatabaseManager component
   - Add button to sidebar
   - Wire up modal rendering
   - Implement features incrementally

Would you like me to implement any specific part of this?
