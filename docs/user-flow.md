# Rune Home Flow & App Dock Specification

## 1. Overview

Rune is an **AI-native runtime**, not a conventional browser. The Home surface (powered by Peco) acts as:

- System-level hub
- App launcher
- Conversational workspace
- Router for IR Apps and URL pages
- Entry point for navigation and agentic workflows

To support efficient switching between system apps, IR apps, and browsing contexts, Rune introduces an **App Dock**, accessible via a **Home Button**.

This document defines the **UX flow**, **behavior model**, and **state machine** for the Home Tab and Dock.

---

# 2. Home Tab (Peco) Flow

### Purpose

Serve as the AI-native home screen and workspace.
This is _not_ a website and therefore has **no browser toolbar**.

### Sections

- **Conversation panel** (Peco chat)
- **Recommended actions / shortcuts**
- **IR App Launcher**
- **Recent items**
- **Home Button** (persistent UI element)

### Allowed User Actions

- Chat with Peco
- Launch IR apps
- Open URLs (Peco interprets “open xyz.com”)
- Search and run commands
- Invoke Dock (via Home Button)

### No Toolbar

- Address bar is intentionally removed here
- Navigation is mediated by Peco or Dock
- Toolbar appears only in **Browser Tabs**

---

# 3. Navigation Modes

Rune operates in three modes:

### 3.1 Home Mode

- No toolbar
- Peco-centric
- System-wide workspace
- Dock accessible

### 3.2 IR App Mode

- Displays local apps (Mail, Docs, Settings, etc.)
- No toolbar
- Within-app navigation only
- Dock accessible

### 3.3 Browser Mode

- Displays URL-based content (IR-first, fallback CEF)
- **Toolbar visible**
- Back/forward, reload, URL input
- Dock accessible

---

# 4. Home Button

A persistent control located bottom-left or bottom-center (depending on layout density).

### 4.1 Actions

#### Tap

- Always returns user to **Home Tab (Peco)**
- Minimizes all apps/tabs
- Clears modal overlays
- Does _not_ close running browser tabs or IR apps

#### Long-Press (or press-and-hold)

- Opens the **App Dock** overlay
- Does not leave current context
- Dock floats above content

#### Secondary Click (optional)

- Opens quick actions:

  - New IR App
  - New Browser Tab
  - Settings

---

# 5. App Dock Specification

The App Dock provides a unified, system-level way to access apps and active browser tabs.

## 5.1 Dock Structure

```
+----------------------------------------------------------+
|  Dock Overlay                                             |
|  ------------------------------------------------------   |
|  |  Peco | Mail | Docs | Sheets | Settings | + Add |     |
|  ------------------------------------------------------   |
|  Recent: GitHub, HN, Gmail, Splenta Docs                 |
+----------------------------------------------------------+
```

### Sections

#### 1. Pinned Apps

Manually pinned IR apps + special system apps:

- Peco (always first)
- Wisp Mail
- Wisp Docs
- Settings
- Any custom IR apps

#### 2. Active Browser Tabs

Each tab shown as:

- Favicon
- Title / domain
- Mini thumbnail (optional)

#### 3. Recent Apps / Sites

Chronologically sorted list of last opened:

- IR apps
- Browser tabs
- Documents

#### 4. Add Shortcut

Allows user to pin new IR apps or frequently visited URLs.

---

## 5.2 Dock Interactions

### Open App

- Single click → switch to that tab
- If IR app not opened → launch new IR tab

### Close App (optional)

- Right-click → “Close”
- Or swipe up on touch devices

### Reorder Apps

- Drag pinned icons to reorder

### Pin/Unpin

- Right-click → Pin / Unpin
- IR apps can be pinned like native apps

### Dismiss Dock

- Tap outside overlay
- Press Esc
- Press Home Button again
- Swipe down gesture

---

# 6. Mode Behaviors With Dock

### 6.1 In Home Mode (Peco)

- Dock opens as overlay
- Selecting an app/tab switches context
- Selecting a URL tab → opens Browser Mode (toolbar enabled)

### 6.2 In IR App Mode

- Dock opens without disrupting the app
- Selecting another IR app switches
- Selecting Peco returns to Home Mode
- Selecting a URL tab switches to Browser Mode

### 6.3 In Browser Mode

- Toolbar remains visible
- Dock opens over page
- Switching to IR app removes toolbar
- Switching to Peco returns Home

---

# 7. State Model

### States

```
HOME → IR_APP → BROWSER
 ↑        ↑        ↓
 |--------|--------|
         DOCK (overlay)
```

### Transitions

| Action                       | From             | To           |
| ---------------------------- | ---------------- | ------------ |
| Press Home Button            | IR_APP / BROWSER | HOME         |
| Long-press Home Button       | ANY              | DOCK OVERLAY |
| Select IR App from Dock      | ANY              | IR_APP       |
| Select Browser Tab from Dock | ANY              | BROWSER      |
| Select Peco from Dock        | ANY              | HOME         |
| Enter URL via Peco           | HOME             | BROWSER      |
| Open IR app from Peco        | HOME             | IR_APP       |

---

# 8. Implementation Notes

### Rendering

- Dock rendered in **rune-layout** and **rune-scene**, layered above all content.
- Standard offscreen RGBA → BGRA path applies (per GPU architecture docs).

### Platform Integration

- Dock state stored in **rune-core** reducer state.
- Pinned apps persisted using settings storage.
- Dock overlay is a normal scene rebuild (no special GPU passes).

### Interop

- IR apps are launched by routing IR request → IR translator → layout → scene.
- Browser tabs (CEF fallback) mapped as “apps” for Dock.

---

# 9. Why Dock Works For Rune

- Unifies IR apps + browser tabs
- Keeps Home clean (no toolbar)
- Creates a true “system identity” beyond Chrome clones
- Enhances Peco's role as system assistant, not search bar
- Provides clear, fast app switching
- Makes IR apps feel native
- Retains familiar mental model (iPhone + macOS hybrid)

---

# 10. Future Enhancements

- Global search integrated into Dock overlay
- Drag-and-drop document movement between apps
- Multi-window IR apps
- Right-click “Show in Dock” for websites
- GPU-accelerated animations for Dock open/close
- App groups (folders)
