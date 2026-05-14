//! Git-Style SEARCH/REPLACE Parser
//!
//! This parser handles the git-style conflict marker format used in prompts:
//!
//! ```
//! **File: path/to/file.ext**
//!
//! <<<<<<< SEARCH
//! [exact code to find]
//! =======
//! [replacement code]
//! >>>>>>> REPLACE
//! ```
//!
//! ALSO supports Unicode Box delimiters (NEW STANDARD):
//!
//! ```
//! **File: path/to/file.ext**
//!
//! ╔═══════ SEARCH
//! [exact code to find]
//! ╠═══════ REPLACE
//! [replacement code]
//! ╚═══════ END
//! ```
//!
//! The Unicode Box format is PREFERRED as it:
//! - Doesn't conflict with HTML rendering (no H1/H2 tags)
//! - Highly unique (rarely appears in actual code)
//! - Visually distinct for both humans and parsers

use crate::apply_system::parsers::Parser;
use crate::apply_system::shared::types::ChangeQueueItem;
use regex::Regex;

pub struct GitStyleSearchReplaceParser;

impl Parser for GitStyleSearchReplaceParser {
    fn name(&self) -> &'static str {
        "GitStyleSearchReplace"
    }

    fn can_handle(&self, raw_response: &str) -> bool {
        // Check for git-style markers (7 character delimiters)
        let has_git_style = raw_response.contains("<<<<<<< SEARCH")
            || raw_response.contains(">>>>>>> REPLACE");

        // Check for Unicode Box delimiters (NEW STANDARD)
        let has_unicode_box = raw_response.contains("╔═══════ SEARCH")
            || raw_response.contains("╠═══════ REPLACE")
            || raw_response.contains("╚═══════ END");

        has_git_style || has_unicode_box
    }

    fn parse(&self, raw_response: &str) -> Result<Vec<ChangeQueueItem>, String> {
        let mut changes = Vec::new();

        // Try Unicode Box format first (NEW STANDARD - higher priority)
        if raw_response.contains("╔═══════ SEARCH") {
            changes.extend(self.parse_unicode_box_blocks(raw_response)?);
        }

        // Fallback to git-style markers (LEGACY - for backward compatibility)
        if changes.is_empty() && raw_response.contains("<<<<<<< SEARCH") {
            changes.extend(self.parse_git_style_blocks(raw_response)?);
        }

        if changes.is_empty() {
            return Err("No valid git-style or Unicode Box SEARCH/REPLACE blocks found".to_string());
        }

        Ok(changes)
    }
}

impl GitStyleSearchReplaceParser {

    /// Parse Unicode Box delimiter blocks (NEW STANDARD)
    ///
    /// Format:
    /// ```
    /// **File: path/to/file.ext**
    ///
    /// ╔═══════ SEARCH
    /// old code
    /// ╠═══════ REPLACE
    /// new code
    /// ╚═══════ END
    /// ```
    fn parse_unicode_box_blocks(&self, text: &str) -> Result<Vec<ChangeQueueItem>, String> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        // Keep track of the last found file path to support multiple blocks for the same file
        // without repeating the file header every time.
        let mut last_found_path: Option<String> = None;

        // Extract file path (can be before or in same block)
        // ENHANCED: Support multiple formats:
        // **File: path** or **Plik: path** (Polish) - markdown bold
        // File: `path` or Plik: `path` - with backticks
        // // File: path or # File: path - as comments (NEW - primary format)
        // <!-- File: path --> - HTML comments
        let file_path_pattern = r"(?:(?:\*\*)|(?://)|(?:#)|(?:/\*)|(?:<!--))\s*(?:File|Plik|file|plik):\s*`?([^`\*\n]+?\.(?:py|ts|js|tsx|jsx|rs|go|java|cpp|c|h|html|htm|php|rb|swift|kt|cs|css|scss|json|yaml|yml|toml|xml|md|txt|vue|svelte))`?(?:\*\*|-->)?";
        let file_path_re = Regex::new(file_path_pattern).map_err(|e| format!("Regex error: {}", e))?;

