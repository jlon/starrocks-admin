//! Root Cause Analysis Engine (v5.0)
//!
//! Implements multi-dimensional causal analysis without LLM:
//! 1. Intra-Node Causality - same node, multiple diagnostics
//! 2. Inter-Node Propagation - DAG-based propagation
//! 3. Multiple Root Causes - identify all independent root causes
//! 4. Causal Graph - build and visualize causal relationships

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use super::rules::Diagnostic;

// ============================================================================
// Root Cause Analysis Result Types
// ============================================================================

/// Complete root cause analysis result
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RootCauseAnalysis {
    /// Identified root causes (sorted by impact)
    pub root_causes: Vec<RootCause>,
    /// Causal chains explaining how root causes lead to symptoms
    pub causal_chains: Vec<CausalChain>,
    /// Natural language summary
    pub summary: String,
    /// Total number of diagnostics analyzed
    pub total_diagnostics: usize,
}

/// A single root cause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCause {
    /// Unique ID (e.g., "RC001")
    pub id: String,
    /// Related diagnostic rule IDs
    pub diagnostic_ids: Vec<String>,
    /// Description of the root cause
    pub description: String,
    /// Impact percentage (0-100)
    pub impact_percentage: f64,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Node paths affected
    pub affected_nodes: Vec<String>,
    /// Evidence supporting this conclusion
    pub evidence: Vec<String>,
    /// Symptoms caused by this root cause
    pub symptoms: Vec<String>,
    /// Suggested actions
    pub suggestions: Vec<String>,
}

/// A causal chain from root cause to symptom
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalChain {
    /// Chain elements: ["Root Cause", "→", "Intermediate", "→", "Symptom"]
    pub chain: Vec<String>,
    /// Explanation of this causal relationship
    pub explanation: String,
    /// Confidence score
    pub confidence: f64,
}

// ============================================================================
// Intra-Node Causality Rules
// ============================================================================

/// Rule defining causality within the same node
struct IntraNodeRule {
    /// Cause diagnostic IDs (any of these)
    causes: &'static [&'static str],
    /// Effect diagnostic ID
    effect: &'static str,
    /// Description of the causal relationship
    description: &'static str,
}

/// Predefined intra-node causality rules based on v5.0 design
const INTRA_NODE_RULES: &[IntraNodeRule] = &[
    // SCAN node causality
    IntraNodeRule {
        causes: &["S016", "S006"],  // Small files, Rowset fragmentation
        effect: "S007",             // IO bottleneck
        description: "小文件/碎片化导致IO瓶颈",
    },
    IntraNodeRule {
        causes: &["S009"],          // Low cache hit
        effect: "S007",             // IO bottleneck
        description: "缓存命中率低导致IO瓶颈",
    },
    IntraNodeRule {
        causes: &["S008", "S012", "S013"],  // ZoneMap, Bitmap, BloomFilter issues
        effect: "S003",                      // Poor filter effectiveness
        description: "索引未生效导致过滤效果差",
    },
    IntraNodeRule {
        causes: &["S001"],          // Data skew
        effect: "G003",             // Execution time skew
        description: "数据倾斜导致执行时间倾斜",
    },
    // JOIN node causality
    IntraNodeRule {
        causes: &["J002"],          // Suboptimal join order
        effect: "J001",             // Hash table too large
        description: "Join顺序不优导致Hash表过大",
    },
    IntraNodeRule {
        causes: &["J005"],          // Broadcast table too large
        effect: "E002",             // Network bottleneck
        description: "Broadcast表过大导致网络瓶颈",
    },
    // AGG node causality
    IntraNodeRule {
        causes: &["A001"],          // Aggregation skew
        effect: "Q003",             // Spill occurred
        description: "聚合倾斜导致内存溢出",
    },
    IntraNodeRule {
        causes: &["A003"],          // Too many group by keys
        effect: "A002",             // Large hash table
        description: "Group by 键过多导致Hash表过大",
    },
];

