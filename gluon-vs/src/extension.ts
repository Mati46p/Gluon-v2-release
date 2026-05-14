import * as vscode from 'vscode';
import WebSocket from 'ws';
import * as path from 'path';
import { ChatPanel, setOutputChannel } from './chatPanel';

let ws: WebSocket | null = null;
let reconnectInterval: NodeJS.Timeout | null = null;
let isDeactivating = false;
let outputChannel: vscode.OutputChannel;

// ============================================================================
// TERMINAL CAPTURE SYSTEM
// ============================================================================

interface TerminalConfig {
    stripAnsi: boolean;
    maxLines: number;
    autoSend: boolean;
}

// Active terminal listeners (terminal -> pairing code)
const activeTerminalListeners = new Map<vscode.Terminal, {
    pairingCode: string;
    config: TerminalConfig;
    buffer: string[];
}>();

// ============================================================================
// GLUON CHANGE TRACKING REGISTRY
// ============================================================================

interface GluonChangeRecord {
    id: string;              // UUID from browser
    batchId: string;         // Common ID for changes from one window
    filePath: string;
    oldContent: string;      // Snapshot before change
    newContent: string;      // Content after change
    timestamp: number;
    applied: boolean;
    lineStart?: number;
}

// Registry tracking all Gluon changes
const gluonChangeRegistry = new Map<string, GluonChangeRecord>();

// Separate undo/redo stacks for Gluon changes
const gluonUndoStack: string[] = [];   // changeId[]
const gluonRedoStack: string[] = [];

export function activate(context: vscode.ExtensionContext) {
    // Create Output Channel for logs
    outputChannel = vscode.window.createOutputChannel('Gluon Debug');
    context.subscriptions.push(outputChannel);

    // Share output channel with ChatPanel
    setOutputChannel(outputChannel);

    console.log('🚀 Gluon VS Code Bridge is active');
    outputChannel.appendLine('[Gluon] 🚀 VS Code Bridge is active');

    connectToGluon();

    // Register Chat Panel command
    let chatDisposable = vscode.commands.registerCommand('gluon.openChat', () => {
        outputChannel.appendLine(`[Gluon] 🎯 gluon.openChat triggered, ws state: ${ws?.readyState}`);
        ChatPanel.createOrShow(context.extensionUri, ws);

        if (ChatPanel.currentPanel && ws) {
            ChatPanel.currentPanel.updateWebSocket(ws);
        }
    });
    context.subscriptions.push(chatDisposable);

    // Track window focus to send heartbeat to backend (for workspace prioritization)
    vscode.window.onDidChangeWindowState(e => {
        if (e.focused && ws && ws.readyState === WebSocket.OPEN) {
            // Window gained focus - send heartbeat to update activity timestamp
            let roots: string[] = [];
            if (vscode.workspace.workspaceFolders) {
                roots = vscode.workspace.workspaceFolders.map(f => f.uri.fsPath);
            }

            ws.send(JSON.stringify({
                type: 'heartbeat',
                roots: roots
            }));
            console.log('[Gluon] Window focused - sent heartbeat for roots:', roots);
        }
    });

    // Rejestracja komendy do ręcznego łączenia (opcjonalnie)
    let reconnectDisposable = vscode.commands.registerCommand('gluon.reconnect', () => {
        connectToGluon();
    });
    context.subscriptions.push(reconnectDisposable);

    // Komenda: Undo last Gluon change
    let undoDisposable = vscode.commands.registerCommand('gluon.undoChange', async () => {
        await undoLastGluonChange();
    });
    context.subscriptions.push(undoDisposable);

    // Komenda: Redo last Gluon change
    let redoDisposable = vscode.commands.registerCommand('gluon.redoChange', async () => {
        await redoLastGluonChange();
    });
    context.subscriptions.push(redoDisposable);

    // Komenda: Undo specific change by ID (called from browser)
    let undoSpecificDisposable = vscode.commands.registerCommand('gluon.undoSpecificChange', async (changeId: string) => {
        await undoSpecificChange(changeId);
    });
    context.subscriptions.push(undoSpecificDisposable);

    // Komenda: Redo specific change by ID (called from browser)
    let redoSpecificDisposable = vscode.commands.registerCommand('gluon.redoSpecificChange', async (changeId: string) => {
        await redoSpecificChange(changeId);
    });
    context.subscriptions.push(redoSpecificDisposable);

    // Komenda: Capture Terminal (pair terminal with workflow node)
    let captureTerminalDisposable = vscode.commands.registerCommand('gluon.captureTerminal', async (pairingCode?: string) => {
        await captureTerminalOutput(pairingCode);
    });
    context.subscriptions.push(captureTerminalDisposable);

    // Komenda: Stop Terminal Capture
    let stopTerminalCaptureDisposable = vscode.commands.registerCommand('gluon.stopTerminalCapture', async () => {
        await stopTerminalCapture();
    });
    context.subscriptions.push(stopTerminalCaptureDisposable);

    // Komenda: Send Terminal Output (manual trigger)
    let sendTerminalOutputDisposable = vscode.commands.registerCommand('gluon.sendTerminalOutput', async () => {
        await sendTerminalOutput();
    });
    context.subscriptions.push(sendTerminalOutputDisposable);

    // Cleanup on terminal close
    vscode.window.onDidCloseTerminal(terminal => {
        activeTerminalListeners.delete(terminal);
        console.log('[Gluon Terminal] Terminal closed, removed from listeners');
    });
}