        while i < lines.len() {
            // Look for file path first (locally near this line)
            let mut detected_path: Option<String> = None;

            // Scan backward for file path (within 10 lines to catch more cases)
            for offset in 0..10 {
                if i >= offset {
                    if let Some(captures) = file_path_re.captures(lines[i - offset]) {
                        detected_path = Some(captures.get(1)
                            .map(|m| m.as_str().trim().to_string())
                            .unwrap_or_default());
                        break;
                    }
                }
            }

            // Also scan forward (in case file path comes after SEARCH marker)
            if detected_path.is_none() && i + 1 < lines.len() {
                for offset in 1..5 {
                    if i + offset < lines.len() {
                        if let Some(captures) = file_path_re.captures(lines[i + offset]) {
                            detected_path = Some(captures.get(1)
                                .map(|m| m.as_str().trim().to_string())
                                .unwrap_or_default());
                            break;
                        }
                    }
                }
            }

            // Update persistent state if we found a path
            if let Some(p) = detected_path {
                last_found_path = Some(p);
            }

            // Look for SEARCH marker
            if lines[i].contains("╔═══════ SEARCH") {
                // Use the last known path. This allows models to output one file header
                // followed by multiple SEARCH/REPLACE blocks.
                let file_path = last_found_path.clone().ok_or_else(|| {
                    format!("Missing file path before Unicode Box SEARCH block at line {}", i + 1)
                })?;

                // Collect old_code until ╠═══════ REPLACE
                let mut old_code_lines = Vec::new();
                i += 1;

                while i < lines.len() && !lines[i].contains("╠═══════ REPLACE") && !lines[i].contains("╠═══════") {
                    old_code_lines.push(lines[i]);
                    i += 1;
                }

                if i >= lines.len() {
                    return Err(format!(
                        "Missing ╠═══════ REPLACE separator in Unicode Box block for {}",
                        file_path
                    ));
                }

                let old_code = crate::apply_system::parsers::sanitize_code_block(old_code_lines);

                // Skip ╠═══════ REPLACE line
                i += 1;

                // Collect new_code until ╚═══════ END
                let mut new_code_lines = Vec::new();

                while i < lines.len() && !lines[i].contains("╚═══════ END") && !lines[i].contains("╚═══════") {
                    new_code_lines.push(lines[i]);
                    i += 1;
                }

                if i >= lines.len() {
                    return Err(format!("Missing ╚═══════ END marker for {}", file_path));
                }

                let new_code = crate::apply_system::parsers::sanitize_code_block(new_code_lines);

                // [GLUON SAFETY] Lazy Coding Check
                if let Err(e) = crate::apply_system::parsers::detect_lazy_coding(&new_code) {
                    return Err(format!("Lazy coding detected in {}: {}", file_path, e));
                }

                // Validate
                if old_code.trim().is_empty() {
                    return Err(format!("Empty old_code in Unicode Box SEARCH block for {}", file_path));
                }

                // Create change item
                let change = ChangeQueueItem::new(
                    file_path.clone(),
                    0, // Line numbers will be determined by matcher
                    0,
                    old_code,
                    new_code,
                );

                blocks.push(change);
            }

            i += 1;
        }