// ============================================================================
// Inter-Node Propagation Rules
// ============================================================================

/// Propagation mode between nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropagationMode {
    /// Data volume propagates: upstream data → downstream processing time
    DataVolume,
    /// Skew propagates: upstream skew → downstream skew
    Skew,
    /// Memory pressure propagates: sibling memory usage → spill
    Memory,
    /// IO wait propagates: upstream IO → downstream stall
    IoWait,
}

/// Rule for inter-node propagation
struct InterNodeRule {
    /// Upstream diagnostic pattern
    upstream: &'static str,
    /// Downstream diagnostic pattern
    downstream: &'static str,
    /// Propagation mode
    mode: PropagationMode,
    /// Description
    description: &'static str,
}

const INTER_NODE_RULES: &[InterNodeRule] = &[
    // Skew propagation
    InterNodeRule {
        upstream: "S001",   // SCAN data skew
        downstream: "G003", // Execution time skew
        mode: PropagationMode::Skew,
        description: "SCAN数据倾斜传导到下游执行时间倾斜",
    },
    InterNodeRule {
        upstream: "S001",   // SCAN data skew
        downstream: "J003", // Join probe rows skew
        mode: PropagationMode::Skew,
        description: "SCAN数据倾斜传导到Join探测端",
    },
    InterNodeRule {
        upstream: "S001",   // SCAN data skew
        downstream: "A001", // Aggregation skew
        mode: PropagationMode::Skew,
        description: "SCAN数据倾斜传导到聚合倾斜",
    },
    // Data volume propagation
    InterNodeRule {
        upstream: "S003",   // Poor filter effectiveness
        downstream: "J001", // Hash table too large
        mode: PropagationMode::DataVolume,
        description: "过滤效果差导致下游Join数据量大",
    },
    InterNodeRule {
        upstream: "S003",   // Poor filter effectiveness
        downstream: "A002", // Aggregation hash table large
        mode: PropagationMode::DataVolume,
        description: "过滤效果差导致聚合处理数据量大",
    },
    // Memory propagation
    InterNodeRule {
        upstream: "J001",   // Hash table large
        downstream: "Q003", // Spill
        mode: PropagationMode::Memory,
        description: "Join内存占用高导致触发Spill",
    },
    InterNodeRule {
        upstream: "A002",   // Aggregation hash table large
        downstream: "Q003", // Spill
        mode: PropagationMode::Memory,
        description: "聚合内存占用高导致触发Spill",
    },
];

// ============================================================================
// Root Cause Analyzer
// ============================================================================

/// Root cause analyzer implementing v5.0 design
pub struct RootCauseAnalyzer;

impl RootCauseAnalyzer {
    /// Analyze diagnostics and identify root causes
    pub fn analyze(diagnostics: &[Diagnostic]) -> RootCauseAnalysis {
        if diagnostics.is_empty() {
            return RootCauseAnalysis::default();
        }
        
        // Build diagnostic lookup
        let diag_map = Self::build_diagnostic_map(diagnostics);
        
        // Step 1: Find intra-node causal relationships
        let intra_edges = Self::find_intra_node_causality(diagnostics);
        
        // Step 2: Find inter-node causal relationships
        let inter_edges = Self::find_inter_node_propagation(diagnostics);
        
        // Step 3: Build causal graph
        let all_edges: Vec<(String, String, String)> = intra_edges.into_iter()
            .chain(inter_edges.into_iter())
            .collect();
        
        // Step 4: Find root causes (nodes with no incoming edges)
        let root_causes = Self::identify_root_causes(diagnostics, &all_edges, &diag_map);
        
        // Step 5: Build causal chains
        let causal_chains = Self::build_causal_chains(&root_causes, &all_edges, &diag_map);
        
        // Step 6: Generate summary
        let summary = Self::generate_summary(&root_causes);
        
        RootCauseAnalysis {
            root_causes,
            causal_chains,
            summary,
            total_diagnostics: diagnostics.len(),
        }
    }
    