function connectToGluon() {
    if (ws && ws.readyState === WebSocket.OPEN) {
        return;
    }

    // Port Gluona
    ws = new WebSocket('ws://127.0.0.1:8743');

    ws.on('open', () => {
        console.log('Connected to Gluon');
 
        if (reconnectInterval) {
            clearInterval(reconnectInterval);
            reconnectInterval = null;
        }

        // Collect workspace roots to route requests correctly
        let roots: string[] = [];

        if (vscode.workspace.workspaceFolders) {
            roots = vscode.workspace.workspaceFolders.map(f => f.uri.fsPath);
        }

        // FALLBACK: If no workspace folder is open (Single File Mode), 
        // use the directory of the currently active file as the root.
        // This ensures the backend can still match paths to this window.
        if (roots.length === 0 && vscode.window.activeTextEditor) {
            const doc = vscode.window.activeTextEditor.document;
            if (doc.uri.scheme === 'file') {
                const fileDir = path.dirname(doc.uri.fsPath);
                roots.push(fileDir);
                console.log('[Gluon] No workspace folder, using active file dir as root:', fileDir);
            }
        }
 
        // Identyfikacja jako edytor
        ws?.send(JSON.stringify({
            type: 'identify',
            client: 'vscode',
            roots: roots
        }));
        vscode.window.setStatusBarMessage('$(plug) Gluon Connected', 3000);
    });

    // Create decoration type for the "Flash" effect (green background, fades out)
    const flashDecorationType = vscode.window.createTextEditorDecorationType({
        backgroundColor: 'rgba(0, 255, 0, 0.3)', // Visible green highlight
        rangeBehavior: vscode.DecorationRangeBehavior.OpenOpen,
    });

    ws.on('message', async (data) => {
        try {
            console.log('[Gluon] Received message:', data.toString()); // Logowanie debug
            const msg = JSON.parse(data.toString());

            if (msg.type === 'apply_edit') {
                await handleApplyEdit(msg, flashDecorationType);
            }
            else if (msg.type === 'show_changes') {
                await handleShowChanges(msg, flashDecorationType);
            }
            else if (msg.type === 'undo_change') {
                // Request from Tauri/Browser to undo specific change
                const { changeId } = msg;
                console.log(`[Gluon] Received UNDO request for: ${changeId}`);
                await undoSpecificChange(changeId);
            }
            else if (msg.type === 'redo_change') {
                // Request from Tauri/Browser to redo specific change
                const { changeId } = msg;
                await redoSpecificChange(changeId);
            }
            // Przekazywanie wiadomości dla interfejsu Chat Panel (skonfigurowane z v3 dla v2)
            else if (
                msg.type === 'chat_response' || msg.type === 'status' || msg.type === 'error' ||
                msg.type === 'question_for_user' || msg.type === 'plan_proposal' || 
                msg.type === 'waiting_for_input' || msg.type === 'budget_alert' || 
                msg.type === 'system_notification'
            ) {
                if (ChatPanel.currentPanel) {
                    ChatPanel.currentPanel.handleBackendResponse(msg);
                }
            } 
            else if (
                msg.type === 'thinking' || msg.type === 'token' || msg.type === 'done' ||
                msg.type === 'tool_call_start' || msg.type === 'tool_call_result' || 
                msg.type === 'debug_log'
            ) {
                if (ChatPanel.currentPanel) {
                    ChatPanel.currentPanel.forwardMessage(msg);
                }
            }
        } catch (e: any) {
            console.error('[Gluon] Failed to parse/handle message', e);
            if (outputChannel) outputChannel.appendLine(`[Gluon] ❌ Error handling msg: ${e.message}`);
        }
    });

    ws.on('close', () => {
        console.log('Disconnected from Gluon');
        
        if (isDeactivating) {
            return;
        }
 
        // Auto-reconnect
        if (!reconnectInterval) {
            reconnectInterval = setInterval(connectToGluon, 5000);
        }
    });

    ws.on('error', (err) => {
        // Silent fail on connection error loop
    });
}