        Ok(blocks)
    }

    /// Parse git-style conflict marker blocks (LEGACY - for backward compatibility)
    ///
    /// Format:
    /// ```
    /// **File: path/to/file.ext**
    ///
    /// <<<<<<< SEARCH
    /// old code
    /// =======
    /// new code
    /// >>>>>>> REPLACE
    /// ```
    fn parse_git_style_blocks(&self, text: &str) -> Result<Vec<ChangeQueueItem>, String> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        // Keep track of the last found file path to support multiple blocks for the same file
        let mut last_found_path: Option<String> = None;

        // Extract file path (can be before or in same block)
        // ENHANCED: Support multiple formats (same as Unicode Box parser)
        // **File: path** or **Plik: path** (Polish) - markdown bold
        // File: `path` or Plik: `path` - with backticks
        // // File: path or # File: path - as comments (NEW - primary format)
        // <!-- File: path --> - HTML comments
        let file_path_pattern = r"(?:(?:\*\*)|(?://)|(?:#)|(?:/\*)|(?:<!--))\s*(?:File|Plik|file|plik):\s*`?([^`\*\n]+?\.(?:py|ts|js|tsx|jsx|rs|go|java|cpp|c|h|html|htm|php|rb|swift|kt|cs|css|scss|json|yaml|yml|toml|xml|md|txt|vue|svelte))`?(?:\*\*|-->)?";
        let file_path_re = Regex::new(file_path_pattern).map_err(|e| format!("Regex error: {}", e))?;

        while i < lines.len() {
            // Look for file path first (locally)
            let mut detected_path: Option<String> = None;

            // Scan backward for file path (within 10 lines)
            for offset in 0..10 {
                if i >= offset {
                    if let Some(captures) = file_path_re.captures(lines[i - offset]) {
                        detected_path = Some(captures.get(1)
                            .map(|m| m.as_str().trim().to_string())
                            .unwrap_or_default());
                        break;
                    }
                }
            }

            // Also scan forward
            if detected_path.is_none() && i + 1 < lines.len() {
                for offset in 1..5 {
                    if i + offset < lines.len() {
                        if let Some(captures) = file_path_re.captures(lines[i + offset]) {
                            detected_path = Some(captures.get(1)
                                .map(|m| m.as_str().trim().to_string())
                                .unwrap_or_default());
                            break;
                        }
                    }
                }
            }

            // Update persistent state
            if let Some(p) = detected_path {
                last_found_path = Some(p);
            }

            // Look for SEARCH marker (7 character version)
            if lines[i].trim().starts_with("<<<<<<< SEARCH") || lines[i].contains("<<<<<<< SEARCH") {
                let file_path = last_found_path.clone().ok_or_else(|| {
                    format!("Missing file path before SEARCH block at line {}", i + 1)
                })?;

                // Collect old_code until =======
                let mut old_code_lines = Vec::new();
                i += 1;

                while i < lines.len() && !lines[i].trim().starts_with("=======") {
                    old_code_lines.push(lines[i]);
                    i += 1;
                }

                if i >= lines.len() {
                    return Err(format!(
                        "Missing ======= separator in SEARCH block for {}",
                        file_path
                    ));
                }

                let old_code = crate::apply_system::parsers::sanitize_code_block(old_code_lines);

                // Skip ======= line
                i += 1;

                // Collect new_code until >>>>>>> REPLACE
                let mut new_code_lines = Vec::new();

                while i < lines.len() && !lines[i].trim().starts_with(">>>>>>> REPLACE") {
                    new_code_lines.push(lines[i]);
                    i += 1;
                }

                if i >= lines.len() {
                    return Err(format!("Missing >>>>>>> REPLACE marker for {}", file_path));
                }

                let new_code = crate::apply_system::parsers::sanitize_code_block(new_code_lines);

                // [GLUON SAFETY] Lazy Coding Check
                if let Err(e) = crate::apply_system::parsers::detect_lazy_coding(&new_code) {
                    return Err(format!("Lazy coding detected in {}: {}", file_path, e));
                }

                // Validate
                if old_code.trim().is_empty() {
                    return Err(format!("Empty old_code in SEARCH block for {}", file_path));
                }

                // Create change item
                let change = ChangeQueueItem::new(
                    file_path.clone(),
                    0, // Line numbers will be determined by matcher
                    0,
                    old_code,
                    new_code,
                );

                blocks.push(change);
            }

            i += 1;
        }

        Ok(blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_git_style() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/test.ts**

<<<<<<< SEARCH
old code
=======
new code
>>>>>>> REPLACE
"#;

        assert!(parser.can_handle(text));
    }

    #[test]
    fn test_can_handle_unicode_box() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/test.ts**

╔═══════ SEARCH
old code
╠═══════ REPLACE
new code
╚═══════ END
"#;

        assert!(parser.can_handle(text));
    }

    #[test]
    fn test_parse_git_style_block() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/auth.ts**

<<<<<<< SEARCH
export function login(username: string) {
  return authenticate(username);
}
=======
export function login(username: string, password: string) {
  return authenticate(username, password);
}
>>>>>>> REPLACE
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/auth.ts");
        assert!(changes[0].old_code.contains("authenticate(username)"));
        assert!(changes[0].new_code.contains("authenticate(username, password)"));
    }

    #[test]
    fn test_parse_unicode_box_block() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/auth.ts**

╔═══════ SEARCH
export function login(username: string) {
  return authenticate(username);
}
╠═══════ REPLACE
export function login(username: string, password: string) {
  return authenticate(username, password);
}
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/auth.ts");
        assert!(changes[0].old_code.contains("authenticate(username)"));
        assert!(changes[0].new_code.contains("authenticate(username, password)"));
    }

    #[test]
    fn test_parse_with_nested_backticks() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/generator.py**

╔═══════ SEARCH
def create_markdown():
    content = """
    code block
    print("hello")
    """
    return content
╠═══════ REPLACE
def create_markdown():
    content = """
    code block
    print("hello world")
    """
    return content
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        // sanitize_code_block removes backticks, so we check for actual code content
        assert!(changes[0].old_code.contains("code block"));
        assert!(changes[0].old_code.contains("print(\"hello\")"));
        assert!(changes[0].new_code.contains("print(\"hello world\")"));
    }

    #[test]
    fn test_multiple_unicode_box_blocks_same_file() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/auth.ts**

╔═══════ SEARCH
function login() {
  return true;
}
╠═══════ REPLACE
function login(user: string) {
  return validateUser(user);
}
╚═══════ END

╔═══════ SEARCH
function logout() {
  return false;
}
╠═══════ REPLACE
function logout(user: string) {
  clearSession(user);
  return true;
}
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "src/auth.ts");
        assert_eq!(changes[1].file_path, "src/auth.ts");
        assert!(changes[0].old_code.contains("login"));
        assert!(changes[1].old_code.contains("logout"));
    }

    #[test]
    fn test_multiple_git_style_blocks_same_file() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/utils.js**

