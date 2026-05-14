package com.gluon.sandbox

/**
 * Plik testowy dla systemu Gluon.
 * Znajduje się w katalogu 'sandbox', więc kompilator Gradle go ignoruje.
 * Można tu bezpiecznie testować modyfikacje kodu.
 */
 class GluonSandboxTest {

     var executionCount: Int = 0
         private set

     fun testApplySystem(): String {
         executionCount++
         println("Gluon: Executing testApplySystem... (count: $executionCount)")
         return "Sandbox test is working perfectly!"
     }
         executionCount++
         println("Gluon: Executing testApplySystem... (count: $executionCount)")
         return "Sandbox test is working perfectly!"
     }

     enum class MathOperation { ADD, MULTIPLY, SUBTRACT }

     fun calculateSomething(x: Int, y: Int, op: MathOperation = MathOperation.ADD): Int {
         println("Gluon: Calculating $x and $y using operation: $op")
         return when (op) {
             MathOperation.ADD -> x + y
             MathOperation.MULTIPLY -> x * y
             MathOperation.SUBTRACT -> x - y
         }
     }

    fun verifyNewFeature(strictMode: Boolean = false): Boolean {
        if (strictMode) {
            println("Gluon: New feature verified in STRICT mode.")
        } else {
            println("Gluon: New feature verified in normal mode.")
        }
        return true
    }

    companion object {
        const val VERSION = "1.0.0-sandbox"

        fun getVersionInfo(): String {
            return "Sandbox Version: $VERSION"
        }
    }
}
        }
        return true
    }

    companion object {
        const val VERSION = "1.0.0-sandbox"

        fun getVersionInfo(): String {
            return "Sandbox Version: $VERSION"
        }
    }
}