/**
 * Helper to resolve URI from path string (handles relative and absolute paths).
 */
function resolveUri(filePath: string): vscode.Uri {
    // Check if absolute
    if (filePath.startsWith('/') || /^[a-zA-Z]:/.test(filePath)) {
        return vscode.Uri.file(filePath);
    }

    // Try to find in workspace folders
    if (vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders.length > 0) {
        // Assume relative to the first workspace folder for simplicity
        // In multi-root workspaces, this might need refinement (searching for file existence)
        return vscode.Uri.joinPath(vscode.workspace.workspaceFolders[0].uri, filePath);
    }

    // Fallback
    return vscode.Uri.file(filePath);
}

async function handleApplyEdit(msg: any, decorationType: vscode.TextEditorDecorationType) {
    const { id, filePath, content, changeId, batchId, oldContent, lineStart } = msg;

    try {
        const uri = resolveUri(filePath);
        console.log(`[Gluon] Apply Edit to: ${uri.fsPath} (changeId: ${changeId || 'none'})`);

        const document = await vscode.workspace.openTextDocument(uri);

        // Capture old content BEFORE applying change (for undo)
        const currentContent = document.getText();

        const fullRange = new vscode.Range(
            document.positionAt(0),
            document.positionAt(document.getText().length)
        );

        const edit = new vscode.WorkspaceEdit();
        edit.replace(uri, fullRange, content);

        const success = await vscode.workspace.applyEdit(edit);

        if (success) {
            await document.save();
            // Register change in Gluon registry for undo/redo
            if (changeId) {
                const record: GluonChangeRecord = {
                    id: changeId,
                    batchId: batchId || changeId, // Use changeId as fallback
                    filePath: uri.fsPath,
                    oldContent: oldContent || currentContent, // Prefer passed oldContent, fallback to current
                    newContent: content,
                    timestamp: Date.now(),
                    applied: true,
                    lineStart: lineStart
                };

                gluonChangeRegistry.set(changeId, record);
                gluonUndoStack.push(changeId);

                // Clear redo stack when new change is applied
                gluonRedoStack.length = 0;

                console.log(`[Gluon] Registered change ${changeId} in registry (undo stack size: ${gluonUndoStack.length})`);
            } else {
                console.warn('[Gluon] Warning: Change applied WITHOUT changeId - Undo will not work for this edit!');
            }

            // ALSO SHOW THE DOCUMENT (Fix for "doesn't open file")
            const editor = await vscode.window.showTextDocument(document, {
                preview: false,
                preserveFocus: false
            });

            // Flash the whole file briefly to indicate "Massive Change"
            editor.setDecorations(decorationType, [fullRange]);
            setTimeout(() => editor.setDecorations(decorationType, []), 1000);

            sendResponse(id, true);
        } else {
            sendResponse(id, false, "VS Code rejected the edit");
        }

    } catch (e: any) {
        console.error('[Gluon] ApplyEdit Error:', e);
        sendResponse(id, false, e.message);
    }
}

