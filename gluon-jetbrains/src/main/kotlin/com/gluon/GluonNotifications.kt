package com.gluon

import com.intellij.notification.Notification
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project

/**
 * Gluon Notification System
 *
 * Provides user-friendly notifications for Gluon operations.
 */
object GluonNotifications {

    private const val NOTIFICATION_GROUP_ID = "Gluon Notifications"

    fun showInfo(project: Project?, title: String, content: String) {
        NotificationGroupManager.getInstance()
            .getNotificationGroup(NOTIFICATION_GROUP_ID)
            .createNotification(title, content, NotificationType.INFORMATION)
            .notify(project)
    }

    fun showWarning(project: Project?, title: String, content: String) {
        NotificationGroupManager.getInstance()
            .getNotificationGroup(NOTIFICATION_GROUP_ID)
            .createNotification(title, content, NotificationType.WARNING)
            .notify(project)
    }

    fun showError(project: Project?, title: String, content: String) {
        NotificationGroupManager.getInstance()
            .getNotificationGroup(NOTIFICATION_GROUP_ID)
            .createNotification(title, content, NotificationType.ERROR)
            .notify(project)
    }

    // ========================================================================
    // Gluon-specific notifications
    // ========================================================================

    fun notifyConnected(project: Project?) {
        showInfo(project, "Gluon Connected", "Successfully connected to Gluon Desktop App")
    }

    fun notifyDisconnected(project: Project?) {
        if (project == null) {
            showWarning(project, "Gluon Disconnected", "Connection to Gluon Desktop App lost. Attempting to reconnect...")
            return
        }

        val notification = NotificationGroupManager.getInstance()
            .getNotificationGroup(NOTIFICATION_GROUP_ID)
            .createNotification(
                "Gluon Disconnected",
                "Connection to Gluon Desktop App lost. Attempting to reconnect...",
                NotificationType.WARNING
            )

        // Add manual reconnect action
        notification.addAction(object : AnAction("Reconnect Now") {
            override fun actionPerformed(e: AnActionEvent) {
                val service = project.getService(GluonProjectService::class.java)
                service?.forceReconnect()
                notification.expire()
            }
        })

        notification.notify(project)
    }

    fun notifyChangeApplied(project: Project?, fileName: String) {
        showInfo(project, "Change Applied", "Successfully applied changes to $fileName")
    }

    fun notifyChangeUndone(project: Project?, fileName: String) {
        showInfo(project, "Change Undone", "Successfully undone changes to $fileName")
    }

    fun notifyChangeRedone(project: Project?, fileName: String) {
        showInfo(project, "Change Redone", "Successfully redone changes to $fileName")
    }

    fun notifyApplyError(project: Project?, fileName: String, error: String) {
        showError(project, "Apply Failed", "Failed to apply changes to $fileName: $error")
    }
}
