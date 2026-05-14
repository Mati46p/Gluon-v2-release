//! Language Configuration and Detection
//!
//! Maps file extensions to Tree-sitter grammars.

use tree_sitter::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Python,
    JavaScript,
    TypeScript,
    TypeScriptReact, // .tsx
    Rust,
    Go,
    Java,
    Kotlin,
    Cpp,
}
 
impl SupportedLanguage {
    /// Detects language based on file extension
    pub fn from_path(path: &str) -> Option<Self> {
        let path_lower = path.to_lowercase();

        if path_lower.ends_with(".py") || path_lower.ends_with(".pyw") {
            return Some(Self::Python);
        }
        if path_lower.ends_with(".js") || path_lower.ends_with(".mjs") || path_lower.ends_with(".cjs") || path_lower.ends_with(".jsx") {
            return Some(Self::JavaScript);
        }
        if path_lower.ends_with(".ts") {
            return Some(Self::TypeScript);
        }
        if path_lower.ends_with(".tsx") {
            return Some(Self::TypeScriptReact);
        }
        if path_lower.ends_with(".rs") {
            return Some(Self::Rust);
        }
        if path_lower.ends_with(".go") {
            return Some(Self::Go);
        }
        if path_lower.ends_with(".java") {
            return Some(Self::Java);
        }
        if path_lower.ends_with(".kt") || path_lower.ends_with(".kts") {
            return Some(Self::Kotlin);
        }
        if path_lower.ends_with(".cpp") || path_lower.ends_with(".cc") || path_lower.ends_with(".cxx") || path_lower.ends_with(".h") || path_lower.ends_with(".hpp") {
            return Some(Self::Cpp);
        }

        // Add more mappings here as needed
        None
    }
 
    /// Returns the raw Tree-sitter language grammar
    pub fn get_grammar(&self) -> Language {
        match self {
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::TypeScriptReact => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::Kotlin => tree_sitter_java::LANGUAGE.into(), // Use Java grammar as fallback for Kotlin structure similarity
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        }
    }
 
    /// Returns a human-readable name
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::TypeScriptReact => "TSX",
            Self::Rust => "Rust",
            Self::Go => "Go",
            Self::Java => "Java",
            Self::Kotlin => "Kotlin",
            Self::Cpp => "C++",
        }
    }
}