    /// Build a map of rule_id -> diagnostics for quick lookup
    fn build_diagnostic_map(diagnostics: &[Diagnostic]) -> HashMap<String, Vec<&Diagnostic>> {
        let mut map: HashMap<String, Vec<&Diagnostic>> = HashMap::new();
        for diag in diagnostics {
            map.entry(diag.rule_id.clone()).or_default().push(diag);
        }
        map
    }
    
    /// Find intra-node causal relationships
    fn find_intra_node_causality(diagnostics: &[Diagnostic]) -> Vec<(String, String, String)> {
        let mut edges = Vec::new();
        
        // Group diagnostics by node path
        let mut by_node: HashMap<&str, Vec<&Diagnostic>> = HashMap::new();
        for diag in diagnostics {
            by_node.entry(&diag.node_path).or_default().push(diag);
        }
        
        // Check each node for intra-node causality
        for (_node_path, node_diags) in by_node {
            let rule_ids: HashSet<&str> = node_diags.iter().map(|d| d.rule_id.as_str()).collect();
            
            for rule in INTRA_NODE_RULES {
                // Check if any cause is present
                let has_cause = rule.causes.iter().any(|c| rule_ids.contains(*c));
                // Check if effect is present
                let has_effect = rule_ids.contains(rule.effect);
                
                if has_cause && has_effect {
                    // Find the specific cause that's present
                    for cause in rule.causes {
                        if rule_ids.contains(*cause) {
                            edges.push((
                                cause.to_string(),
                                rule.effect.to_string(),
                                rule.description.to_string(),
                            ));
                        }
                    }
                }
            }
        }
        
        edges
    }
    
    /// Find inter-node causal relationships based on DAG
    fn find_inter_node_propagation(diagnostics: &[Diagnostic]) -> Vec<(String, String, String)> {
        let mut edges = Vec::new();
        let rule_ids: HashSet<&str> = diagnostics.iter().map(|d| d.rule_id.as_str()).collect();
        
        for rule in INTER_NODE_RULES {
            if rule_ids.contains(rule.upstream) && rule_ids.contains(rule.downstream) {
                edges.push((
                    rule.upstream.to_string(),
                    rule.downstream.to_string(),
                    rule.description.to_string(),
                ));
            }
        }
        
        edges
    }
    
