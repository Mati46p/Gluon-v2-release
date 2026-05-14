package com.gluon

import com.google.gson.Gson
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.command.CommandProcessor
import com.intellij.openapi.command.UndoConfirmationPolicy
import com.intellij.openapi.components.Service
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.editor.markup.HighlighterLayer
import com.intellij.openapi.editor.markup.HighlighterTargetArea
import com.intellij.openapi.editor.markup.RangeHighlighter
import com.intellij.openapi.editor.markup.TextAttributes
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.ui.JBColor
import org.java_websocket.client.WebSocketClient
import org.java_websocket.handshake.ServerHandshake
import java.awt.Color
import java.net.URI
import java.util.*
import java.util.concurrent.ConcurrentHashMap
import kotlin.concurrent.schedule
import com.intellij.openapi.Disposable

/**
 * Gluon Project Service - Main integration service for JetBrains IDEs
 *
 * This service manages:
 * - WebSocket connection to Gluon Desktop App (ws://127.0.0.1:8743)
 * - File editing operations (Apply functionality)
 * - Code highlighting and decorations
 * - Undo/Redo with snapshot management
 *
 * Architecture based on VSCode extension implementation.
 */
 @Service(Service.Level.PROJECT)
 class GluonProjectService(private val project: Project) : Disposable {

    private val logger = Logger.getInstance(GluonProjectService::class.java)
    private val gson = Gson()

    // WebSocket connection
    private var webSocketClient: GluonWebSocketClient? = null
    private var reconnectAttempts = 0
    private val maxReconnectAttempts = 5
    private val reconnectDelay = 2000L

    // Pending requests (request_id -> callback)
    private val pendingRequests = ConcurrentHashMap<String, (JsonObject) -> Unit>()

    // Code highlighting (file_path -> List<RangeHighlighter>)
    private val activeHighlighters = ConcurrentHashMap<String, MutableList<RangeHighlighter>>()

    init {
        logger.info("[Gluon] Initializing GluonProjectService for project: ${project.name}")
        connect()
    }

    // ========================================================================
    // WebSocket Connection Management
    // ========================================================================

    private fun connect() {
        if (webSocketClient != null && webSocketClient!!.isOpen) {
            logger.info("[Gluon] WebSocket already connected")
            return
        }

        try {
            logger.info("[Gluon] Connecting to Desktop App at ws://127.0.0.1:8743")
            webSocketClient = GluonWebSocketClient(URI("ws://127.0.0.1:8743"))
            webSocketClient!!.connect()
        } catch (e: Exception) {
            logger.error("[Gluon] Failed to connect to Desktop App", e)
            scheduleReconnect()
        }
    }

    private fun scheduleReconnect() {
        if (reconnectAttempts >= maxReconnectAttempts) {
            logger.warn("[Gluon] Max reconnect attempts reached. Giving up.")
            return
        }

        reconnectAttempts++
        logger.info("[Gluon] Scheduling reconnect attempt $reconnectAttempts/$maxReconnectAttempts in ${reconnectDelay}ms")

        Timer().schedule(reconnectDelay) {
            connect()
        }
    }

    /**
     * Force reconnect to Gluon Desktop App (resets attempt counter)
     * Called manually from ReconnectGluonAction or when user wants to retry
     */
    fun forceReconnect() {
        logger.info("[Gluon] Force reconnect initiated by user")
        reconnectAttempts = 0

        // Close existing connection if open
        if (webSocketClient != null && webSocketClient!!.isOpen) {
            try {
                webSocketClient!!.close()
            } catch (e: Exception) {
                logger.warn("[Gluon] Error closing existing connection", e)
            }
        }

        // Clear the reference so connect() will create a new client
        webSocketClient = null

        // Attempt to connect immediately
        connect()
    }

    // ========================================================================
    // WebSocket Client Implementation
    // ========================================================================

    private inner class GluonWebSocketClient(serverUri: URI) : WebSocketClient(serverUri) {

        override fun onOpen(handshakedata: ServerHandshake?) {
            logger.info("[Gluon] ✓ WebSocket Connected to Desktop App")
            reconnectAttempts = 0

            // Notify user
            GluonNotifications.notifyConnected(project)

            // Register this editor with Desktop
            registerEditor()
        }

        override fun onMessage(message: String?) {
            if (message == null) return

            try {
                val response = JsonParser.parseString(message).asJsonObject
                val type = response.get("type")?.asString ?: response.get("action")?.asString
                val requestId = response.get("request_id")?.asString

                logger.debug("[Gluon] Received message: type=$type, request_id=$requestId")

                // Handle pending request callbacks
                if (requestId != null && pendingRequests.containsKey(requestId)) {
                    val callback = pendingRequests.remove(requestId)
                    callback?.invoke(response)
                }

                // Route message based on type
                when (type) {
                    "apply_edit", "apply_code_changes" -> handleApplyEdit(response)
                    "show_changes" -> handleShowChanges(response)
                    else -> logger.debug("[Gluon] Unhandled type/action: $type")
                }

            } catch (e: Exception) {
                logger.error("[Gluon] Error processing message", e)
            }
        }

        override fun onClose(code: Int, reason: String?, remote: Boolean) {
            logger.warn("[Gluon] WebSocket closed: code=$code, reason=$reason, remote=$remote")
            GluonNotifications.notifyDisconnected(project)
            scheduleReconnect()
        }

        override fun onError(ex: Exception?) {
            logger.error("[Gluon] WebSocket error", ex)
        }
    }

    // ========================================================================
    // Editor Registration
    // ========================================================================

    private fun registerEditor() {
        val projectRoots = listOf(project.basePath ?: "")

        val registerMessage = mapOf(
            "type" to "identify",
            "client" to "jetbrains",
            "roots" to projectRoots
        )

        sendMessage(registerMessage)
    }

    // ========================================================================
    // Message Sending
    // ========================================================================

    private fun sendMessage(message: Map<String, Any>) {
        try {
            val json = gson.toJson(message)
            webSocketClient?.send(json)
            logger.debug("[Gluon] Sent message: $json")
        } catch (e: Exception) {
            logger.error("[Gluon] Failed to send message", e)
        }
    }

    private fun sendMessageWithCallback(message: Map<String, Any>, callback: (JsonObject) -> Unit) {
        val requestId = UUID.randomUUID().toString()
        val messageWithId = message.toMutableMap()
        messageWithId["request_id"] = requestId

        pendingRequests[requestId] = callback
        sendMessage(messageWithId)

        // Timeout after 30 seconds
        Timer().schedule(30000L) {
            if (pendingRequests.remove(requestId) != null) {
                logger.warn("[Gluon] Request timeout: $requestId")
            }
        }
    }

    // ========================================================================
    // Apply Code Changes Handler
    // ========================================================================

    private fun handleApplyEdit(response: JsonObject) {
        val id = response.get("id")?.asString ?: UUID.randomUUID().toString()
        val filePath = response.get("filePath")?.asString ?: response.get("file_path")?.asString
        val newCode = response.get("content")?.asString ?: response.get("new_content")?.asString ?: response.get("newCode")?.asString
        val lineStart = if (response.has("lineStart") && !response.get("lineStart").isJsonNull) response.get("lineStart").asInt else null

        if (filePath == null || newCode == null) {
            logger.error("[Gluon] Invalid change: missing filePath or content")
            sendEditResult(id, false, "Invalid change: missing filePath or content")
            return
        }

        ApplicationManager.getApplication().invokeLater {
            try {
                val virtualFile = LocalFileSystem.getInstance().findFileByPath(filePath)
                if (virtualFile == null) {
                    logger.error("[Gluon] File not found: $filePath")
                    sendEditResult(id, false, "File not found")
                    return@invokeLater
                }

                // Open file in editor immediately (before applying changes)
                FileEditorManager.getInstance(project).openFile(virtualFile, true)

                val document = FileDocumentManager.getInstance().getDocument(virtualFile)
                if (document == null) {
                    logger.error("[Gluon] Could not get document for: $filePath")
                    sendEditResult(id, false, "Could not open file")
                    return@invokeLater
                }

                val currentContent = document.text

                // Execute change through CommandProcessor for proper Undo/Redo support
                CommandProcessor.getInstance().executeCommand(
                    project,
                    {
                        ApplicationManager.getApplication().runWriteAction {
                            document.setText(newCode)
                            FileDocumentManager.getInstance().saveDocument(document)
                        }
                    },
                    "Gluon: Apply Code Changes",
                    null,
                    UndoConfirmationPolicy.DEFAULT
                )

                // Provide json object for highlighting
                val changeJson = JsonObject().apply {
                    if (lineStart != null) addProperty("lineStart", lineStart)
                    addProperty("newCode", newCode)
                }

                // Open file in editor and highlight changes
                openAndHighlightFile(virtualFile, changeJson, currentContent)

                // Notify Desktop of success
                sendEditResult(id, true, null)

                // Notify user
                val fileName = virtualFile.name
                GluonNotifications.notifyChangeApplied(project, fileName)

                logger.info("[Gluon] ✓ Applied change to: $filePath")

            } catch (e: Exception) {
                logger.error("[Gluon] Error applying change", e)
                sendEditResult(id, false, e.message)
                val fileName = filePath.substringAfterLast('/')
                GluonNotifications.notifyApplyError(project, fileName, e.message ?: "Unknown error")
            }
        }
    }

    // ========================================================================
    // Code Highlighting
    // ========================================================================

    private fun handleShowChanges(response: JsonObject) {
        val files = response.getAsJsonArray("files") ?: response.getAsJsonObject("payload")?.getAsJsonArray("files") ?: return

        ApplicationManager.getApplication().invokeLater {
            for (fileElement in files) {
                val fileChange = fileElement.asJsonObject
                val filePath = fileChange.get("path")?.asString ?: continue
                val ranges = fileChange.getAsJsonArray("ranges") ?: continue

                // Open file and highlight ranges (like VS Code does)
                openFileAndHighlightRanges(filePath, ranges)
            }
        }
    }

    private fun openFileAndHighlightRanges(filePath: String, ranges: com.google.gson.JsonArray) {
        val virtualFile = LocalFileSystem.getInstance().findFileByPath(filePath)
        if (virtualFile == null) {
            logger.warn("[Gluon] File not found for highlight: $filePath")
            return
        }

        ApplicationManager.getApplication().invokeLater {
            // Open the file in editor with focus
            val fileEditors = FileEditorManager.getInstance(project).openFile(virtualFile, true)
            if (fileEditors.isEmpty()) {
                logger.warn("[Gluon] Could not open editor for: $filePath")
                return@invokeLater
            }

            // Get the text editor
            val textEditor = fileEditors.firstOrNull { it is com.intellij.openapi.fileEditor.TextEditor }
                as? com.intellij.openapi.fileEditor.TextEditor

            if (textEditor == null) {
                logger.warn("[Gluon] No text editor available for: $filePath")
                return@invokeLater
            }

            val editor = textEditor.editor
            val document = editor.document

            // Clear old highlights for this file
            clearHighlights(filePath)

            val markupModel = editor.markupModel
            val highlighters = mutableListOf<RangeHighlighter>()

            // Track first line to scroll to
            var firstLine: Int? = null

            // Highlight all ranges
            for (rangeElement in ranges) {
                val range = rangeElement.asJsonObject
                val startLine = range.get("start_line")?.asInt ?: continue
                val endLine = range.get("end_line")?.asInt ?: continue

                if (firstLine == null) {
                    firstLine = startLine
                }

                // Highlight each line in range
                for (line in startLine..endLine) {
                    if (line >= document.lineCount) break

                    val lineStartOffset = document.getLineStartOffset(line)
                    val lineEndOffset = document.getLineEndOffset(line)

                    // Green background for added/changed lines (more visible)
                    val textAttributes = TextAttributes().apply {
                        backgroundColor = JBColor(
                            Color(200, 255, 200, 80),  // Light mode: brighter green
                            Color(46, 160, 67, 60)     // Dark mode: visible green
                        )
                    }

                    val highlighter = markupModel.addRangeHighlighter(
                        lineStartOffset,
                        lineEndOffset,
                        HighlighterLayer.SELECTION - 1,
                        textAttributes,
                        HighlighterTargetArea.LINES_IN_RANGE
                    )

                    highlighters.add(highlighter)
                }
            }

            activeHighlighters[filePath] = highlighters

            // Scroll to the first highlighted line (center it in viewport)
            if (firstLine != null && firstLine < document.lineCount) {
                val offset = document.getLineStartOffset(firstLine)
                editor.caretModel.moveToOffset(offset)
                editor.scrollingModel.scrollToCaret(com.intellij.openapi.editor.ScrollType.CENTER)
            }

            // Auto-clear highlights after 3 seconds (like VS Code)
            Timer().schedule(3000L) {
                ApplicationManager.getApplication().invokeLater {
                    clearHighlights(filePath)
                }
            }

            logger.info("[Gluon] ✓ Opened and highlighted ${virtualFile.name}")
        }
    }

    private fun openAndHighlightFile(virtualFile: VirtualFile, change: JsonObject, oldContentStr: String) {
        ApplicationManager.getApplication().invokeLater {
            // Open file in editor with focus
            val fileEditors = FileEditorManager.getInstance(project).openFile(virtualFile, true)
            if (fileEditors.isEmpty()) {
                logger.warn("[Gluon] Could not open editor for: ${virtualFile.path}")
                return@invokeLater
            }

            // Get the text editor from the opened file
            val textEditor = fileEditors.firstOrNull { it is com.intellij.openapi.fileEditor.TextEditor }
                as? com.intellij.openapi.fileEditor.TextEditor

            if (textEditor == null) {
                logger.warn("[Gluon] No text editor available for: ${virtualFile.path}")
                return@invokeLater
            }

            val editor = textEditor.editor
            val document = editor.document

            // Safely extract explicit line ranges from change (if available)
            val lineStartElement = change.get("lineStart") ?: change.get("line_start")
            val explicitStart = if (lineStartElement != null && !lineStartElement.isJsonNull) lineStartElement.asInt else null

            val lineEndElement = change.get("lineEnd") ?: change.get("line_end")
            val explicitEnd = if (lineEndElement != null && !lineEndElement.isJsonNull) lineEndElement.asInt else null

            val newCode = change.get("newCode")?.asString ?: change.get("new_code")?.asString

            var startLine = explicitStart ?: 0
            var endLine = explicitEnd

            // If we don't have an explicit range, compute a diff between old and new content
            if (explicitStart == null && newCode != null) {
                val oldLines = oldContentStr.lines()
                val newLines = newCode.lines()

                var diffStart = 0
                while (diffStart < oldLines.size && diffStart < newLines.size && oldLines[diffStart] == newLines[diffStart]) {
                    diffStart++
                }

                if (diffStart < newLines.size || diffStart < oldLines.size) {
                    var oldEnd = oldLines.size - 1
                    var newEnd = newLines.size - 1
                    while (oldEnd >= diffStart && newEnd >= diffStart && oldLines[oldEnd] == newLines[newEnd]) {
                        oldEnd--
                        newEnd--
                    }
                    startLine = diffStart
                    endLine = maxOf(diffStart, newEnd)
                } else {
                    // No differences found
                    endLine = startLine
                }
            } else if (endLine == null && newCode != null) {
                val lineCount = newCode.lines().size
                endLine = startLine + lineCount - 1
            }

            // Fallback
            if (endLine == null) {
                endLine = startLine + 10
            }

            // Ensure we're within document bounds
            startLine = startLine.coerceIn(0, maxOf(0, document.lineCount - 1))
            endLine = endLine.coerceIn(startLine, maxOf(0, document.lineCount - 1))

            // Scroll to the changed region (center it in viewport)
            if (startLine < document.lineCount) {
                val offset = document.getLineStartOffset(startLine)
                editor.caretModel.moveToOffset(offset)
                editor.scrollingModel.scrollToCaret(com.intellij.openapi.editor.ScrollType.CENTER)
            }

            // Highlight the changed lines
            if (startLine <= endLine && endLine < document.lineCount) {
                highlightLines(virtualFile.path, startLine, endLine)
            }

            logger.info("[Gluon] ✓ Opened and highlighted ${virtualFile.name} (lines $startLine-$endLine)")
        }
    }


    private fun highlightLines(filePath: String, startLine: Int, endLine: Int) {
        ApplicationManager.getApplication().invokeLater {
            val virtualFile = LocalFileSystem.getInstance().findFileByPath(filePath) ?: return@invokeLater
            val document = FileDocumentManager.getInstance().getDocument(virtualFile) ?: return@invokeLater

            val editors = FileEditorManager.getInstance(project).getAllEditors(virtualFile)

            // Clear old highlights for this file once before adding new ones
            clearHighlights(filePath)
            val allHighlighters = mutableListOf<RangeHighlighter>()

            for (editorWrapper in editors) {
                val editor = (editorWrapper as? com.intellij.openapi.fileEditor.TextEditor)?.editor ?: continue
                val markupModel = editor.markupModel

                // Highlight each line in range
                for (line in startLine..endLine) {
                    if (line >= document.lineCount) break

                    val lineStartOffset = document.getLineStartOffset(line)
                    val lineEndOffset = document.getLineEndOffset(line)

                    // Green background for added/changed lines (more visible)
                    val textAttributes = TextAttributes().apply {
                        backgroundColor = JBColor(
                            Color(200, 255, 200, 80),  // Light mode: brighter green
                            Color(46, 160, 67, 60)     // Dark mode: visible green
                        )
                    }

                    val highlighter = markupModel.addRangeHighlighter(
                        lineStartOffset,
                        lineEndOffset,
                        HighlighterLayer.SELECTION - 1,
                        textAttributes,
                        HighlighterTargetArea.LINES_IN_RANGE
                    )

                    allHighlighters.add(highlighter)
                }
            }

            if (allHighlighters.isNotEmpty()) {
                activeHighlighters[filePath] = allHighlighters

                // Auto-clear highlights after 3 seconds (like VS Code)
                Timer().schedule(3000L) {
                    ApplicationManager.getApplication().invokeLater {
                        clearHighlights(filePath)
                    }
                }
            }

            logger.info("[Gluon] ✓ Highlighted lines $startLine-$endLine in ${virtualFile.name}")
        }
    }

    private fun clearHighlights(filePath: String) {
        val highlighters = activeHighlighters.remove(filePath) ?: return

        for (highlighter in highlighters) {
            try {
                highlighter.dispose()
            } catch (e: Exception) {
                // Highlighter may already be disposed
            }
        }
    }

    // ========================================================================
    // Progress Updates
    // ========================================================================

    private fun handleApplyProgress(response: JsonObject) {
        val payload = response.getAsJsonObject("payload")
        val changeId = payload.get("changeId")?.asString
        val message = payload.get("message")?.asString
        val progress = payload.get("progress")?.asInt

        logger.info("[Gluon] Progress: changeId=$changeId, progress=$progress%, message=$message")

        // TODO: Show progress in UI (status bar or notification)
    }

    // ========================================================================
    // Status Updates
    // ========================================================================

    private fun sendEditResult(id: String, success: Boolean, error: String?) {
        val message = mutableMapOf<String, Any>(
            "type" to "edit_result",
            "id" to id,
            "success" to success
        )
        if (error != null) {
            message["error"] = error
        }
        sendMessage(message)
    }

    private fun sendChangeStatusEvent(type: String, changeId: String, batchId: String?) {
        val message = mutableMapOf<String, Any>(
            "type" to type,
            "changeId" to changeId
        )
        if (batchId != null) {
            message["batchId"] = batchId
        }
        sendMessage(message)
    }

    // ========================================================================
    // Cleanup
    // ========================================================================

    override fun dispose() {
        logger.info("[Gluon] Disposing GluonProjectService")

        // Clear all highlights
        for (filePath in activeHighlighters.keys.toList()) {
            clearHighlights(filePath)
        }

        // Close WebSocket
        webSocketClient?.close()
    }

}