async function handleShowChanges(msg: any, decorationType: vscode.TextEditorDecorationType) {
    const files = msg.files; // Array of { path, ranges: [{start_line, end_line}] }

    for (const file of files) {
        try {
            // Normalizacja ścieżki przed logowaniem i otwarciem
            const cleanPath = file.path.replace(/\\/g, '/'); 
            const uri = resolveUri(cleanPath);
            
            console.log(`[Gluon] Request to open: ${file.path} -> Resolved URI: ${uri.toString()}`);

            const doc = await vscode.workspace.openTextDocument(uri);
            const editor = await vscode.window.showTextDocument(doc, {
                preview: false,
                preserveFocus: false // Force focus
            });

            const ranges = file.ranges.map((r: any) => new vscode.Range(
                new vscode.Position(r.start_line, 0),
                new vscode.Position(r.end_line, 0)
            ));

            if (ranges.length === 0) {
                console.log('[Gluon] No ranges provided for file, skipping highlight.');
                continue;
            }

            editor.setDecorations(decorationType, ranges);

            // Scroll to the first change
            editor.revealRange(ranges[0], vscode.TextEditorRevealType.InCenter);

            // Cleanup decoration
            setTimeout(() => {
                editor.setDecorations(decorationType, []);
            }, 8000);

        } catch (err) {
            console.error(`[Gluon] Failed to open/highlight file: ${file.path}`, err);
        }
    }
}

function sendResponse(id: string, success: boolean, error?: string) {
    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({
            type: 'edit_result',
            id,
            success,
            error
        }));
    }
}

// ============================================================================
// UNDO/REDO FUNCTIONALITY
// ============================================================================

/**
 * Undo last Gluon change (from undo stack)
 */
async function undoLastGluonChange(): Promise<void> {
    if (gluonUndoStack.length === 0) {
        vscode.window.showInformationMessage('No Gluon changes to undo');
        return;
    }

    const changeId = gluonUndoStack.pop()!;
    await undoSpecificChange(changeId);
}

/**
 * Redo last undone Gluon change (from redo stack)
 */
async function redoLastGluonChange(): Promise<void> {
    if (gluonRedoStack.length === 0) {
        vscode.window.showInformationMessage('No Gluon changes to redo');
        return;
    }

    const changeId = gluonRedoStack.pop()!;
    await redoSpecificChange(changeId);
}

/**
 * Undo specific change by ID (called from browser or command palette)
 */
async function undoSpecificChange(changeId: string): Promise<void> {
    const record = gluonChangeRegistry.get(changeId);

    if (!record) {
        vscode.window.showErrorMessage(`Change ${changeId} not found in registry`);
        console.error(`[Gluon] Change ${changeId} not found in registry`);
        return;
    }

    if (!record.applied) {
        vscode.window.showInformationMessage('Change already undone');
        return;
    }

    try {
        const uri = resolveUri(record.filePath);
        console.log(`[Gluon] Undoing change ${changeId} in ${uri.fsPath}`);

        const document = await vscode.workspace.openTextDocument(uri);
        const fullRange = new vscode.Range(
            document.positionAt(0),
            document.positionAt(document.getText().length)
        );

        const edit = new vscode.WorkspaceEdit();
        edit.replace(uri, fullRange, record.oldContent);

        const success = await vscode.workspace.applyEdit(edit);

        if (success) {
            await document.save();
            record.applied = false;

            // Add to redo stack (remove from undo stack if it was there)
            const undoIndex = gluonUndoStack.indexOf(changeId);
            if (undoIndex !== -1) {
                gluonUndoStack.splice(undoIndex, 1);
            }
            gluonRedoStack.push(changeId);

            // Notify Tauri → Browser
            sendToTauri({
                type: 'change_undone',
                changeId: changeId,
                batchId: record.batchId
            });

            vscode.window.showInformationMessage(`✓ Undone: ${path.basename(record.filePath)}`);
            console.log(`[Gluon] Successfully undone change ${changeId}`);
        } else {
            vscode.window.showErrorMessage('VS Code rejected the undo operation');
        }
    } catch (e: any) {
        console.error('[Gluon] Undo Error:', e);
        vscode.window.showErrorMessage(`Undo failed: ${e.message}`);
    }
}

/**
 * Redo specific change by ID (called from browser or command palette)
 */
