# Undo/Redo System Implementation (Phase 6.8)

## Overview

Implemented a comprehensive undo/redo system for the rune-text library, completing Phase 6.8 of the text rendering checklist. The system supports operation grouping, size limits, and tracks both text changes and selection state.

## Architecture

### Core Components

#### 1. `TextOperation` Enum (`undo.rs`)

Represents a single undoable/redoable operation:

```rust
pub enum TextOperation {
    Insert {
        offset: usize,
        text: String,
        selection_before: Selection,
        selection_after: Selection,
    },
    Delete {
        offset: usize,
        text: String,
        selection_before: Selection,
        selection_after: Selection,
    },
    Replace {
        offset: usize,
        old_text: String,
        new_text: String,
        selection_before: Selection,
        selection_after: Selection,
    },
}
```

**Features:**
- Stores both text changes and selection state
- Supports merging for operation grouping
- Can determine if operations can be grouped together

#### 2. `OperationGroup` Struct

Groups related operations that should be undone/redone together:

- Tracks timestamp for time-based grouping
- Merges consecutive typing operations
- Groups operations within 500ms window

#### 3. `UndoStack` Struct

Main undo/redo manager:

```rust
pub struct UndoStack {
    undo_stack: Vec<OperationGroup>,
    redo_stack: Vec<OperationGroup>,
    limit: usize,                    // Default: 1000
    group_operations: bool,          // Default: true
}
```

**Key Methods:**
- `push()` - Add operation to undo stack
- `undo()` - Undo last operation group
- `redo()` - Redo last undone operation
- `clear()` - Clear all history
- `set_limit()` - Configure stack size
- `set_grouping()` - Enable/disable operation grouping

### Integration with TextLayout

The `TextLayout` struct now includes:

```rust
pub struct TextLayout {
    text: String,
    lines: Vec<LineBox>,
    prefix_sums: PrefixSums,
    undo_stack: UndoStack,  // New field
}
```

**New Public Methods:**

1. **Undo/Redo Operations:**
   - `undo()` - Undo last operation, returns new cursor position and selection
   - `redo()` - Redo last undone operation
   - `can_undo()` - Check if undo is available
   - `can_redo()` - Check if redo is available

2. **Configuration:**
   - `clear_undo_history()` - Clear all undo/redo history
   - `set_undo_limit()` - Set maximum undo operations (default: 1000)
   - `undo_limit()` - Get current limit
   - `set_undo_grouping()` - Enable/disable operation grouping

3. **Modified Text Editing Methods:**
   - `insert_str_with_undo()` - Insert with undo tracking
   - `replace_selection()` - Now records undo operations
   - `delete_backward()` - Now records undo operations
   - `delete_selection()` - Now records undo operations

## Operation Grouping

### How It Works

Consecutive operations are automatically grouped if:

1. **Time Threshold:** Operations occur within 500ms of each other
2. **Position Adjacency:** 
   - Insertions at consecutive positions (typing)
   - Deletions at consecutive positions (backspace)

### Examples

**Typing "Hello":**
```
Insert "H" at 0
Insert "e" at 1  } Grouped into single
Insert "l" at 2  } "Hello" insertion
Insert "l" at 3  }
Insert "o" at 4
```

**Backspace:**
```
Delete "o" at 4
Delete "l" at 3  } Grouped into single
Delete "l" at 2  } "llo" deletion
```

**Not Grouped:**
- Operations with >500ms gap
- Non-adjacent positions
- Different operation types
- Grouping disabled

## Usage Examples

### Basic Undo/Redo

```rust
let font = FontFace::from_path("font.ttf", 0)?;
let mut layout = TextLayout::new("Hello", &font, 16.0);

// Insert text with undo tracking
let selection = Selection::collapsed(5);
layout.insert_str_with_undo(5, " World", selection, &font, 16.0, None, WrapMode::NoWrap);

// Undo the insertion
let current_selection = Selection::collapsed(11);
if let Some((new_cursor, new_selection)) = layout.undo(&current_selection, &font, 16.0, None, WrapMode::NoWrap) {
    println!("Undone! Cursor at: {}", new_cursor);
}

// Redo
if let Some((new_cursor, new_selection)) = layout.redo(&current_selection, &font, 16.0, None, WrapMode::NoWrap) {
    println!("Redone! Cursor at: {}", new_cursor);
}
```

