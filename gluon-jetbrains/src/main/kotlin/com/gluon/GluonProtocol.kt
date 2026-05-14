package com.gluon

import com.google.gson.annotations.SerializedName

/**
 * Gluon Protocol Data Classes
 *
 * These classes match the WebSocket protocol used by Gluon Desktop App.
 * Based on editor_bridge.rs and VSCode extension implementation.
 */

// ============================================================================
// Messages FROM Desktop TO Editor
// ============================================================================

data class EditorEditRequest(
    val id: String,
    @SerializedName("file_path")
    val filePath: String,
    @SerializedName("new_content")
    val newContent: String,

    // Extended fields for undo/redo tracking
    @SerializedName("change_id")
    val changeId: String? = null,
    @SerializedName("batch_id")
    val batchId: String? = null,
    @SerializedName("old_content")
    val oldContent: String? = null,
    @SerializedName("line_start")
    val lineStart: Int? = null
)

data class ChangeRange(
    @SerializedName("start_line")
    val startLine: Int,  // 0-based
    @SerializedName("end_line")
    val endLine: Int     // 0-based
)

data class FileChangeNotification(
    val path: String,
    val ranges: List<ChangeRange>
)

data class ShowChangesMessage(
    val files: List<FileChangeNotification>
)

// ============================================================================
// Messages FROM Editor TO Desktop
// ============================================================================

data class EditorEditResponse(
    val id: String,
    val success: Boolean,
    val error: String? = null
)

data class RegisterEditorPayload(
    @SerializedName("editor_type")
    val editorType: String = "jetbrains",
    @SerializedName("project_name")
    val projectName: String,
    val roots: List<String>
)

data class ChangeStatusUpdate(
    @SerializedName("changeId")
    val changeId: String,
    @SerializedName("change_id")
    val changeIdAlt: String = changeId,  // Some messages use snake_case
    val status: String,  // "pending", "applying", "success", "undone", "error"
    val error: String? = null
)

// ============================================================================
// Generic WebSocket Message Envelope
// ============================================================================

data class WebSocketMessage(
    val action: String,
    val payload: Any,
    @SerializedName("request_id")
    val requestId: String? = null
)

// ============================================================================
// Apply System Types
// ============================================================================

data class CodeChange(
    val id: String,
    @SerializedName("filePath")
    val filePath: String,
    @SerializedName("file_path")
    val filePathAlt: String? = null,
    @SerializedName("oldCode")
    val oldCode: String? = null,
    @SerializedName("old_code")
    val oldCodeAlt: String? = null,
    @SerializedName("newCode")
    val newCode: String,
    @SerializedName("new_code")
    val newCodeAlt: String? = null,
    val format: String? = null,  // "CREATE", "LazyStitcher", etc.
    @SerializedName("lineStart")
    val lineStart: Int? = null,
    @SerializedName("line_start")
    val lineStartAlt: Int? = null,
    @SerializedName("batchId")
    val batchId: String? = null,
    @SerializedName("batch_id")
    val batchIdAlt: String? = null,
    @SerializedName("match_result")
    val matchResult: MatchResult? = null
)

data class MatchResult(
    val confidence: Double,
    @SerializedName("method_used")
    val methodUsed: String? = null,
    @SerializedName("confidence_breakdown")
    val confidenceBreakdown: Map<String, Any>? = null
)

data class ApplyCodeChangesPayload(
    val changes: List<CodeChange>,
    @SerializedName("selectedProjects")
    val selectedProjects: List<String>? = null
)

data class UndoRedoPayload(
    @SerializedName("changeId")
    val changeId: String,
    @SerializedName("change_id")
    val changeIdAlt: String? = null,
    @SerializedName("filePath")
    val filePath: String? = null,
    @SerializedName("file_path")
    val filePathAlt: String? = null
)

data class ApplyProgressPayload(
    @SerializedName("changeId")
    val changeId: String,
    val message: String,
    val progress: Int,  // 0-100
    val step: String? = null
)

// ============================================================================
// Action Types (Constants)
// ============================================================================

object GluonActions {
    // FROM Desktop TO Editor
    const val APPLY_CODE_CHANGES = "apply_code_changes"
    const val APPLY_PROGRESS_UPDATE = "apply_progress_update"
    const val CHANGE_STATUS_UPDATE = "change_status_update"
    const val SHOW_CHANGES = "show_changes"
    const val UNDO_CHANGE = "undo_change"
    const val REDO_CHANGE = "redo_change"

    // FROM Editor TO Desktop
    const val REGISTER_EDITOR = "register_editor"
    const val HEARTBEAT = "heartbeat"
    const val EDIT_RESPONSE = "edit_response"

    // Errors
    const val ERROR = "error"
}
