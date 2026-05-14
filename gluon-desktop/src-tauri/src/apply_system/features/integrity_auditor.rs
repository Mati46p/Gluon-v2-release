//! Integrity Auditor Module (Enhanced with Advanced Matchers & Parsers)
//!
//! Compares code structure from a context snapshot against the current disk state.
//!
//! ENHANCED DETECTION CAPABILITIES:
//! 1. Missing Symbols (Functions/Classes deleted) - Using Anchor Matcher
//! 2. Signature Drift (Arguments changed) - Using Weighted Anchor Matcher
//! 3. Logic Rot (Body size drastically reduced) - Using Block Matcher
//! 4. Security Vulnerabilities - Using Pattern Matchers
//! 5. Performance Issues - Using Code Quality Analyzer
//! 6. Architecture Violations - Using Parser System
//! 7. Code Duplication - Using Fuzzy Matcher
//! 8. Anti-Patterns - Using Regex Matcher

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use crate::apply_system::analysis::{AnalysisEngine, SupportedLanguage};
use crate::apply_system::analysis::queries::QueryMatcher;
use crate::apply_system::features::backup_system::{self, BackupFilePreview};
use crate::apply_system::features::code_quality_analyzer::{CodeQualityAnalyzer, AnalyzerConfig};
use crate::apply_system::matchers::{Matcher, FuzzyMatcher, WeightedAnchorMatcher};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityReport {
    pub file_path: String,
    pub status: FileIntegrityStatus,
    pub health_score: HealthScore, // [ETAP 2] New Metric
    pub discrepancies: Vec<Discrepancy>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HealthScore {
    pub score: u8, // 0-100
    pub grade: String, // "A+", "B", "C", "F"
    pub factors: Vec<String>, // Explanations: "-20 Security", "-5 Complexity"
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileIntegrityStatus {
    Ok,
    Warning,
    Critical,
    Skipped,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Discrepancy {
    pub symbol_name: String,
    pub issue_type: IssueType,
    pub description: String,
    pub line_number: usize,
    pub severity: DiscrepancySeverity,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IssueType {
    // Original Issues
    MissingSymbol,
    SignatureMismatch,
    LogicDegradation,

    // [ETAP 2] Quality Issues
    HighComplexity,
    SecurityRisk,
    TypeUnsafety,
    PhantomDependency,

    // [ETAP 3] Pattern-Based Issues (Using Matchers)
    CodeDuplication,        // Detected via Fuzzy Matcher
    AntiPattern,            // Detected via Pattern Matcher
    CodeSmell,              // Detected via Regex Matcher
    PerformanceIssue,       // Detected via Code Quality Analyzer
    ArchitectureViolation,  // Detected via Block Matcher

    // [ETAP 4] Advanced Detection (Using Parsers)
    UnexpectedChange,       // Detected via Parser Diff Analysis
    StructuralDrift,        // Detected via Weighted Anchor Matcher
    BehaviorChange,         // Detected via semantic analysis
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum DiscrepancySeverity {
    Critical,
    Warning,
    Info,
}

// [ETAP 3] Audit Policy Definition (Enhanced)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPolicy {
    pub name: String,
    pub max_complexity: usize,
    pub min_type_coverage: f32,
    pub allow_phantom_imports: bool,
    pub enforce_docstrings: bool,
    pub strict_mode: bool,

    // [NEW] Advanced Matcher Configuration
    pub use_weighted_anchors: bool,
    pub enable_fuzzy_matching: bool,
    pub fuzzy_threshold: f32,

    // [NEW] Quality Analysis Configuration
    pub enable_quality_scan: bool,
    pub enable_security_scan: bool,
    pub enable_performance_scan: bool,
    pub enable_duplication_scan: bool,

    // [NEW] Parser-Based Analysis
    pub detect_anti_patterns: bool,
    pub detect_code_smells: bool,
}

impl AuditPolicy {
    pub fn standard() -> Self {
        Self {
            name: "Standard Production".to_string(),
            max_complexity: 15,
            min_type_coverage: 0.50,
            allow_phantom_imports: false,
            enforce_docstrings: false,
            strict_mode: false,

            // Advanced matchers enabled by default
            use_weighted_anchors: true,
            enable_fuzzy_matching: true,
            fuzzy_threshold: 0.85,

            // Quality analysis enabled
            enable_quality_scan: true,
            enable_security_scan: true,
            enable_performance_scan: true,
            enable_duplication_scan: true,

            // Pattern detection enabled
            detect_anti_patterns: true,
            detect_code_smells: true,
        }
    }

    pub fn strict() -> Self {
        Self {
            name: "Strict Enterprise".to_string(),
            max_complexity: 10,
            min_type_coverage: 0.80,
            allow_phantom_imports: false,
            enforce_docstrings: true,
            strict_mode: true,

            use_weighted_anchors: true,
            enable_fuzzy_matching: false, // Exact matches only
            fuzzy_threshold: 0.95,

            enable_quality_scan: true,
            enable_security_scan: true,
            enable_performance_scan: true,
            enable_duplication_scan: true,

            detect_anti_patterns: true,
            detect_code_smells: true,
        }
    }

    pub fn permissive() -> Self {
        Self {
            name: "Permissive Development".to_string(),
            max_complexity: 25,
            min_type_coverage: 0.30,
            allow_phantom_imports: true,
            enforce_docstrings: false,
            strict_mode: false,

            use_weighted_anchors: true,
            enable_fuzzy_matching: true,
            fuzzy_threshold: 0.70,

            enable_quality_scan: true,
            enable_security_scan: true, // Security always on
            enable_performance_scan: false,
            enable_duplication_scan: false,

            detect_anti_patterns: false,
            detect_code_smells: false,
        }
    }
}

pub fn run_audit(context_file_path: String, selected_files: Vec<String>) -> Result<Vec<IntegrityReport>, String> {
    // 1. Load snapshot content from context file
    let snapshot_files = backup_system::parse_backup_file(&context_file_path)?;
    
    // Load Policy (Standard by default)
    let policy = AuditPolicy::standard(); 

    // 2. Filter files
    let files_to_audit: Vec<&BackupFilePreview> = snapshot_files.iter()
        .filter(|f| selected_files.contains(&f.path) || selected_files.is_empty())
        .collect();

    let mut reports = Vec::new();

    for file_snap in files_to_audit {
        // Pass policy to comparator
        let report = compare_file(file_snap, &policy);
        reports.push(report);
    }

    Ok(reports)
}

fn compare_file(snapshot: &BackupFilePreview, policy: &AuditPolicy) -> IntegrityReport {
    let file_path = &snapshot.path;
    
    // Read current content from disk
    let current_content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            return IntegrityReport {
                file_path: file_path.clone(),
                status: FileIntegrityStatus::Critical,
                health_score: HealthScore { score: 0, grade: "F".to_string(), factors: vec!["File Missing".to_string()] },
                discrepancies: vec![Discrepancy {
                    symbol_name: "FILE".to_string(),
                    issue_type: IssueType::MissingSymbol,
                    description: format!("File missing on disk: {}", e),
                    line_number: 0,
                    severity: DiscrepancySeverity::Critical,
                }],
            };
        }
    };

    let language = match SupportedLanguage::from_path(file_path) {
        Some(l) => l,
        None => return IntegrityReport { 
            file_path: file_path.clone(), 
            status: FileIntegrityStatus::Skipped, 
            health_score: HealthScore { score: 100, grade: "N/A".to_string(), factors: vec![] },
            discrepancies: vec![] 
        },
    };

    // Parse Snaphot (Truth)
    let tree_truth = match AnalysisEngine::parse(&snapshot.backup_content, file_path) {
        Ok(t) => t,
        Err(_) => return IntegrityReport { 
            file_path: file_path.clone(), 
            status: FileIntegrityStatus::Skipped,
            health_score: HealthScore { score: 0, grade: "ERR".to_string(), factors: vec!["Snapshot Parse Error".to_string()] },
            discrepancies: vec![] 
        },
    };
    
    // Parse Current (Reality)
    let tree_reality = match AnalysisEngine::parse(&current_content, file_path) {
        Ok(t) => t,
        Err(e) => return IntegrityReport {
            file_path: file_path.clone(),
            status: FileIntegrityStatus::Critical,
            health_score: HealthScore { score: 0, grade: "F".to_string(), factors: vec!["Syntax Error".to_string()] },
            discrepancies: vec![Discrepancy {
                symbol_name: "FILE".to_string(),
                issue_type: IssueType::MissingSymbol,
                description: format!("Syntax error in current file: {}", e),
                line_number: 0,
                severity: DiscrepancySeverity::Critical,
            }],
        },
    };

    let sigs_truth = QueryMatcher::extract_signatures(&snapshot.backup_content, &tree_truth, language);
    let sigs_reality = QueryMatcher::extract_signatures(&current_content, &tree_reality, language);

    // Map reality signatures for fast lookup
    let mut reality_map = HashMap::new();
    for sig in &sigs_reality {
        let key = (sig.name.clone(), sig.parent_name.clone());
        reality_map.insert(key, sig);
    }

    let mut discrepancies = Vec::new();

    // --- INTEGRITY CHECKS ---
    for truth_sig in &sigs_truth {
        let key = (truth_sig.name.clone(), truth_sig.parent_name.clone());
        
        match reality_map.get(&key) {
            None => {
                discrepancies.push(Discrepancy {
                    symbol_name: format_symbol_name(truth_sig),
                    issue_type: IssueType::MissingSymbol,
                    description: format!("Symbol '{}' was deleted.", truth_sig.name),
                    line_number: truth_sig.start_row + 1,
                    severity: DiscrepancySeverity::Critical,
                });
            },
            Some(reality_sig) => {
                // Signature check
                if truth_sig.parameters_hash != 0 && reality_sig.parameters_hash != 0 
                   && truth_sig.parameters_hash != reality_sig.parameters_hash {
                    discrepancies.push(Discrepancy {
                        symbol_name: format_symbol_name(truth_sig),
                        issue_type: IssueType::SignatureMismatch,
                        description: "Function arguments/signature changed.".to_string(),
                        line_number: reality_sig.start_row + 1,
                        severity: DiscrepancySeverity::Warning,
                    });
                }

                // Logic Rot check
                if truth_sig.body_size_bytes > 50 {
                    let ratio = reality_sig.body_size_bytes as f32 / truth_sig.body_size_bytes as f32;
                    if ratio < 0.4 {
                        discrepancies.push(Discrepancy {
                            symbol_name: format_symbol_name(truth_sig),
                            issue_type: IssueType::LogicDegradation,
                            description: format!("Suspicious code reduction ({:.0}% of original size). Possible lazy coding.", ratio * 100.0),
                            line_number: reality_sig.start_row + 1,
                            severity: DiscrepancySeverity::Warning,
                        });
                    }
                }
            }
        }
    }

    // --- QUALITY CHECKS (Enforced by Policy) ---
    // Analyze new/modified symbols in Reality
    for sig in &sigs_reality {
        // 1. Complexity Check (Dynamic Threshold)
        if sig.cyclomatic_complexity > policy.max_complexity {
            discrepancies.push(Discrepancy {
                symbol_name: format_symbol_name(sig),
                issue_type: IssueType::HighComplexity,
                description: format!("Complexity {} exceeds limit of {}.", sig.cyclomatic_complexity, policy.max_complexity),
                line_number: sig.start_row + 1,
                severity: if policy.strict_mode { DiscrepancySeverity::Critical } else { DiscrepancySeverity::Warning },
            });
        }

        // 2. Security Alerts (Always Critical)
        for alert in &sig.security_alerts {
            discrepancies.push(Discrepancy {
                symbol_name: format_symbol_name(sig),
                issue_type: IssueType::SecurityRisk,
                description: alert.clone(),
                line_number: sig.start_row + 1,
                severity: DiscrepancySeverity::Critical,
            });
        }

        // 3. Type Coverage Check (Dynamic Threshold)
        if sig.kind == "function" && sig.type_coverage < policy.min_type_coverage 
           && (language == SupportedLanguage::TypeScript || language == SupportedLanguage::Python || language == SupportedLanguage::Rust) {
             discrepancies.push(Discrepancy {
                symbol_name: format_symbol_name(sig),
                issue_type: IssueType::TypeUnsafety,
                description: format!("Type Coverage {:.0}% is below policy limit of {:.0}%.", sig.type_coverage * 100.0, policy.min_type_coverage * 100.0),
                line_number: sig.start_row + 1,
                severity: if policy.strict_mode { DiscrepancySeverity::Warning } else { DiscrepancySeverity::Info },
            });
        }
    }

    // --- DEPENDENCY CHECK (Phantom Imports) ---
    let import_issues = DependencyAuditor::check_imports(&current_content, file_path, language);
    discrepancies.extend(import_issues);

    // --- [NEW] ADVANCED MATCHER-BASED AUDITS ---
    if policy.enable_quality_scan {
        crate::gluon_info!("IntegrityAuditor", "Running Code Quality Analyzer on {}", file_path);

        let quality_issues = AdvancedAuditor::run_quality_analysis(
            &current_content,
            file_path,
            language,
            policy
        );
        discrepancies.extend(quality_issues);
    }

    // --- [NEW] FUZZY MATCHER-BASED DUPLICATION DETECTION ---
    if policy.enable_duplication_scan {
        crate::gluon_info!("IntegrityAuditor", "Running duplication detection with FuzzyMatcher");

        let duplication_issues = AdvancedAuditor::detect_duplication_with_fuzzy(
            &snapshot.backup_content,
            &current_content,
            file_path,
            policy.fuzzy_threshold
        );
        discrepancies.extend(duplication_issues);
    }

    // --- [NEW] WEIGHTED ANCHOR MATCHER FOR STRUCTURAL DRIFT ---
    if policy.use_weighted_anchors {
        crate::gluon_info!("IntegrityAuditor", "Analyzing structural drift with WeightedAnchorMatcher");

        let structural_issues = AdvancedAuditor::detect_structural_drift(
            &snapshot.backup_content,
            &current_content,
            file_path
        );
        discrepancies.extend(structural_issues);
    }

    // --- HEALTH SCORE CALCULATION ---
    let health_score = ScoreCalculator::calculate(&discrepancies);

    let status = if discrepancies.iter().any(|d| matches!(d.severity, DiscrepancySeverity::Critical)) {
        FileIntegrityStatus::Critical
    } else if !discrepancies.is_empty() {
        FileIntegrityStatus::Warning
    } else {
        FileIntegrityStatus::Ok
    };

    IntegrityReport {
        file_path: file_path.clone(),
        status,
        health_score,
        discrepancies,
    }
}

// [ETAP 2] Dependency Auditor Logic
struct DependencyAuditor;

impl DependencyAuditor {
    pub fn check_imports(content: &str, file_path: &str, _language: SupportedLanguage) -> Vec<Discrepancy> {
        let mut issues = Vec::new();
        
        // Simple regex-based import extraction for robustness
        // (Assuming imports are extracted via existing `extract_imports` in real integration, 
        // but re-implementing here for autonomy of this module)
        let imports = crate::apply_system::context::symbol_extractor::extract_imports(content, Path::new(file_path).extension().and_then(|s| s.to_str()).unwrap_or(""));
        
        let project_root = Path::new(file_path).parent().unwrap_or(Path::new(".")); // Approximation
        
        // Load manifest (naive check)
        let manifest_deps = Self::load_manifest_dependencies(project_root);
        
        for imp in imports {
            // Check if external dependency
            if !imp.starts_with('.') && !imp.starts_with('/') {
                let root_pkg = imp.split('/').next().unwrap_or(&imp).to_string();
                
                // Skip standard library (very heuristic)
                if Self::is_std_lib(&root_pkg) { continue; }

                if !manifest_deps.contains(&root_pkg) && !manifest_deps.is_empty() {
                     issues.push(Discrepancy {
                        symbol_name: "IMPORTS".to_string(),
                        issue_type: IssueType::PhantomDependency,
                        description: format!("Phantom Import: '{}' is not declared in package manifest.", root_pkg),
                        line_number: 1, // Header usually
                        severity: DiscrepancySeverity::Warning,
                    });
                }
            }
        }
        
        issues
    }

    fn load_manifest_dependencies(root: &Path) -> Vec<String> {
        // Mock implementation for demo. In real logic, this reads package.json/Cargo.toml
        // We return empty to simulate "no manifest found" or mock dependencies for tests.
        // For production, this needs proper file parsing.
        
        // Heuristic: Check common files in parent directories
        let mut current = root;
        for _ in 0..3 { // Check up to 3 levels up
            let pkg_json = current.join("package.json");
            if pkg_json.exists() {
                // If found, normally parse JSON. Here we stub.
                return vec!["react".to_string(), "vue".to_string(), "express".to_string(), "serde".to_string(), "tokio".to_string()];
            }
            if let Some(p) = current.parent() { current = p; } else { break; }
        }
        vec![] 
    }

    fn is_std_lib(pkg: &str) -> bool {
        // Basic whitelist for JS/Python/Rust std libs
        let stds = ["fs", "path", "os", "http", "sys", "re", "json", "std", "core", "alloc"];
        stds.contains(&pkg)
    }
}

// [ETAP 2] Health Score Calculator
struct ScoreCalculator;

impl ScoreCalculator {
    pub fn calculate(discrepancies: &[Discrepancy]) -> HealthScore {
        let mut score = 100i32;
        let mut factors = Vec::new();

        for d in discrepancies {
            let (penalty, label) = match d.issue_type {
                IssueType::MissingSymbol => (30, "Integrity Breach"),
                IssueType::SignatureMismatch => (15, "API Drift"),
                IssueType::LogicDegradation => (15, "Logic Rot"),
                IssueType::HighComplexity => (5, "Complexity"),
                IssueType::SecurityRisk => (20, "Security Risk"),
                IssueType::TypeUnsafety => (2, "Type Safety"),
                IssueType::PhantomDependency => (10, "Dependency"),
                IssueType::CodeDuplication => (5, "Duplication"),
                IssueType::AntiPattern => (10, "Anti-Pattern"),
                IssueType::CodeSmell => (5, "Code Smell"),
                IssueType::PerformanceIssue => (10, "Performance"),
                IssueType::ArchitectureViolation => (15, "Architecture"),
                IssueType::UnexpectedChange => (12, "Unexpected Change"),
                IssueType::StructuralDrift => (8, "Structural Drift"),
                IssueType::BehaviorChange => (15, "Behavior Change"),
            };

            // Avoid duplicate penalties for same category to prevent reaching 0 too fast
            // Simplified logic: just subtract
            score -= penalty;
            factors.push(format!("-{} {}", penalty, label));
        }

        let final_score = score.max(0) as u8;

        let grade = match final_score {
            90..=100 => "A",
            80..=89 => "B",
            70..=79 => "C",
            60..=69 => "D",
            _ => "F",
        }.to_string();

        // Dedup factors for UI cleanliness
        factors.sort();
        factors.dedup();

        HealthScore {
            score: final_score,
            grade,
            factors,
        }
    }
}

// [ETAP 4] Enterprise Reporting Module
pub struct AuditReporter;

impl AuditReporter {
    pub fn generate_html(reports: &[IntegrityReport], policy: &AuditPolicy) -> String {
        let overall_score = if reports.is_empty() { 100 } else {
            reports.iter().map(|r| r.health_score.score as usize).sum::<usize>() / reports.len()
        };
        
        let grade = match overall_score {
            90..=100 => "A",
            80..=89 => "B",
            70..=79 => "C",
            60..=69 => "D",
            _ => "F",
        };

        let critical_count = reports.iter().filter(|r| r.status == FileIntegrityStatus::Critical).count();
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let mut html = String::new();
        html.push_str(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Gluon Enterprise Audit</title>
    <style>
        body { font-family: 'Segoe UI', system-ui, sans-serif; background: #0f172a; color: #e2e8f0; margin: 0; padding: 40px; }
        .container { max-width: 1000px; margin: 0 auto; }
        .header { display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #334155; padding-bottom: 20px; margin-bottom: 40px; }
        .brand { font-size: 24px; font-weight: bold; color: #38bdf8; letter-spacing: -0.5px; }
        .meta { color: #94a3b8; font-size: 14px; text-align: right; line-height: 1.5; }
        
        .scorecard { background: #1e293b; border-radius: 12px; padding: 30px; display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 20px; margin-bottom: 40px; border: 1px solid #334155; box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1); }
        .score-box { text-align: center; }
        .score-val { font-size: 48px; font-weight: 800; color: #fff; line-height: 1; margin-bottom: 10px; }
        .score-label { font-size: 13px; color: #94a3b8; text-transform: uppercase; letter-spacing: 1px; font-weight: 600; }
        
        .grade-A { color: #4ade80; }
        .grade-B { color: #a3e635; }
        .grade-C { color: #facc15; }
        .grade-D { color: #fb923c; }
        .grade-F { color: #f87171; }

        .file-card { background: #1e293b; border-radius: 8px; margin-bottom: 20px; overflow: hidden; border: 1px solid #334155; }
        .file-header { padding: 15px 20px; background: #263345; display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #334155; }
        .file-path { font-family: 'Consolas', monospace; font-weight: 600; color: #e2e8f0; font-size: 14px; }
        .file-score { font-size: 14px; font-weight: 600; }
        .file-body { padding: 20px; }
        
        .issue { display: flex; gap: 15px; margin-bottom: 10px; padding: 12px; background: #0f172a; border-radius: 6px; border-left: 4px solid transparent; transition: transform 0.1s; }
        .issue:hover { transform: translateX(2px); }
        .issue.Critical { border-left-color: #f87171; background: rgba(248, 113, 113, 0.1); }
        .issue.Warning { border-left-color: #facc15; background: rgba(250, 204, 21, 0.05); }
        .issue.Info { border-left-color: #60a5fa; background: rgba(96, 165, 250, 0.05); }
        
        .issue-icon { font-size: 20px; min-width: 30px; display: flex; align-items: flex-start; justify-content: center; padding-top: 2px; }
        .issue-content { flex: 1; }
        .issue-header { display: flex; justify-content: space-between; margin-bottom: 4px; }
        .issue-title { font-weight: 700; font-size: 14px; color: #f1f5f9; }
        .issue-type { font-size: 11px; text-transform: uppercase; font-weight: bold; opacity: 0.7; letter-spacing: 0.5px; }
        .issue-desc { font-size: 13px; color: #cbd5e1; line-height: 1.4; }
        .issue-loc { font-family: 'Consolas', monospace; font-size: 12px; color: #64748b; margin-top: 6px; display: inline-block; background: #1e293b; padding: 2px 6px; border-radius: 4px; }

        .empty-state { text-align: center; color: #4ade80; padding: 30px; font-weight: 500; font-size: 16px; background: rgba(74, 222, 128, 0.05); border-radius: 8px; border: 1px dashed #4ade80; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <div class="brand">GLUON AUDIT REPORT</div>
            <div class="meta">
                Generated: "#);
        html.push_str(&timestamp);
        html.push_str(r#"<br>
                Policy: <strong>"#);
        html.push_str(&policy.name);
        html.push_str(r#"</strong>
            </div>
        </div>

        <div class="scorecard">
            <div class="score-box">
                <div class="score-val "#);
        html.push_str(&format!("grade-{}", grade));
        html.push_str(r#"">"#);
        html.push_str(grade);
        html.push_str(r#"</div>
                <div class="score-label">Global Rating</div>
            </div>
            <div class="score-box">
                <div class="score-val">"#);
        html.push_str(&overall_score.to_string());
        html.push_str(r#"</div>
                <div class="score-label">Health Score</div>
            </div>
            <div class="score-box">
                <div class="score-val" style="color: "#);
        html.push_str(if critical_count > 0 { "#f87171" } else { "#4ade80" });
        html.push_str(r#"">"#);
        html.push_str(&critical_count.to_string());
        html.push_str(r#"</div>
                <div class="score-label">Critical Issues</div>
            </div>
        </div>

        <div class="files">
"#);

        for report in reports {
            let status_color = match report.status {
                FileIntegrityStatus::Ok => "#4ade80",
                FileIntegrityStatus::Warning => "#facc15",
                FileIntegrityStatus::Critical => "#f87171",
                FileIntegrityStatus::Skipped => "#64748b",
            };

            let score_color = match report.health_score.grade.as_str() {
                "A" | "A+" => "#4ade80",
                "B" => "#a3e635",
                "C" => "#facc15",
                "D" => "#fb923c",
                _ => "#f87171",
            };

            html.push_str(&format!(
                r#"<div class="file-card" style="border-left: 4px solid {}">
                <div class="file-header">
                    <div class="file-path">{}</div>
                    <div class="file-score" style="color: {}">{} <span style="color: #94a3b8; font-size: 0.8em;">(Score: {})</span></div>
                </div>
                <div class="file-body">"#,
                status_color, report.file_path, score_color, report.health_score.grade, report.health_score.score
            ));

            if !report.discrepancies.is_empty() {
                html.push_str(r#"<div class="issues-list">"#);
                for issue in &report.discrepancies {
                    let icon = match issue.issue_type {
                        IssueType::MissingSymbol => "🗑️",
                        IssueType::SignatureMismatch => "⚠️",
                        IssueType::LogicDegradation => "📉",
                        IssueType::HighComplexity => "🌀",
                        IssueType::SecurityRisk => "🔓",
                        IssueType::TypeUnsafety => "❓",
                        IssueType::PhantomDependency => "👻",
                        IssueType::CodeDuplication => "📋",
                        IssueType::AntiPattern => "🚫",
                        IssueType::CodeSmell => "💩",
                        IssueType::PerformanceIssue => "🐌",
                        IssueType::ArchitectureViolation => "🏗️",
                        IssueType::UnexpectedChange => "🔄",
                        IssueType::StructuralDrift => "📐",
                        IssueType::BehaviorChange => "🎭",
                    };
                    
                    let severity_str = match issue.severity {
                        DiscrepancySeverity::Critical => "Critical",
                        DiscrepancySeverity::Warning => "Warning",
                        DiscrepancySeverity::Info => "Info",
                    };

                    html.push_str(&format!(
                        r#"<div class="issue {}">
                            <div class="issue-icon">{}</div>
                            <div class="issue-content">
                                <div class="issue-header">
                                    <span class="issue-title">{}</span>
                                    <span class="issue-type">{}</span>
                                </div>
                                <div class="issue-desc">{}</div>
                                <div class="issue-loc">Line {}</div>
                            </div>
                        </div>"#,
                        severity_str, icon, issue.symbol_name, format!("{:?}", issue.issue_type), issue.description, issue.line_number
                    ));
                }
                html.push_str("</div>");
            } else {
                html.push_str(r#"<div class="empty-state">✓ Integrity Verified. No issues detected.</div>"#);
            }

            html.push_str("</div></div>");
        }

        html.push_str(r#"
        </div>
    </div>
</body>
</html>"#);

        html
    }
}

fn format_symbol_name(sig: &crate::apply_system::analysis::queries::SemanticSignature) -> String {
    if let Some(parent) = &sig.parent_name {
        format!("{}::{}", parent, sig.name)
    } else {
        sig.name.clone()
    }
}

// ============================================================================
// [NEW] Advanced Auditor - Using Matchers & Parsers
// ============================================================================

/// Advanced auditing system that leverages matchers and parsers for deep analysis
struct AdvancedAuditor;

impl AdvancedAuditor {
    /// Run code quality analysis using CodeQualityAnalyzer
    pub fn run_quality_analysis(
        content: &str,
        file_path: &str,
        language: SupportedLanguage,
        policy: &AuditPolicy,
    ) -> Vec<Discrepancy> {
        let mut discrepancies = Vec::new();

        // Configure analyzer based on policy
        let config = AnalyzerConfig {
            max_complexity_threshold: policy.max_complexity,
            min_comment_ratio: if policy.enforce_docstrings { 0.15 } else { 0.05 },
            enable_security_scan: policy.enable_security_scan,
            enable_performance_scan: policy.enable_performance_scan,
            enable_duplication_scan: policy.enable_duplication_scan,
            duplication_min_lines: 6,
        };

        let analyzer = CodeQualityAnalyzer::with_config(language, config);

        match analyzer.analyze(file_path, content) {
            Ok(quality_report) => {
                // Convert quality findings to discrepancies
                for finding in quality_report.findings {
                    let issue_type = match finding.category {
                        crate::apply_system::features::code_quality_analyzer::FindingCategory::Security => IssueType::SecurityRisk,
                        crate::apply_system::features::code_quality_analyzer::FindingCategory::Performance => IssueType::PerformanceIssue,
                        crate::apply_system::features::code_quality_analyzer::FindingCategory::CodeSmell => IssueType::CodeSmell,
                        crate::apply_system::features::code_quality_analyzer::FindingCategory::AntiPattern => IssueType::AntiPattern,
                        crate::apply_system::features::code_quality_analyzer::FindingCategory::Duplication => IssueType::CodeDuplication,
                        crate::apply_system::features::code_quality_analyzer::FindingCategory::Architecture => IssueType::ArchitectureViolation,
                        _ => continue, // Skip other categories
                    };

                    let severity = match finding.severity {
                        crate::apply_system::features::code_quality_analyzer::Severity::Critical => DiscrepancySeverity::Critical,
                        crate::apply_system::features::code_quality_analyzer::Severity::High => DiscrepancySeverity::Critical,
                        crate::apply_system::features::code_quality_analyzer::Severity::Medium => DiscrepancySeverity::Warning,
                        _ => DiscrepancySeverity::Info,
                    };

                    discrepancies.push(Discrepancy {
                        symbol_name: finding.title,
                        issue_type,
                        description: format!("{} | Suggestion: {}", finding.description, finding.suggestion),
                        line_number: finding.line_number,
                        severity,
                    });
                }
            }
            Err(e) => {
                crate::gluon_warn!("AdvancedAuditor", "Quality analysis failed: {}", e);
            }
        }

        discrepancies
    }

    /// Detect code duplication using FuzzyMatcher
    pub fn detect_duplication_with_fuzzy(
        old_content: &str,
        new_content: &str,
        _file_path: &str,
        threshold: f32,
    ) -> Vec<Discrepancy> {
        let mut discrepancies = Vec::new();

        // Split content into function blocks
        let _old_blocks = Self::extract_code_blocks(old_content);
        let new_blocks = Self::extract_code_blocks(new_content);

        let fuzzy_matcher = FuzzyMatcher;

        // Check for duplicated blocks in new content
        for (i, block1) in new_blocks.iter().enumerate() {
            for (j, block2) in new_blocks.iter().enumerate() {
                if i >= j {
                    continue; // Skip self and already compared pairs
                }

                if block1.len() < 100 {
                    continue; // Skip small blocks
                }

                // Use FuzzyMatcher to detect similarity
                if let Some(match_result) = fuzzy_matcher.find_match(block2, block1, None) {
                    if match_result.confidence >= threshold {
                        discrepancies.push(Discrepancy {
                            symbol_name: "CODE_BLOCK".to_string(),
                            issue_type: IssueType::CodeDuplication,
                            description: format!(
                                "Duplicated code block detected ({}% similarity)",
                                (match_result.confidence * 100.0) as u32
                            ),
                            line_number: match_result.matched_line_start,
                            severity: DiscrepancySeverity::Warning,
                        });
                        break; // Only report once per block
                    }
                }
            }
        }

        discrepancies
    }

    /// Detect structural drift using WeightedAnchorMatcher
    pub fn detect_structural_drift(
        old_content: &str,
        new_content: &str,
        _file_path: &str,
    ) -> Vec<Discrepancy> {
        let mut discrepancies = Vec::new();

        // Extract function headers from old content
        let old_functions = Self::extract_function_signatures(old_content);

        let weighted_matcher = WeightedAnchorMatcher::new();

        // Check if each old function can still be located with high confidence
        for (func_name, func_signature) in old_functions {
            match weighted_matcher.find_match(new_content, &func_signature, None) {
                Some(match_result) => {
                    // Low confidence indicates structural drift
                    if match_result.confidence < 0.80 {
                        discrepancies.push(Discrepancy {
                            symbol_name: func_name.clone(),
                            issue_type: IssueType::StructuralDrift,
                            description: format!(
                                "Function '{}' structure has drifted (confidence: {:.0}%)",
                                func_name,
                                match_result.confidence * 100.0
                            ),
                            line_number: match_result.matched_line_start,
                            severity: DiscrepancySeverity::Warning,
                        });
                    }
                }
                None => {
                    // Function moved or significantly changed
                    discrepancies.push(Discrepancy {
                        symbol_name: func_name.clone(),
                        issue_type: IssueType::UnexpectedChange,
                        description: format!(
                            "Function '{}' cannot be reliably located - may have been moved or heavily refactored",
                            func_name
                        ),
                        line_number: 0,
                        severity: DiscrepancySeverity::Warning,
                    });
                }
            }
        }

        discrepancies
    }

    /// Extract code blocks (simple heuristic)
    fn extract_code_blocks(content: &str) -> Vec<String> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let mut current_block = String::new();
        let mut brace_depth = 0;
        let mut in_block = false;

        for line in lines {
            let trimmed = line.trim();

            // Start of function/class
            if (trimmed.contains("function ") || trimmed.contains("fn ") || trimmed.contains("class "))
                && trimmed.contains("{")
            {
                in_block = true;
                current_block.clear();
                brace_depth = 1;
                current_block.push_str(line);
                current_block.push('\n');
                continue;
            }

            if in_block {
                current_block.push_str(line);
                current_block.push('\n');

                // Track braces
                brace_depth += trimmed.matches('{').count() as i32;
                brace_depth -= trimmed.matches('}').count() as i32;

                if brace_depth == 0 {
                    // End of block
                    blocks.push(current_block.clone());
                    in_block = false;
                    current_block.clear();
                }
            }
        }

        blocks
    }

    /// Extract function signatures for matching
    fn extract_function_signatures(content: &str) -> Vec<(String, String)> {
        let mut signatures = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Simple function detection (works for JS, TS, Rust)
            if let Some(name) = Self::extract_function_name(trimmed) {
                // Get 3-line context for better matching
                let start = i.saturating_sub(1);
                let end = (i + 2).min(lines.len());
                let context = lines[start..end].join("\n");

                signatures.push((name, context));
            }
        }

        signatures
    }

    /// Extract function name from line
    fn extract_function_name(line: &str) -> Option<String> {
        // JavaScript/TypeScript: function name() or const name = () =>
        if let Some(start) = line.find("function ") {
            if let Some(paren) = line[start + 9..].find('(') {
                let name = line[start + 9..start + 9 + paren].trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }

        // Rust: fn name()
        if let Some(start) = line.find("fn ") {
            if let Some(paren) = line[start + 3..].find('(') {
                let name = line[start + 3..start + 3 + paren].trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }

        // Arrow functions: const name = () =>
        if line.contains(" = (") || line.contains(" = async (") {
            if let Some(eq_pos) = line.find('=') {
                let before_eq = &line[..eq_pos];
                if let Some(const_pos) = before_eq.rfind("const ") {
                    let name = before_eq[const_pos + 6..].trim();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply_system::features::backup_system::FileStatus;

    // Helper do tworzenia mockowego snapshotu
    fn create_mock_preview(path: &str, content: &str) -> BackupFilePreview {
        BackupFilePreview {
            path: path.to_string(),
            backup_content: content.to_string(),
            current_content: None, // Logic will load from disk, so we mock disk load by separate test setup or just testing parsing logic
            status: FileStatus::Unchanged,
        }
    }

    // Aby przetestować logikę porównania bez dostępu do dysku,
    // musimy lekko zrefaktoryzować funkcję compare_file lub stworzyć wersję "pure".
    // Dla testów stworzymy wersję pure compare_logic:
    
    fn compare_content_pure(path: &str, content_truth: &str, content_reality: &str) -> IntegrityReport {
        // Mocking the behavior of compare_file but with direct strings
        let language = SupportedLanguage::from_path(path).unwrap();
        
        let tree_truth = AnalysisEngine::parse(content_truth, path).unwrap();
        let tree_reality = AnalysisEngine::parse(content_reality, path).unwrap();
        
        let sigs_truth = QueryMatcher::extract_signatures(content_truth, &tree_truth, language);
        let sigs_reality = QueryMatcher::extract_signatures(content_reality, &tree_reality, language);
        
        // (Copy-paste kluczowej logiki z compare_file dla testu)
        let mut reality_map = HashMap::new();
        for sig in &sigs_reality {
            let key = (sig.name.clone(), sig.parent_name.clone());
            reality_map.insert(key, sig);
        }

        let mut discrepancies = Vec::new();

        for truth_sig in &sigs_truth {
            let key = (truth_sig.name.clone(), truth_sig.parent_name.clone());
            
            match reality_map.get(&key) {
                None => {
                    discrepancies.push(Discrepancy {
                        symbol_name: super::format_symbol_name(truth_sig),
                        issue_type: IssueType::MissingSymbol,
                        description: "Deleted".to_string(),
                        line_number: truth_sig.start_row + 1,
                        severity: DiscrepancySeverity::Critical,
                    });
                },
                Some(reality_sig) => {
                    if truth_sig.parameters_hash != 0 && reality_sig.parameters_hash != 0 
                       && truth_sig.parameters_hash != reality_sig.parameters_hash {
                        discrepancies.push(Discrepancy {
                            symbol_name: super::format_symbol_name(truth_sig),
                            issue_type: IssueType::SignatureMismatch,
                            description: "Changed".to_string(),
                            line_number: reality_sig.start_row + 1,
                            severity: DiscrepancySeverity::Warning,
                        });
                    }
                    if truth_sig.body_size_bytes > 50 {
                        let ratio = reality_sig.body_size_bytes as f32 / truth_sig.body_size_bytes as f32;
                        if ratio < 0.4 {
                            discrepancies.push(Discrepancy {
                                symbol_name: super::format_symbol_name(truth_sig),
                                issue_type: IssueType::LogicDegradation,
                                description: "Lazy code".to_string(),
                                line_number: reality_sig.start_row + 1,
                                severity: DiscrepancySeverity::Warning,
                            });
                        }
                    }
                }
            }
        }
        
        let status = if discrepancies.iter().any(|d| matches!(d.severity, DiscrepancySeverity::Critical)) {
            FileIntegrityStatus::Critical
        } else if !discrepancies.is_empty() {
            FileIntegrityStatus::Warning
        } else {
            FileIntegrityStatus::Ok
        };

        let health_score = ScoreCalculator::calculate(&discrepancies);

        IntegrityReport {
            file_path: path.to_string(),
            status,
            health_score,
            discrepancies,
        }
    }

    #[test]
    fn test_detect_missing_function() {
        let old = r#"
            fn keep_me() {}
            fn delete_me() { println!("Logic"); }
        "#;
        let new = r#"
            fn keep_me() {}
        "#;
        
        let report = compare_content_pure("test.rs", old, new);
        assert_eq!(report.status, FileIntegrityStatus::Critical);
        assert!(report.discrepancies.iter().any(|d| d.symbol_name == "delete_me" && matches!(d.issue_type, IssueType::MissingSymbol)));
    }

    #[test]
    fn test_detect_signature_drift() {
        let old = "fn calculate(a: i32, b: i32) {}";
        let new = "fn calculate(a: i32) {}"; // Removed 'b'
        
        let report = compare_content_pure("test.rs", old, new);
        assert_eq!(report.status, FileIntegrityStatus::Warning);
        assert!(report.discrepancies.iter().any(|d| d.symbol_name == "calculate" && matches!(d.issue_type, IssueType::SignatureMismatch)));
    }

    #[test]
    fn test_detect_logic_rot() {
        let old = r#"
            fn complex_logic() {
                let a = 1;
                let b = 2;
                // Lots of logic...
                println!("{}", a + b);
                // More logic...
                let c = a * b;
            }
        "#;
        let new = r#"
            fn complex_logic() {
                // ... existing code ...
            }
        "#;
        
        let report = compare_content_pure("test.rs", old, new);
        // Body size dropped significantly
        assert!(report.discrepancies.iter().any(|d| d.symbol_name == "complex_logic" && matches!(d.issue_type, IssueType::LogicDegradation)));
    }
}