### Configuring Undo Behavior

```rust
let mut layout = TextLayout::new("", &font, 16.0);

// Set a smaller undo limit for memory-constrained environments
layout.set_undo_limit(100);

// Disable operation grouping for fine-grained undo
layout.set_undo_grouping(false);

// Clear history when loading a new document
layout.clear_undo_history();
```

### Checking Undo/Redo Availability

```rust
if layout.can_undo() {
    println!("Undo available");
}

if layout.can_redo() {
    println!("Redo available");
}
```

## Implementation Details

### Operation Recording

Text modification methods automatically record operations:

```rust
// In insert_str_with_undo()
let operation = TextOperation::Insert {
    offset,
    text: text.to_string(),
    selection_before,
    selection_after,
};
self.record_operation(operation);
```

### Undo Application

Undo reverses operations in reverse order:

```rust
for operation in operations.iter().rev() {
    match operation {
        TextOperation::Insert { offset, text, .. } => {
            // Undo insert by deleting
            self.text.replace_range(*offset..(offset + text.len()), "");
        }
        TextOperation::Delete { offset, text, .. } => {
            // Undo delete by inserting
            self.text.insert_str(*offset, text);
        }
        // ... handle Replace
    }
}
```

### Redo Application

Redo re-applies operations in forward order:

```rust
for operation in operations.iter() {
    match operation {
        TextOperation::Insert { offset, text, .. } => {
            self.text.insert_str(*offset, text);
        }
        // ... handle other operations
    }
}
```

## Testing

Comprehensive test coverage includes:

### UndoStack Tests (11 tests)
- Stack creation and basic operations
- Push and undo
- Redo functionality
- Operation grouping (enabled/disabled)
- Size limits
- Clear functionality
- Delete grouping

### TextLayout Integration Tests (10 tests)
- Undo insert operations
- Undo delete operations
- Undo replace operations
- Redo functionality
- Multiple operations
- can_undo/can_redo checks
- Clear history
- Undo limit enforcement
- Operation grouping (enabled/disabled)

**All 95 tests pass successfully.**

## Performance Characteristics

### Memory Usage

- **Per Operation:** ~100-200 bytes (depends on text length)
- **Default Limit:** 1000 operations ≈ 100-200 KB
- **Configurable:** Can be reduced for memory-constrained environments

### Time Complexity

- **Push:** O(1) amortized (may merge with previous operation)
- **Undo:** O(n) where n = number of operations in group
- **Redo:** O(n) where n = number of operations in group
- **Can Undo/Redo:** O(1)

### Space Optimization

- Operation grouping reduces memory usage for typing
- Size limit prevents unbounded growth
- Redo stack cleared on new operations

## Features Completed

✅ Implement undo stack data structure  
✅ Record text insertion operations  
✅ Record text deletion operations  
✅ Record selection changes  
✅ Implement undo operation  
✅ Implement redo operation  
✅ Group consecutive operations (typing)  
✅ Set undo stack size limit  
✅ Clear undo stack on major changes  

## Future Enhancements

Potential improvements for future versions:

1. **Persistent Undo:** Save undo history to disk
2. **Branching Undo:** Support undo tree instead of linear stack
3. **Selective Undo:** Undo specific operations, not just last one
4. **Undo Compression:** Compress old operations to save memory
5. **Undo Metadata:** Track timestamps, user info for collaborative editing
6. **Macro Recording:** Group arbitrary operations into named macros

## Best Practices

### When to Clear History

- Loading a new document
- Major refactoring operations
- After saving (optional)
- When switching editing modes

### Grouping Guidelines

- **Enable** for normal text editing (better UX)
- **Disable** for programmatic edits
- **Disable** for testing individual operations

### Memory Management

- Set appropriate limits based on:
  - Available memory
  - Expected document size
  - User editing patterns
- Monitor undo stack size in long-running sessions

## References

- [Text Rendering Checklist](./text-rendering-checklist.md) - Phase 6.8
- [Clipboard Implementation](./clipboard-implementation.md) - Phase 6.7
- [Command Pattern](https://en.wikipedia.org/wiki/Command_pattern) - Design pattern used
