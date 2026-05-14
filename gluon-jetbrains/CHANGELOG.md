# Changelog - Gluon JetBrains Plugin

All notable changes to this project will be documented in this file.

## [1.0.0] - 2026-02-15

### 🎉 Initial Release

#### ✅ Added
- **WebSocket Integration** - Bi-directional communication with Gluon Desktop App (ws://127.0.0.1:8743)
- **Apply System** - Apply AI-generated code changes directly to files
- **Code Highlighting** - Visual green background highlighting for changed code blocks
- **Undo/Redo Functionality** - Full snapshot-based undo/redo for individual changes
- **User Notifications** - Balloon notifications for connection status and operations
- **Auto-Reconnect** - Automatic reconnection to Desktop App with 5 retry attempts
- **Editor Registration** - Automatic registration with Gluon Desktop on project open
- **Progress Tracking** - Real-time progress updates (logged, not yet in UI)

#### 📦 Components
- `GluonProjectService.kt` - Main service managing all Gluon operations
- `GluonStartupActivity.kt` - Auto-initialization on project open
- `GluonProtocol.kt` - WebSocket protocol data classes
- `GluonNotifications.kt` - User notification system

#### 🔌 Protocol Support
Implements WebSocket protocol compatible with:
- Gluon Desktop App (Rust backend)
- VSCode Extension (JavaScript)

#### 📋 Supported Messages
**FROM Desktop:**
- `apply_code_changes` - Apply code modifications
- `apply_progress_update` - Real-time progress
- `change_status_update` - Status sync
- `show_changes` - Highlight code
- `undo_change` - Undo operation
- `redo_change` - Redo operation

**TO Desktop:**
- `register_editor` - Register as active editor
- `change_status_update` - Operation results
- `heartbeat` - Keep-alive

#### 🎯 Compatibility
- **IntelliJ Platform**: 2023.2+ (build 232)
- **Until Build**: 242.* (2024.2)
- **JDK**: 17+
- **Kotlin**: 1.9.22

#### 📚 Dependencies
- Java-WebSocket 1.5.4
- Gson 2.10.1
- IntelliJ Platform SDK

---

## [Unreleased]

### 🚧 Planned Features

#### High Priority
- [ ] Progress bar UI widget (currently logged only)
- [ ] Gutter icons for change markers
- [ ] Settings panel for WebSocket port configuration
- [ ] Keyboard shortcuts for Apply/Undo/Redo

#### Medium Priority
- [ ] Diff view panel (similar to VSCode overlay)
- [ ] Batch operations UI
- [ ] Persistent snapshots (survive IDE restart)
- [ ] Change preview before apply

#### Low Priority
- [ ] Conflict resolution UI
- [ ] Multi-file change grouping
- [ ] Export/import change history
- [ ] Dark/Light theme customization for highlights

---

## Version History

### Version Numbering
- **Major.Minor.Patch** (Semantic Versioning)
- **Major**: Breaking changes to protocol or API
- **Minor**: New features, backwards compatible
- **Patch**: Bug fixes, minor improvements

### Changelog Categories
- **Added**: New features
- **Changed**: Changes to existing functionality
- **Deprecated**: Soon-to-be removed features
- **Removed**: Removed features
- **Fixed**: Bug fixes
- **Security**: Security improvements

---

## Migration Guide

### From VSCode Extension

The JetBrains plugin uses the **same WebSocket protocol** as VSCode:
- No changes needed in Gluon Desktop App
- Both can run simultaneously
- Changes from Desktop apply to whichever editor has the file open

### Differences from VSCode

| Feature | VSCode | JetBrains |
|---------|--------|-----------|
| Protocol | ✅ Same | ✅ Same |
| Apply Changes | ✅ | ✅ |
| Undo/Redo | Desktop App | Local Snapshots |
| Highlighting | DOM/CSS | RangeHighlighter API |
| Progress UI | Overlay | Logged (UI planned) |
| Diff View | Full Overlay | Planned |

---

## Known Issues

### v1.0.0

1. **Progress not shown in UI**
   - Progress updates are logged but not displayed
   - Workaround: Check IDE logs or wait for completion notification

2. **Snapshots not persistent**
   - Undo history lost on IDE restart
   - Workaround: Commit changes to VCS regularly

3. **No conflict detection**
   - If file changed externally, apply may conflict
   - Workaround: VCS integration handles conflicts

4. **Gradle wrapper not included**
   - Must use IntelliJ IDEA or install Gradle manually
   - Workaround: See BUILD_INSTRUCTIONS.md

---

## Credits

- **Architecture**: Based on Gluon VSCode extension
- **Protocol**: Designed to match Rust backend (`editor_bridge.rs`)
- **Implementation**: Kotlin + IntelliJ Platform SDK
- **Date**: February 15, 2026

---

## License

[Your License Here]
