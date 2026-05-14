# 🔨 Build Instructions for Gluon JetBrains Plugin

## Quick Start (Recommended)

### Option 1: Using IntelliJ IDEA (Easiest)

1. **Open the project in IntelliJ IDEA**:
   ```
   File → Open → Select gluon-jetbrains folder
   ```

2. **Wait for Gradle sync** - IntelliJ will automatically download Gradle and dependencies

3. **Build the plugin**:
   - Open Gradle panel (View → Tool Windows → Gradle)
   - Navigate to: `gluon-jetbrains → Tasks → intellij → buildPlugin`
   - Double-click to run

4. **Find the built plugin**:
   ```
   gluon-jetbrains/build/distributions/Gluon-1.0-SNAPSHOT.zip
   ```

### Option 2: Using Gradle Wrapper (Command Line)

1. **First, initialize Gradle Wrapper** (if not already present):

   If you have Gradle installed:
   ```bash
   cd gluon-jetbrains
   gradle wrapper --gradle-version 8.5
   ```

   If you don't have Gradle, download wrapper files from another project or:
   - Download Gradle from https://gradle.org/releases/
   - Extract and add to PATH
   - Then run the above command

2. **Build using wrapper**:
   ```bash
   # On Windows
   gradlew.bat build

   # On Linux/Mac
   ./gradlew build
   ```

3. **Find the built plugin**:
   ```
   build/distributions/Gluon-1.0-SNAPSHOT.zip
   ```

## Testing the Plugin

### Run in IDE Sandbox

This runs a new IntelliJ instance with the plugin installed:

```bash
# Using IntelliJ IDEA
Gradle panel → Tasks → intellij → runIde

# Using command line
./gradlew runIde
```

### Manual Testing

1. Build the plugin (see above)
2. In your main IDE:
   - Settings → Plugins → ⚙️ → Install Plugin from Disk
   - Select `build/distributions/Gluon-1.0-SNAPSHOT.zip`
   - Restart IDE

## Troubleshooting

### "Gradle not found"

**Solution 1** - Use IntelliJ IDEA (recommended):
- IntelliJ bundles its own Gradle distribution
- Just open the project and it handles everything

**Solution 2** - Install Gradle:
```bash
# Windows (using Chocolatey)
choco install gradle

# macOS (using Homebrew)
brew install gradle

# Linux (using SDKMAN)
curl -s "https://get.sdkman.io" | bash
sdk install gradle
```

### "Java version mismatch"

The plugin requires **JDK 17 or higher**.

Check your version:
```bash
java -version
```

If you need to install JDK 17:
- Download from: https://adoptium.net/
- Or use package managers:
  ```bash
  # Windows (Chocolatey)
  choco install temurin17

  # macOS (Homebrew)
  brew install openjdk@17

  # Linux (apt)
  sudo apt install openjdk-17-jdk
  ```

### "Build fails with compilation errors"

Make sure you're using the correct Kotlin and IntelliJ Platform versions:

```kotlin
// In build.gradle.kts
plugins {
    id("org.jetbrains.kotlin.jvm") version "1.9.22"
    id("org.jetbrains.intellij") version "1.17.2"
}

intellij {
    version.set("2023.2.5")
    type.set("IC") // IntelliJ Community
}
```

### "Cannot resolve dependencies"

Clear Gradle cache and rebuild:
```bash
./gradlew clean build --refresh-dependencies
```

## Development Workflow

### 1. Make Code Changes

Edit files in `src/main/kotlin/com/gluon/`

### 2. Run Tests (if any)

```bash
./gradlew test
```

### 3. Build Plugin

```bash
./gradlew buildPlugin
```

### 4. Test in Sandbox

```bash
./gradlew runIde
```

### 5. Verify Plugin XML

Check `src/main/resources/META-INF/plugin.xml` for:
- Correct version numbers
- Valid dependency declarations
- Proper extension points

## Publishing (Future)

### To JetBrains Marketplace

1. **Create account** at https://plugins.jetbrains.com/

2. **Get publish token**:
   - Account → Authentication Tokens → Generate Token

3. **Set environment variable**:
   ```bash
   export PUBLISH_TOKEN=your-token-here
   ```

4. **Publish**:
   ```bash
   ./gradlew publishPlugin
   ```

### Manual Upload

1. Build the plugin: `./gradlew buildPlugin`
2. Go to https://plugins.jetbrains.com/
3. Upload `build/distributions/Gluon-1.0-SNAPSHOT.zip`

## File Checklist Before Building

- [x] `build.gradle.kts` - Dependencies and versions correct
- [x] `plugin.xml` - Metadata filled in
- [x] All `.kt` files compile without errors
- [x] WebSocket URL matches Desktop App port (8743)
- [x] Notification group registered
- [x] Service properly registered

## Build Output

After successful build, you'll find:

```
build/
├── classes/               # Compiled .class files
├── distributions/         # 📦 PLUGIN ZIP HERE
│   └── Gluon-1.0-SNAPSHOT.zip
├── libs/                  # Plugin JAR
│   └── Gluon-1.0-SNAPSHOT.jar
└── tmp/                   # Temporary build files
```

The **ZIP file** in `distributions/` is what you install in JetBrains IDEs.

## Next Steps After Building

1. **Install the plugin** (see README.md)
2. **Start Gluon Desktop App** (must be running on port 8743)
3. **Open a project in JetBrains IDE**
4. **Look for "Gluon Connected" notification**
5. **Test apply/undo/redo functionality**

## Need Help?

- Check logs: `~/.local/share/JetBrains/<IDE>/log/idea.log` (Linux/Mac)
- Check logs: `%APPDATA%\JetBrains\<IDE>\log\idea.log` (Windows)
- Enable debug: Add `-Didea.log.debug.categories=#com.gluon` to VM options
- Report issues: [Your GitHub repo]

---

**Good luck building! 🚀**
