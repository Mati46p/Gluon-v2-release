# Gluon VS Code Integration

This extension bridges Visual Studio Code with the **Gluon Desktop** application. It enables real-time synchronization of code changes applied by Gluon's AI agents.

## Features

- **Automatic File Opening**: Instantly opens files modified by Gluon in the editor.
- **Flash Effect**: Visually highlights code changes (green flash) to draw attention to modifications.
- **Seamless Connectivity**: Connects automatically to the local Gluon Desktop instance via WebSocket.

## Requirements

- **Gluon Desktop** application running locally.

## Usage

1. Start **Gluon Desktop**.
2. This extension attempts to connect automatically.
3. If connection is lost, use the command palette (`Ctrl+Shift+P`) and run `Gluon: Reconnect`.

## Release Notes

### 0.0.1
- Initial release with WebSocket bridge and Flash Effect.