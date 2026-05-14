package com.gluon

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project

/**
 * Action to manually reconnect to Gluon Desktop App
 *
 * Available in Tools menu: Tools > Gluon > Reconnect
 */
class ReconnectGluonAction : AnAction("Reconnect to Gluon") {

    private val logger = Logger.getInstance(ReconnectGluonAction::class.java)

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return

        logger.info("[Gluon] Reconnect action triggered")

        val service = project.getService(GluonProjectService::class.java)
        if (service != null) {
            service.forceReconnect()
            logger.info("[Gluon] ✓ Reconnect initiated")
        } else {
            logger.error("[Gluon] Service not available")
        }
    }

    override fun update(e: AnActionEvent) {
        e.presentation.isEnabled = e.project != null
    }
}