async function redoSpecificChange(changeId: string): Promise<void> {
    const record = gluonChangeRegistry.get(changeId);

    if (!record) {
        vscode.window.showErrorMessage(`Change ${changeId} not found in registry`);
        console.error(`[Gluon] Change ${changeId} not found in registry`);
        return;
    }

    if (record.applied) {
        vscode.window.showInformationMessage('Change already applied');
        return;
    }

    try {
        const uri = resolveUri(record.filePath);
        console.log(`[Gluon] Redoing change ${changeId} in ${uri.fsPath}`);

        const document = await vscode.workspace.openTextDocument(uri);
        const fullRange = new vscode.Range(
            document.positionAt(0),
            document.positionAt(document.getText().length)
        );

        const edit = new vscode.WorkspaceEdit();
        edit.replace(uri, fullRange, record.newContent);

        const success = await vscode.workspace.applyEdit(edit);

        if (success) {
            await document.save();
            record.applied = true;

            // Add to undo stack (remove from redo stack if it was there)
            const redoIndex = gluonRedoStack.indexOf(changeId);
            if (redoIndex !== -1) {
                gluonRedoStack.splice(redoIndex, 1);
            }
            gluonUndoStack.push(changeId);

            // Notify Tauri → Browser
            sendToTauri({
                type: 'change_redone',
                changeId: changeId,
                batchId: record.batchId
            });

            vscode.window.showInformationMessage(`✓ Redone: ${path.basename(record.filePath)}`);
            console.log(`[Gluon] Successfully redone change ${changeId}`);
        } else {
            vscode.window.showErrorMessage('VS Code rejected the redo operation');
        }
    } catch (e: any) {
        console.error('[Gluon] Redo Error:', e);
        vscode.window.showErrorMessage(`Redo failed: ${e.message}`);
    }
}

/**
 * Send message to Tauri backend
 */
function sendToTauri(message: any): void {
    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify(message));
        console.log('[Gluon] Sent to Tauri:', message);
    } else {
        console.warn('[Gluon] Cannot send to Tauri - WebSocket not connected');
    }
}

// ============================================================================
// TERMINAL CAPTURE IMPLEMENTATION
// ============================================================================

/**
 * Captures terminal output and pairs it with a workflow node
 */
async function captureTerminalOutput(pairingCode?: string): Promise<void> {
    const terminal = vscode.window.activeTerminal;

    if (!terminal) {
        vscode.window.showErrorMessage('No active terminal found. Please open a terminal first.');
        return;
    }

    // Ask for pairing code if not provided
    let code = pairingCode;
    if (!code) {
        code = await vscode.window.showInputBox({
            prompt: 'Enter Terminal Node pairing code (e.g., #TERM1)',
            placeHolder: '#TERM1',
            validateInput: (value) => {
                if (!value || !value.startsWith('#')) {
                    return 'Pairing code must start with #';
                }
                return null;
            }
        });
    }

    if (!code) {
        return; // User cancelled
    }

    // Ask for configuration
    const stripAnsi = await vscode.window.showQuickPick(['Yes', 'No'], {
        placeHolder: 'Remove ANSI color codes?'
    });

    const maxLinesInput = await vscode.window.showInputBox({
        prompt: 'Maximum lines to capture (0 = unlimited)',
        value: '200',
        validateInput: (value) => {
            const num = parseInt(value);
            if (isNaN(num) || num < 0) {
                return 'Please enter a valid number';
            }
            return null;
        }
    });

    const maxLines = parseInt(maxLinesInput || '200');

    const config: TerminalConfig = {
        stripAnsi: stripAnsi === 'Yes',
        maxLines: maxLines,
        autoSend: true
    };

    // Register terminal listener
    activeTerminalListeners.set(terminal, {
        pairingCode: code,
        config: config,
        buffer: []
    });

    // Send pairing handshake to backend
    sendToTauri({
        type: 'terminal_paired',
        pairing_code: code,
        terminal_name: terminal.name
    });

    vscode.window.showInformationMessage(`✓ Terminal "${terminal.name}" paired with ${code}`);
    console.log(`[Gluon Terminal] Paired terminal "${terminal.name}" with ${code}`);

    // Start capturing by creating a custom terminal with output monitoring
    // Note: VS Code API doesn't provide direct access to terminal output
    // This is a limitation - we'll use a workaround with terminal tasks
    startTerminalMonitoring(terminal, code, config);
}