    /// Identify root causes (diagnostics with no incoming causal edges)
    fn identify_root_causes(
        diagnostics: &[Diagnostic],
        edges: &[(String, String, String)],
        diag_map: &HashMap<String, Vec<&Diagnostic>>,
    ) -> Vec<RootCause> {
        // Find all rule_ids that have incoming edges (are effects)
        let effects: HashSet<&str> = edges.iter().map(|(_, effect, _)| effect.as_str()).collect();
        
        // Find rule_ids that are causes (have outgoing edges)
        let causes: HashSet<&str> = edges.iter().map(|(cause, _, _)| cause.as_str()).collect();
        
        // Root causes: present in diagnostics, not an effect of another, OR has no incoming edge
        let mut root_cause_ids: HashSet<&str> = HashSet::new();
        
        // All unique rule_ids in diagnostics
        let all_ids: HashSet<&str> = diagnostics.iter().map(|d| d.rule_id.as_str()).collect();
        
        for id in &all_ids {
            // If this ID is not caused by anything else in our graph, it's a root cause
            if !effects.contains(id) {
                root_cause_ids.insert(id);
            }
        }
        
        // If no root causes found, treat the highest severity diagnostics as root causes
        if root_cause_ids.is_empty() {
            for diag in diagnostics.iter().take(3) {
                root_cause_ids.insert(&diag.rule_id);
            }
        }
        
        // Build RootCause objects
        let mut root_causes: Vec<RootCause> = Vec::new();
        let mut rc_counter = 0;
        
        for rule_id in root_cause_ids {
            if let Some(diags) = diag_map.get(rule_id) {
                rc_counter += 1;
                let first_diag = diags[0];
                
                // Find symptoms caused by this root cause
                let symptoms: Vec<String> = edges.iter()
                    .filter(|(cause, _, _)| cause == rule_id)
                    .map(|(_, effect, _)| effect.clone())
                    .collect();
                
                // Calculate impact based on severity and symptom count
                let base_impact = match first_diag.severity {
                    super::rules::RuleSeverity::Error => 40.0,
                    super::rules::RuleSeverity::Warning => 25.0,
                    super::rules::RuleSeverity::Info => 15.0,
                };
                let symptom_bonus = symptoms.len() as f64 * 10.0;
                let impact = (base_impact + symptom_bonus).min(100.0);
                
                root_causes.push(RootCause {
                    id: format!("RC{:03}", rc_counter),
                    diagnostic_ids: vec![rule_id.to_string()],
                    description: first_diag.message.clone(),
                    impact_percentage: impact,
                    confidence: 1.0,  // Rule-based = 100% confidence
                    affected_nodes: diags.iter().map(|d| d.node_path.clone()).collect(),
                    evidence: vec![first_diag.reason.clone()],
                    symptoms,
                    suggestions: first_diag.suggestions.clone(),
                });
            }
        }
        
        // Sort by impact (descending)
        root_causes.sort_by(|a, b| b.impact_percentage.partial_cmp(&a.impact_percentage).unwrap_or(std::cmp::Ordering::Equal));
        
        root_causes
    }
    
    /// Build causal chains from root causes to symptoms
    fn build_causal_chains(
        root_causes: &[RootCause],
        edges: &[(String, String, String)],
        diag_map: &HashMap<String, Vec<&Diagnostic>>,
    ) -> Vec<CausalChain> {
        let mut chains = Vec::new();
        
        for rc in root_causes {
            for diag_id in &rc.diagnostic_ids {
                // BFS to find all paths from this root cause
                let paths = Self::find_paths_from(diag_id, edges, 3); // max depth 3
                
                for path in paths {
                    if path.len() >= 2 {
                        // Build chain representation
                        let mut chain = Vec::new();
                        let mut explanations = Vec::new();
                        
                        for (i, node_id) in path.iter().enumerate() {
                            // Get description from diagnostic
                            let desc = diag_map.get(node_id)
                                .and_then(|d| d.first())
                                .map(|d| d.rule_name.clone())
                                .unwrap_or_else(|| node_id.clone());
                            
                            chain.push(desc);
                            
                            if i < path.len() - 1 {
                                chain.push("→".to_string());
                                // Find edge description
                                if let Some((_, _, edge_desc)) = edges.iter()
                                    .find(|(cause, effect, _)| cause == node_id && effect == &path[i + 1])
                                {
                                    explanations.push(edge_desc.clone());
                                }
                            }
                        }
                        
                        let explanation = if explanations.is_empty() {
                            format!("{} 导致 {}", path[0], path.last().unwrap_or(&path[0]))
                        } else {
                            explanations.join("; ")
                        };
                        
                        chains.push(CausalChain {
                            chain,
                            explanation,
                            confidence: 1.0,
                        });
                    }
                }
            }
        }
        
        chains
    }
    
    /// Find all paths from a starting node using BFS
    fn find_paths_from(start: &str, edges: &[(String, String, String)], max_depth: usize) -> Vec<Vec<String>> {
        let mut paths = Vec::new();
        let mut queue: Vec<(Vec<String>, usize)> = vec![(vec![start.to_string()], 0)];
        
        while let Some((path, depth)) = queue.pop() {
            if depth >= max_depth {
                if path.len() > 1 {
                    paths.push(path);
                }
                continue;
            }
            
            let current = path.last().unwrap();
            let mut found_next = false;
            
            for (cause, effect, _) in edges {
                if cause == current && !path.contains(effect) {
                    found_next = true;
                    let mut new_path = path.clone();
                    new_path.push(effect.clone());
                    queue.push((new_path, depth + 1));
                }
            }
            
            if !found_next && path.len() > 1 {
                paths.push(path);
            }
        }
        
        paths
    }
    
