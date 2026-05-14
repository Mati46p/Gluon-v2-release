# Gluon JetBrains Plugin

Official Gluon integration for JetBrains IDEs (IntelliJ IDEA, WebStorm, PyCharm, etc.)

## ✨ Features

- **✓ WebSocket Integration** - Real-time bi-directional communication with Gluon Desktop App
- **✓ Apply System** - Apply AI-generated code changes directly to your files
- **✓ Code Highlighting** - Visual indicators for changed code blocks
- **✓ Undo/Redo** - Full snapshot-based undo/redo functionality
- **✓ Smart Notifications** - User-friendly notifications for all operations

## 🏗️ Architecture

The plugin is based on the VSCode extension architecture and implements the same WebSocket protocol used by Gluon Desktop App.

### Components

1. **GluonProjectService.kt** - Main service managing WebSocket connection and file operations
2. **GluonProtocol.kt** - Data classes matching the Gluon Desktop WebSocket protocol
3. **GluonNotifications.kt** - User notification system
4. **GluonStartupActivity.kt** - Auto-initialization on project open

### Communication Flow

```
JetBrains IDE <-> WebSocket (ws://127.0.0.1:8743) <-> Gluon Desktop App
```

## 📦 Building

### Prerequisites

- JDK 17 or higher
- Gradle 8.x

### Build Steps

1. **Initialize Gradle Wrapper** (first time only):
```bash
cd gluon-jetbrains
gradle wrapper
```

2. **Build the plugin**:
```bash
./gradlew build
```

3. **Run in IDE sandbox** (for testing):
```bash
./gradlew runIde
```

The built plugin will be in `build/distributions/Gluon-1.0-SNAPSHOT.zip`

## 🔧 Installation

### From Disk

1. Open your JetBrains IDE
2. Go to **Settings/Preferences** → **Plugins**
3. Click the **⚙️** icon → **Install Plugin from Disk...**
4. Select the built `.zip` file from `build/distributions/`
5. Restart the IDE

### From Marketplace (future)

Once published, install directly from the JetBrains Plugin Marketplace.

## 🚀 Usage

1. **Start Gluon Desktop App** - Ensure it's running and listening on port 8743
2. **Open a project in your JetBrains IDE**
3. **Plugin auto-connects** - Look for "Gluon Connected" notification
4. **Use Gluon** - Changes from Gluon Desktop will automatically apply to your files

### Supported Operations

- **Apply Code Changes** - Received changes are automatically applied with highlighting
- **Undo Changes** - Reverts applied changes (per-file snapshot-based)
- **Redo Changes** - Re-applies undone changes
- **Show Changes** - Highlights modified code blocks in the editor

## 🔌 Protocol Compatibility

This plugin implements the same WebSocket protocol as the VSCode extension:

### Messages FROM Desktop TO Editor

- `apply_code_changes` - Apply code modifications
- `apply_progress_update` - Real-time progress updates
- `change_status_update` - Status synchronization
- `show_changes` - Highlight code blocks
- `undo_change` - Undo a specific change
- `redo_change` - Redo a specific change

### Messages FROM Editor TO Desktop

- `register_editor` - Register JetBrains as active editor
- `change_status_update` - Notify Desktop of operation results
- `heartbeat` - Keep-alive messages

## 🐛 Debugging

Enable debug logging by adding this to your IDE's VM options:

```
-Didea.log.debug.categories=#com.gluon
```

Logs will appear in:
- **macOS/Linux**: `~/.local/share/JetBrains/<IDE>/log/idea.log`
- **Windows**: `%APPDATA%\JetBrains\<IDE>\log\idea.log`

## 📄 File Structure

```
gluon-jetbrains/
├── src/main/
│   ├── kotlin/com/gluon/
│   │   ├── GluonProjectService.kt     # Main service (WebSocket, Apply, Undo/Redo)
│   │   ├── GluonStartupActivity.kt    # Auto-initialization
│   │   ├── GluonProtocol.kt           # Protocol data classes
│   │   └── GluonNotifications.kt      # Notification system
│   └── resources/META-INF/
│       └── plugin.xml                  # Plugin configuration
├── build.gradle.kts                    # Build configuration
└── README.md                           # This file
```

## 🔐 Dependencies

- **org.java-websocket:Java-WebSocket:1.5.4** - WebSocket client
- **com.google.code.gson:gson:2.10.1** - JSON serialization
- **IntelliJ Platform SDK** - IDE integration APIs

## 🎯 Compatibility

- **IntelliJ Platform**: 2023.2+ (build 232+)
- **Until Build**: 242.* (2024.2)
- **JVM Target**: Java 17

Compatible with:
- IntelliJ IDEA (Community & Ultimate)
- WebStorm
- PyCharm
- PhpStorm
- GoLand
- RubyMine
- CLion
- Rider
- And all other JetBrains IDEs

## 🤝 Contributing

This plugin is part of the Gluon project. For issues and feature requests, please visit:
https://github.com/your-org/gluon

## 📝 Implementation Notes

### Code Highlighting

The plugin uses IntelliJ's `RangeHighlighter` API to visually mark changed code:
- **Green background** (rgba(46, 160, 67, 0.15)) for added/modified lines
- Highlights are cleared when changes are undone
- Automatic scroll-to-change when files are opened

### Undo/Redo System

Unlike VSCode which relies on Desktop App for undo:
- **Local snapshots** stored in `ChangeSnapshot` objects
- **Full file content** saved before each change
- **Per-change granularity** - undo individual changes, not just entire batches
- Snapshots stored in memory (cleared on IDE restart)

### Auto-Reconnect

The plugin automatically attempts to reconnect to Desktop App:
- **Max attempts**: 5
- **Delay**: 2 seconds between attempts
- **User notification** on connection loss

## 📊 Status

### ✅ Implemented

- [x] WebSocket client with auto-reconnect
- [x] Editor registration with Desktop App
- [x] Apply code changes functionality
- [x] Code highlighting (green background for changes)
- [x] Undo/Redo with local snapshots
- [x] User notifications for all operations
- [x] Progress tracking (logged, not yet shown in UI)
- [x] Error handling and reporting

### 🚧 Future Enhancements

- [ ] Progress bar UI (currently logged only)
- [ ] Gutter icons for change markers
- [ ] Diff view panel (similar to VSCode overlay)
- [ ] Batch operations UI
- [ ] Settings panel for WebSocket configuration
- [ ] Persistent snapshots (survive IDE restart)

## 📜 License

[Your License Here]

## 🙏 Credits

Based on the Gluon VSCode extension implementation.
WebSocket protocol designed to match Rust backend (`editor_bridge.rs`).