/**
 * Stop capturing terminal output
 */
async function stopTerminalCapture(): Promise<void> {
    const terminal = vscode.window.activeTerminal;

    if (!terminal) {
        vscode.window.showErrorMessage('No active terminal found.');
        return;
    }

    const listener = activeTerminalListeners.get(terminal);
    if (!listener) {
        vscode.window.showInformationMessage('Terminal is not being captured.');
        return;
    }

    activeTerminalListeners.delete(terminal);

    // Notify backend
    sendToTauri({
        type: 'terminal_unpaired',
        pairing_code: listener.pairingCode
    });

    vscode.window.showInformationMessage(`✓ Stopped capturing terminal "${terminal.name}"`);
    console.log(`[Gluon Terminal] Stopped capturing terminal "${terminal.name}"`);
}

/**
 * Start monitoring terminal (workaround for limited VS Code API)
 *
 * NOTE: VS Code doesn't provide direct access to terminal output stream.
 * This implementation uses a polling approach with clipboard workaround.
 */
function startTerminalMonitoring(terminal: vscode.Terminal, pairingCode: string, config: TerminalConfig): void {
    console.log(`[Gluon Terminal] Started monitoring for ${pairingCode}`);

    // Store metadata for manual send command
    // In real implementation, you might use terminal tasks or custom pseudoterminal
    // For now, we'll provide a manual "Send Terminal Output" command

    vscode.window.showInformationMessage(
        `Terminal monitoring started. Use "Gluon: Send Terminal Output" to send current terminal content.`,
        'Configure'
    ).then(selection => {
        if (selection === 'Configure') {
            // Show configuration options
            vscode.window.showInformationMessage(
                `Config: Strip ANSI: ${config.stripAnsi}, Max Lines: ${config.maxLines}`
            );
        }
    });
}

/**
 * Manual command to send current terminal output
 * (Workaround for VS Code terminal API limitations)
 */
async function sendTerminalOutput(): Promise<void> {
    const terminal = vscode.window.activeTerminal;
    if (!terminal) {
        vscode.window.showErrorMessage('No active terminal found.');
        return;
    }

    const listener = activeTerminalListeners.get(terminal);
    if (!listener) {
        vscode.window.showErrorMessage('Terminal is not paired. Use "Gluon: Capture Terminal" first.');
        return;
    }

    // Ask user to select all and copy terminal content
    const proceed = await vscode.window.showInformationMessage(
        'Please select all terminal content (Ctrl+A) and copy (Ctrl+C), then click OK.',
        'OK', 'Cancel'
    );

    if (proceed !== 'OK') {
        return;
    }

    // Read from clipboard
    const clipboardContent = await vscode.env.clipboard.readText();

    if (!clipboardContent) {
        vscode.window.showErrorMessage('No content found in clipboard.');
        return;
    }

    // Process content
    let processedContent = clipboardContent;

    // Strip ANSI codes if configured
    if (listener.config.stripAnsi) {
        processedContent = stripAnsiCodes(processedContent);
    }

    // Limit lines
    const lines = processedContent.split('\n');
    if (listener.config.maxLines > 0 && lines.length > listener.config.maxLines) {
        processedContent = lines.slice(-listener.config.maxLines).join('\n');
    }

    // Send to backend
    sendToTauri({
        type: 'terminal_output',
        pairing_code: listener.pairingCode,
        content: processedContent,
        terminal_name: terminal.name
    });

    vscode.window.showInformationMessage(`✓ Sent ${lines.length} lines to Gluon`);
    console.log(`[Gluon Terminal] Sent terminal output (${lines.length} lines)`);
}

/**
 * Strip ANSI escape codes from text
 */
function stripAnsiCodes(text: string): string {
    // Regex to match ANSI escape sequences
    const ansiRegex = /\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])/g;
    return text.replace(ansiRegex, '');
}

export function deactivate() {
    isDeactivating = true;
    if (ws) ws.close();
    if (reconnectInterval) clearInterval(reconnectInterval);
    activeTerminalListeners.clear();
}