    /// Generate a natural language summary
    fn generate_summary(root_causes: &[RootCause]) -> String {
        if root_causes.is_empty() {
            return "未发现明显的性能问题根因".to_string();
        }
        
        if root_causes.len() == 1 {
            let rc = &root_causes[0];
            if rc.symptoms.is_empty() {
                format!("发现 1 个根因: {}", rc.description)
            } else {
                format!(
                    "发现 1 个根因: {}，导致了 {} 个下游问题",
                    rc.description,
                    rc.symptoms.len()
                )
            }
        } else {
            let top_causes: Vec<&str> = root_causes.iter()
                .take(3)
                .map(|rc| rc.diagnostic_ids.first().map(|s| s.as_str()).unwrap_or(""))
                .collect();
            
            format!(
                "发现 {} 个独立根因，主要包括: {}。建议按优先级依次解决",
                root_causes.len(),
                top_causes.join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::rules::RuleSeverity;
    
    fn make_diag(rule_id: &str, node_path: &str, severity: RuleSeverity) -> Diagnostic {
        Diagnostic {
            rule_id: rule_id.to_string(),
            rule_name: format!("Rule {}", rule_id),
            severity,
            node_path: node_path.to_string(),
            plan_node_id: None,
            message: format!("Message for {}", rule_id),
            reason: format!("Reason for {}", rule_id),
            suggestions: vec![format!("Fix {}", rule_id)],
            parameter_suggestions: vec![],
        }
    }
    
    #[test]
    fn test_intra_node_causality() {
        // S016 (small files) + S007 (IO bottleneck) in same SCAN node
        let diagnostics = vec![
            make_diag("S016", "Fragment_1/Pipeline_0/SCAN", RuleSeverity::Warning),
            make_diag("S007", "Fragment_1/Pipeline_0/SCAN", RuleSeverity::Error),
        ];
        
        let result = RootCauseAnalyzer::analyze(&diagnostics);
        
        // S016 should be identified as root cause, S007 as symptom
        assert!(!result.root_causes.is_empty());
        assert!(result.root_causes.iter().any(|rc| rc.diagnostic_ids.contains(&"S016".to_string())));
        assert!(result.causal_chains.iter().any(|cc| cc.chain.iter().any(|s| s.contains("S016") || s.contains("小文件"))));
    }
    
    #[test]
    fn test_inter_node_propagation() {
        // S001 (data skew) in SCAN -> G003 (time skew) in JOIN
        let diagnostics = vec![
            make_diag("S001", "Fragment_1/Pipeline_0/SCAN", RuleSeverity::Warning),
            make_diag("G003", "Fragment_1/Pipeline_1/JOIN", RuleSeverity::Warning),
        ];
        
        let result = RootCauseAnalyzer::analyze(&diagnostics);
        
        // S001 should be root cause, G003 should be symptom
        assert!(!result.root_causes.is_empty());
        assert!(result.root_causes.iter().any(|rc| rc.diagnostic_ids.contains(&"S001".to_string())));
    }
    
    #[test]
    fn test_multiple_root_causes() {
        // Two independent issues: S016 (small files) and J002 (bad join order)
        let diagnostics = vec![
            make_diag("S016", "Fragment_1/Pipeline_0/SCAN_1", RuleSeverity::Warning),
            make_diag("J002", "Fragment_1/Pipeline_1/JOIN", RuleSeverity::Warning),
        ];
        
        let result = RootCauseAnalyzer::analyze(&diagnostics);
        
        // Both should be identified as root causes (no causal relationship between them)
        assert_eq!(result.root_causes.len(), 2);
    }
}
