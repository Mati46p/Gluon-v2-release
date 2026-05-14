package com.gluon

import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import com.intellij.openapi.startup.StartupActivity

/**
 * Gluon Startup Activity
 *
 * Automatically initializes the Gluon service when a project is opened.
 * This ensures WebSocket connection to Gluon Desktop App is established immediately.
 */
class GluonStartupActivity : StartupActivity {

    private val logger = Logger.getInstance(GluonStartupActivity::class.java)

    override fun runActivity(project: Project) {
        logger.info("[Gluon] Starting up for project: ${project.name}")

        // Initialize the service to start connection immediately after project opens
        val service = project.getService(GluonProjectService::class.java)

        if (service != null) {
            logger.info("[Gluon] ✓ Service initialized successfully")
        } else {
            logger.error("[Gluon] ✗ Failed to initialize service")
        }
    }
}