<<<<<<< SEARCH
function add(a, b) {
  return a + b;
}
=======
function add(a, b, c = 0) {
  return a + b + c;
}
>>>>>>> REPLACE

<<<<<<< SEARCH
function subtract(a, b) {
  return a - b;
}
=======
function subtract(a, b, c = 0) {
  return a - b - c;
}
>>>>>>> REPLACE
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].file_path, "src/utils.js");
        assert_eq!(changes[1].file_path, "src/utils.js");
        assert!(changes[0].old_code.contains("add"));
        assert!(changes[1].old_code.contains("subtract"));
    }

    #[test]
    fn test_comment_style_file_path() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
// File: src/config.ts

╔═══════ SEARCH
const API_URL = "http://localhost";
╠═══════ REPLACE
const API_URL = "https://api.example.com";
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/config.ts");
    }

    #[test]
    fn test_hash_comment_file_path() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
# File: src/settings.py

╔═══════ SEARCH
DEBUG = False
╠═══════ REPLACE
DEBUG = True
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/settings.py");
    }

    #[test]
    fn test_empty_old_code_error() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/test.ts**

╔═══════ SEARCH
╠═══════ REPLACE
new code
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty old_code"));
    }

    #[test]
    fn test_missing_file_path_error() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
╔═══════ SEARCH
old code
╠═══════ REPLACE
new code
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing file path"));
    }

    #[test]
    fn test_missing_replace_separator() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/test.ts**

╔═══════ SEARCH
old code
new code
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing ╠═══════ REPLACE"));
    }

    #[test]
    fn test_missing_end_marker() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/test.ts**

╔═══════ SEARCH
old code
╠═══════ REPLACE
new code
"#;

        let result = parser.parse(text);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing ╚═══════ END"));
    }

    #[test]
    fn test_unicode_box_with_indented_code() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/class.py**

╔═══════ SEARCH
class MyClass:
    def __init__(self):
        self.value = 0
╠═══════ REPLACE
class MyClass:
    def __init__(self, initial_value=0):
        self.value = initial_value
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("self.value = 0"));
        assert!(changes[0].new_code.contains("initial_value"));
    }

    #[test]
    fn test_git_style_with_context_lines() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**File: src/app.ts**

<<<<<<< SEARCH
export function initialize() {
  console.log("Starting...");
  setupRoutes();
  console.log("Ready!");
}
=======
export function initialize() {
  console.log("Starting application...");
  setupRoutes();
  setupMiddleware();
  console.log("Application ready!");
}
>>>>>>> REPLACE
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("Starting..."));
        assert!(changes[0].new_code.contains("setupMiddleware"));
    }

    #[test]
    fn test_polish_file_marker() {
        let parser = GitStyleSearchReplaceParser;

        let text = r#"
**Plik: src/main.rs**

╔═══════ SEARCH
fn main() {
    println!("Hello");
}
╠═══════ REPLACE
fn main() {
    println!("Witaj świecie");
}
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/main.rs");
    }

    #[test]
    fn test_backtick_file_path() {
        let parser = GitStyleSearchReplaceParser;

        // Note: Backtick format without ** is not supported by the current regex
        // The regex requires ** or // or # prefix
        let text = r#"
**File: `src/helper.ts`**

╔═══════ SEARCH
export const PI = 3.14;
╠═══════ REPLACE
export const PI = 3.14159;
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/helper.ts");
    }

    #[test]
    fn test_mixed_unicode_and_git_style() {
        let parser = GitStyleSearchReplaceParser;

        // Unicode box has priority, so only Unicode box blocks should be parsed
        let text = r#"
**File: src/mixed.ts**

╔═══════ SEARCH
const x = 1;
╠═══════ REPLACE
const x = 2;
╚═══════ END

<<<<<<< SEARCH
const y = 3;
=======
const y = 4;
>>>>>>> REPLACE
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        // Only Unicode box should be parsed (higher priority)
        assert_eq!(changes.len(), 1);
        assert!(changes[0].old_code.contains("const x = 1"));
    }

    #[test]
    fn test_file_path_scan_backward() {
        let parser = GitStyleSearchReplaceParser;

        // File path is 5 lines before SEARCH marker
        let text = r#"
**File: src/backward.ts**

Some commentary here...
More text...

╔═══════ SEARCH
const config = {};
╠═══════ REPLACE
const config = { debug: true };
╚═══════ END
"#;

        let result = parser.parse(text);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/backward.ts");
    }
}
