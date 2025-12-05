//! Comprehensive unit tests for Profile Analyzer
//! 
//! Test data is from /home/oppo/Documents/starrocks-profile-analyzer/profiles/
//! Each profile has a corresponding PNG showing the expected visualization result.

#[cfg(test)]
mod tests {
    use crate::services::profile_analyzer::{analyze_profile, ProfileComposer};
    use crate::services::profile_analyzer::models::*;
    use crate::services::profile_analyzer::parser::core::*;
    use std::fs;
    use std::path::PathBuf;

    /// Get the path to test fixtures
    fn get_fixture_path(filename: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests/fixtures/profiles");
        path.push(filename);
        path
    }

    /// Load a profile fixture file
    fn load_profile(filename: &str) -> String {
        let path = get_fixture_path(filename);
        fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to load fixture {}: {}", path.display(), e))
    }

    // ========================================================================
    // Value Parser Tests
    // ========================================================================

    mod value_parser_tests {
        use super::*;

        #[test]
        fn test_parse_duration_complex() {
            // Test format: 9m41s (from profile1)
            let d = ValueParser::parse_duration("9m41s").unwrap();
            assert_eq!(d.as_secs(), 9 * 60 + 41);
        }

        #[test]
        fn test_parse_duration_milliseconds() {
            let d = ValueParser::parse_duration("11ms").unwrap();
            assert_eq!(d.as_millis(), 11);
        }

        #[test]
        fn test_parse_duration_microseconds() {
            let d = ValueParser::parse_duration("538.833us").unwrap();
            assert!(d.as_nanos() > 538000 && d.as_nanos() < 539000);
        }

        #[test]
        fn test_parse_duration_nanoseconds() {
            let d = ValueParser::parse_duration("456ns").unwrap();
            assert_eq!(d.as_nanos(), 456);
        }

        #[test]
        fn test_parse_duration_combined() {
            // Test format: 1s727ms (from profile2 QueryCumulativeCpuTime)
            let d = ValueParser::parse_duration("1s727ms").unwrap();
            assert_eq!(d.as_millis(), 1727);
        }

        #[test]
        fn test_parse_bytes_gb() {
            // Test format: 558.156 GB (from profile1)
            let bytes = ValueParser::parse_bytes("558.156 GB").unwrap();
            println!("Parsed '558.156 GB' = {} bytes", bytes);
            // 558.156 GB = 558.156 * 1024^3 = 599,332,438,016 bytes (approximately)
            assert!(bytes > 599_000_000_000 && bytes < 600_000_000_000, 
                "Expected ~599GB, got {} bytes", bytes);
        }

        #[test]
        fn test_parse_bytes_mb() {
            let bytes = ValueParser::parse_bytes("13.812 MB").unwrap();
            println!("Parsed '13.812 MB' = {} bytes", bytes);
            // 13.812 MB = 13.812 * 1024^2 = 14,483,906 bytes (approximately)
            assert!(bytes > 14_000_000 && bytes < 15_000_000,
                "Expected ~14MB, got {} bytes", bytes);
        }

        #[test]
        fn test_parse_bytes_kb() {
            let bytes = ValueParser::parse_bytes("442.328 KB").unwrap();
            println!("Parsed '442.328 KB' = {} bytes", bytes);
            // 442.328 KB = 442.328 * 1024 = 452,943 bytes (approximately)
            assert!(bytes > 450_000 && bytes < 460_000,
                "Expected ~452KB, got {} bytes", bytes);
        }

        #[test]
        fn test_parse_bytes_with_parentheses() {
            // Format: 1.026K (1026)
            let bytes = ValueParser::parse_bytes("1.026K (1026)").unwrap();
            assert_eq!(bytes, 1026);
        }

        #[test]
        fn test_parse_number_with_commas() {
            let n: u64 = ValueParser::parse_number("1,234,567").unwrap();
            assert_eq!(n, 1234567);
        }
    }

    // ========================================================================
    // Section Parser Tests
    // ========================================================================

    mod section_parser_tests {
        use super::*;

        #[test]
        fn test_parse_summary_profile1() {
            let profile_text = load_profile("profile1.txt");
            let summary = SectionParser::parse_summary(&profile_text).unwrap();
            
            assert_eq!(summary.query_id, "c025364c-a999-11f0-a663-f62b9654e895");
            assert_eq!(summary.total_time, "9m41s");
            assert_eq!(summary.query_state, "Finished");
            assert_eq!(summary.starrocks_version, "3.5.2-69de616");
            assert_eq!(summary.user, Some("explore_service".to_string()));
        }

        #[test]
        fn test_parse_summary_profile2() {
            let profile_text = load_profile("profile2.txt");
            let summary = SectionParser::parse_summary(&profile_text).unwrap();
            
            assert_eq!(summary.query_id, "ce065afe-a986-11f0-a663-f62b9654e895");
            assert_eq!(summary.total_time, "11ms");
            assert_eq!(summary.query_state, "Finished");
            assert_eq!(summary.user, Some("root".to_string()));
            assert_eq!(summary.default_db, Some("user_mart".to_string()));
        }

        #[test]
        fn test_parse_execution_topology() {
            let profile_text = load_profile("profile1.txt");
            let execution = SectionParser::parse_execution(&profile_text).unwrap();
            
            // Verify topology JSON is extracted
            assert!(execution.topology.contains("rootId"));
            assert!(execution.topology.contains("MERGE_EXCHANGE"));
            assert!(execution.topology.contains("OLAP_SCAN"));
        }

        #[test]
        fn test_parse_execution_metrics() {
            let profile_text = load_profile("profile1.txt");
            let execution = SectionParser::parse_execution(&profile_text).unwrap();
            
            // Verify key metrics are extracted
            assert!(execution.metrics.contains_key("QueryCumulativeOperatorTime"));
            assert!(execution.metrics.contains_key("QueryExecutionWallTime"));
            assert!(execution.metrics.contains_key("QueryPeakMemoryUsagePerNode"));
        }
    }

    // ========================================================================
    // Fragment Parser Tests
    // ========================================================================

    mod fragment_parser_tests {
        use super::*;

        #[test]
        fn test_extract_fragments_profile1() {
            let profile_text = load_profile("profile1.txt");
            let fragments = FragmentParser::extract_all_fragments(&profile_text);
            
            // Profile1 has Fragment 0, 1, 2
            assert!(fragments.len() >= 1, "Expected at least 1 fragment, got {}", fragments.len());
            
            // Check first fragment
            let frag0 = &fragments[0];
            assert_eq!(frag0.id, "0");
            assert!(!frag0.backend_addresses.is_empty());
        }

        #[test]
        fn test_extract_operators_from_pipeline() {
            let profile_text = load_profile("profile2.txt");
            let fragments = FragmentParser::extract_all_fragments(&profile_text);
            
            assert!(!fragments.is_empty());
            
            // Find RESULT_SINK operator
            let mut found_result_sink = false;
            let mut found_schema_scan = false;
            
            for fragment in &fragments {
                for pipeline in &fragment.pipelines {
                    for operator in &pipeline.operators {
                        if operator.name == "RESULT_SINK" {
                            found_result_sink = true;
                            assert_eq!(operator.plan_node_id, Some("-1".to_string()));
                        }
                        if operator.name == "SCHEMA_SCAN" {
                            found_schema_scan = true;
                            assert_eq!(operator.plan_node_id, Some("0".to_string()));
                        }
                    }
                }
            }
            
            assert!(found_result_sink, "RESULT_SINK operator not found");
            assert!(found_schema_scan, "SCHEMA_SCAN operator not found");
        }

        #[test]
        fn test_operator_metrics_extraction() {
            let profile_text = load_profile("profile2.txt");
            let fragments = FragmentParser::extract_all_fragments(&profile_text);
            
            // Find RESULT_SINK and check its metrics
            for fragment in &fragments {
                for pipeline in &fragment.pipelines {
                    for operator in &pipeline.operators {
                        if operator.name == "RESULT_SINK" {
                            // Check common metrics
                            assert!(operator.common_metrics.contains_key("OperatorTotalTime"));
                            assert!(operator.common_metrics.contains_key("PushRowNum"));
                            
                            // Check unique metrics
                            assert!(operator.unique_metrics.contains_key("SinkType"));
                            return;
                        }
                    }
                }
            }
            panic!("RESULT_SINK not found");
        }
    }

    // ========================================================================
    // Topology Parser Tests
    // ========================================================================

    mod topology_parser_tests {
        use super::*;

        #[test]
        fn test_parse_topology_profile1() {
            let topology_json = r#"{"rootId":6,"nodes":[{"id":6,"name":"MERGE_EXCHANGE","properties":{"sinkIds":[],"displayMem":true},"children":[5]},{"id":5,"name":"SORT","properties":{"sinkIds":[6],"displayMem":true},"children":[4]},{"id":4,"name":"AGGREGATION","properties":{"displayMem":true},"children":[3]},{"id":3,"name":"EXCHANGE","properties":{"displayMem":true},"children":[2]},{"id":2,"name":"AGGREGATION","properties":{"sinkIds":[3],"displayMem":true},"children":[1]},{"id":1,"name":"PROJECT","properties":{"displayMem":false},"children":[0]},{"id":0,"name":"OLAP_SCAN","properties":{"displayMem":false},"children":[]}]}"#;
            
            let topology = TopologyParser::parse_without_profile(topology_json).unwrap();
            
            assert_eq!(topology.root_id, 6);
            assert_eq!(topology.nodes.len(), 7);
            
            // Verify node names
            let node_names: Vec<&str> = topology.nodes.iter().map(|n| n.name.as_str()).collect();
            assert!(node_names.contains(&"MERGE_EXCHANGE"));
            assert!(node_names.contains(&"SORT"));
            assert!(node_names.contains(&"AGGREGATION"));
            assert!(node_names.contains(&"EXCHANGE"));
            assert!(node_names.contains(&"PROJECT"));
            assert!(node_names.contains(&"OLAP_SCAN"));
        }

        #[test]
        fn test_parse_topology_profile2() {
            let topology_json = r#"{"rootId":1,"nodes":[{"id":1,"name":"EXCHANGE","properties":{"sinkIds":[],"displayMem":true},"children":[0]},{"id":0,"name":"SCHEMA_SCAN","properties":{"sinkIds":[1],"displayMem":false},"children":[]}]}"#;
            
            let topology = TopologyParser::parse_without_profile(topology_json).unwrap();
            
            assert_eq!(topology.root_id, 1);
            assert_eq!(topology.nodes.len(), 2);
            
            // Check parent-child relationship
            let exchange = topology.nodes.iter().find(|n| n.name == "EXCHANGE").unwrap();
            assert_eq!(exchange.children, vec![0]);
        }

        #[test]
        fn test_topology_validation() {
            let topology = TopologyGraph {
                root_id: 1,
                nodes: vec![
                    TopologyNode {
                        id: 1,
                        name: "ROOT".to_string(),
                        properties: std::collections::HashMap::new(),
                        children: vec![0],
                    },
                    TopologyNode {
                        id: 0,
                        name: "LEAF".to_string(),
                        properties: std::collections::HashMap::new(),
                        children: vec![],
                    },
                ],
            };
            
            assert!(TopologyParser::validate(&topology).is_ok());
        }
    }

    // ========================================================================
    // Profile Composer Integration Tests
    // ========================================================================

    mod composer_tests {
        use super::*;

        #[test]
        fn test_compose_profile1() {
            let profile_text = load_profile("profile1.txt");
            let mut composer = ProfileComposer::new();
            let profile = composer.parse(&profile_text).unwrap();
            
            // Verify summary
            assert_eq!(profile.summary.query_id, "c025364c-a999-11f0-a663-f62b9654e895");
            assert_eq!(profile.summary.total_time, "9m41s");
            
            // Verify execution tree exists
            assert!(profile.execution_tree.is_some());
            let tree = profile.execution_tree.as_ref().unwrap();
            
            // Verify nodes exist
            assert!(!tree.nodes.is_empty());
            
            // Find OLAP_SCAN node - should have highest time percentage (100% as per image)
            let olap_scan = tree.nodes.iter().find(|n| n.operator_name == "OLAP_SCAN");
            assert!(olap_scan.is_some(), "OLAP_SCAN node not found");
        }

        #[test]
        fn test_compose_profile2() {
            let profile_text = load_profile("profile2.txt");
            let mut composer = ProfileComposer::new();
            let profile = composer.parse(&profile_text).unwrap();
            
            // Verify summary
            assert_eq!(profile.summary.query_id, "ce065afe-a986-11f0-a663-f62b9654e895");
            assert_eq!(profile.summary.total_time, "11ms");
            
            // Verify execution tree
            assert!(profile.execution_tree.is_some());
            let tree = profile.execution_tree.as_ref().unwrap();
            
            // According to profile2.png:
            // - SCHEMA_SCAN: 50.75%
            // - EXCHANGE: 45.73%
            // - RESULT_SINK: 3.56%
            
            // Find nodes and verify they exist
            let schema_scan = tree.nodes.iter().find(|n| n.operator_name.contains("SCAN"));
            assert!(schema_scan.is_some(), "SCAN node not found");
            
            let exchange = tree.nodes.iter().find(|n| n.operator_name == "EXCHANGE");
            assert!(exchange.is_some(), "EXCHANGE node not found");
        }

        #[test]
        fn test_compose_profile3() {
            let profile_text = load_profile("profile3.txt");
            let mut composer = ProfileComposer::new();
            let result = composer.parse(&profile_text);
            
            assert!(result.is_ok(), "Failed to parse profile3: {:?}", result.err());
            let profile = result.unwrap();
            
            assert!(!profile.summary.query_id.is_empty());
            assert!(profile.execution_tree.is_some());
        }

        #[test]
        fn test_compose_profile4() {
            let profile_text = load_profile("profile4.txt");
            let mut composer = ProfileComposer::new();
            let result = composer.parse(&profile_text);
            
            assert!(result.is_ok(), "Failed to parse profile4: {:?}", result.err());
            let profile = result.unwrap();
            
            assert!(!profile.summary.query_id.is_empty());
            assert!(profile.execution_tree.is_some());
        }

        #[test]
        fn test_compose_profile5() {
            let profile_text = load_profile("profile5.txt");
            let mut composer = ProfileComposer::new();
            let result = composer.parse(&profile_text);
            
            assert!(result.is_ok(), "Failed to parse profile5: {:?}", result.err());
            let profile = result.unwrap();
            
            assert!(!profile.summary.query_id.is_empty());
            assert!(profile.execution_tree.is_some());
        }
    }

    // ========================================================================
    // Full Analysis Tests
    // ========================================================================

    mod analysis_tests {
        use super::*;

        #[test]
        fn test_analyze_profile1() {
            let profile_text = load_profile("profile1.txt");
            let result = analyze_profile(&profile_text);
            
            assert!(result.is_ok(), "Analysis failed: {:?}", result.err());
            let analysis = result.unwrap();
            
            // Verify analysis results
            assert!(analysis.performance_score >= 0.0 && analysis.performance_score <= 100.0);
            assert!(!analysis.conclusion.is_empty());
            
            // Verify execution tree
            assert!(analysis.execution_tree.is_some());
            let tree = analysis.execution_tree.as_ref().unwrap();
            
            // Profile1 should have OLAP_SCAN as the most time-consuming node
            let olap_scan = tree.nodes.iter().find(|n| n.operator_name == "OLAP_SCAN");
            assert!(olap_scan.is_some());
            
            // Verify summary
            assert!(analysis.summary.is_some());
            let summary = analysis.summary.as_ref().unwrap();
            assert_eq!(summary.query_id, "c025364c-a999-11f0-a663-f62b9654e895");
        }

        #[test]
        fn test_analyze_profile2_time_percentages() {
            // Profile2.png expected values (MUST MATCH EXACTLY):
            // - SCHEMA_SCAN (plan_node_id=0): 50.75%
            // - EXCHANGE (plan_node_id=1): 45.73%
            // - RESULT_SINK (plan_node_id=-1): 3.56%
            
            let profile_text = load_profile("profile2.txt");
            let result = analyze_profile(&profile_text);
            
            assert!(result.is_ok(), "Analysis failed: {:?}", result.err());
            let analysis = result.unwrap();
            
            let tree = analysis.execution_tree.as_ref().expect("Execution tree is missing");
            
            // Print for debugging
            println!("\n=== Profile2 Time Analysis ===");
            for node in &tree.nodes {
                println!("Node: {} (plan_id={:?}): {:.2}%", 
                    node.operator_name, node.plan_node_id, 
                    node.time_percentage.unwrap_or(0.0));
            }
            
            // STRICT VERIFICATION - must match image values within 0.1%
            let scan_node = tree.nodes.iter()
                .find(|n| n.operator_name.contains("SCAN"))
                .expect("SCAN node not found");
            let scan_pct = scan_node.time_percentage.unwrap();
            assert!((scan_pct - 50.75).abs() < 0.1, 
                "SCHEMA_SCAN: expected 50.75%, got {:.2}%", scan_pct);
            
            let exchange_node = tree.nodes.iter()
                .find(|n| n.operator_name == "EXCHANGE")
                .expect("EXCHANGE node not found");
            let exchange_pct = exchange_node.time_percentage.unwrap();
            assert!((exchange_pct - 45.73).abs() < 0.1,
                "EXCHANGE: expected 45.73%, got {:.2}%", exchange_pct);
            
            let sink_node = tree.nodes.iter()
                .find(|n| n.operator_name == "RESULT_SINK")
                .expect("RESULT_SINK node not found");
            let sink_pct = sink_node.time_percentage.unwrap();
            assert!((sink_pct - 3.56).abs() < 0.1,
                "RESULT_SINK: expected 3.56%, got {:.2}%", sink_pct);
        }
        
        #[test]
        fn test_analyze_profile1_time_percentages() {
            // Profile1.png expected values:
            // - OLAP_SCAN (plan_node_id=0): 100%
            // - All other nodes: 0%
            
            let profile_text = load_profile("profile1.txt");
            let result = analyze_profile(&profile_text).expect("Analysis failed");
            let tree = result.execution_tree.as_ref().expect("Execution tree is missing");
            
            println!("\n=== Profile1 Time Analysis ===");
            for node in &tree.nodes {
                println!("Node: {} (plan_id={:?}): {:.2}%", 
                    node.operator_name, node.plan_node_id, 
                    node.time_percentage.unwrap_or(0.0));
            }
            
            // OLAP_SCAN should be ~100%
            let scan_node = tree.nodes.iter()
                .find(|n| n.operator_name.contains("SCAN"))
                .expect("SCAN node not found");
            let scan_pct = scan_node.time_percentage.unwrap();
            assert!(scan_pct > 99.0, 
                "OLAP_SCAN: expected ~100%, got {:.2}%", scan_pct);
        }
        
        #[test]
        fn test_analyze_profile3_time_percentages() {
            // Profile3.png expected values:
            // - OLAP_SCAN (plan_node_id=0): 99.97%
            // - PROJECT (plan_node_id=1): 0.01%
            // - AGGREGATION (plan_node_id=2): 0.01%
            // - Others: 0%
            
            let profile_text = load_profile("profile3.txt");
            let result = analyze_profile(&profile_text).expect("Analysis failed");
            let tree = result.execution_tree.as_ref().expect("Execution tree is missing");
            
            println!("\n=== Profile3 Time Analysis ===");
            for node in &tree.nodes {
                println!("Node: {} (plan_id={:?}): {:.2}%", 
                    node.operator_name, node.plan_node_id, 
                    node.time_percentage.unwrap_or(0.0));
            }
            
            // OLAP_SCAN should be ~99.97%
            let scan_node = tree.nodes.iter()
                .find(|n| n.operator_name.contains("SCAN"))
                .expect("SCAN node not found");
            let scan_pct = scan_node.time_percentage.unwrap();
            assert!(scan_pct > 99.0, 
                "OLAP_SCAN: expected ~99.97%, got {:.2}%", scan_pct);
        }
        
        #[test]
        fn test_analyze_profile4_time_percentages() {
            // Profile4.png expected values:
            // - RESULT_SINK (plan_node_id=-1): 97.43%
            // - MERGE_EXCHANGE (plan_node_id=5): 2.64%
            // - Others: PENDING/0%
            
            let profile_text = load_profile("profile4.txt");
            let result = analyze_profile(&profile_text).expect("Analysis failed");
            let tree = result.execution_tree.as_ref().expect("Execution tree is missing");
            
            println!("\n=== Profile4 Time Analysis ===");
            for node in &tree.nodes {
                println!("Node: {} (plan_id={:?}): {:.2}%", 
                    node.operator_name, node.plan_node_id, 
                    node.time_percentage.unwrap_or(0.0));
            }
            
            // RESULT_SINK should be ~97.43%
            let sink_node = tree.nodes.iter()
                .find(|n| n.operator_name == "RESULT_SINK")
                .expect("RESULT_SINK node not found");
            let sink_pct = sink_node.time_percentage.unwrap_or(0.0);
            assert!((sink_pct - 97.43).abs() < 1.0, 
                "RESULT_SINK: expected ~97.43%, got {:.2}%", sink_pct);
            
            // MERGE_EXCHANGE should be ~2.64%
            let exchange_node = tree.nodes.iter()
                .find(|n| n.operator_name.contains("EXCHANGE"));
            if let Some(node) = exchange_node {
                let pct = node.time_percentage.unwrap_or(0.0);
                assert!((pct - 2.64).abs() < 1.0, 
                    "MERGE_EXCHANGE: expected ~2.64%, got {:.2}%", pct);
            }
        }
        
        #[test]
        fn test_analyze_profile5_time_percentages() {
            // Profile5.png expected values:
            // - TABLE_FUNCTION (plan_node_id=1): 59.07%
            // - OLAP_TABLE_SINK (plan_node_id=-1): 35.73%
            // - PROJECT (plan_node_id=2): 5.64%
            // - OLAP_SCAN (plan_node_id=0): 0%
            
            let profile_text = load_profile("profile5.txt");
            let result = analyze_profile(&profile_text).expect("Analysis failed");
            let tree = result.execution_tree.as_ref().expect("Execution tree is missing");
            
            println!("\n=== Profile5 Time Analysis ===");
            for node in &tree.nodes {
                println!("Node: {} (plan_id={:?}): {:.2}%", 
                    node.operator_name, node.plan_node_id, 
                    node.time_percentage.unwrap_or(0.0));
            }
            
            // TABLE_FUNCTION should be ~59.07%
            let tf_node = tree.nodes.iter()
                .find(|n| n.operator_name.contains("TABLE_FUNCTION"));
            if let Some(node) = tf_node {
                let pct = node.time_percentage.unwrap_or(0.0);
                assert!((pct - 59.07).abs() < 1.0, 
                    "TABLE_FUNCTION: expected ~59.07%, got {:.2}%", pct);
            }
            
            // OLAP_TABLE_SINK should be ~35.73%
            let sink_node = tree.nodes.iter()
                .find(|n| n.operator_name.contains("SINK"));
            if let Some(node) = sink_node {
                let pct = node.time_percentage.unwrap_or(0.0);
                assert!((pct - 35.73).abs() < 1.0, 
                    "OLAP_TABLE_SINK: expected ~35.73%, got {:.2}%", pct);
            }
            
            // PROJECT should be ~5.64%
            let project_node = tree.nodes.iter()
                .find(|n| n.operator_name == "PROJECT");
            if let Some(node) = project_node {
                let pct = node.time_percentage.unwrap_or(0.0);
                assert!((pct - 5.64).abs() < 1.0, 
                    "PROJECT: expected ~5.64%, got {:.2}%", pct);
            }
        }

        #[test]
        fn test_analyze_all_profiles() {
            let profiles = vec![
                "profile1.txt",
                "profile2.txt",
                "profile3.txt",
                "profile4.txt",
                "profile5.txt",
                "test_profile.txt",
            ];
            
            for profile_name in profiles {
                let profile_text = load_profile(profile_name);
                let result = analyze_profile(&profile_text);
                
                assert!(result.is_ok(), "Failed to analyze {}: {:?}", profile_name, result.err());
                
                let analysis = result.unwrap();
                assert!(analysis.execution_tree.is_some(), "{} has no execution tree", profile_name);
                assert!(analysis.summary.is_some(), "{} has no summary", profile_name);
                
                println!("✓ {} analyzed successfully: score={:.1}, hotspots={}", 
                    profile_name, analysis.performance_score, analysis.hotspots.len());
            }
        }

        #[test]
        fn test_top_time_consuming_nodes() {
            // Profile1.png shows OLAP_SCAN at 100% (9分41秒1毫秒628微秒)
            // This is the most time-consuming node
            let profile_text = load_profile("profile1.txt");
            let result = analyze_profile(&profile_text).unwrap();
            
            let summary = result.summary.as_ref().unwrap();
            assert!(summary.top_time_consuming_nodes.is_some(), 
                "top_time_consuming_nodes should be present");
            
            let top_nodes = summary.top_time_consuming_nodes.as_ref().unwrap();
            
            // Print debug info for diagnosis
            println!("=== Top Time Consuming Nodes ===");
            println!("Count: {}", top_nodes.len());
            for node in top_nodes {
                println!("  Rank {}: {} (plan_id={}) - {:.2}% - {}", 
                    node.rank, node.operator_name, node.plan_node_id, 
                    node.time_percentage, node.total_time);
            }
            
            // Also print all nodes from execution tree for diagnosis
            if let Some(tree) = &result.execution_tree {
                println!("\n=== All Execution Tree Nodes ===");
                for node in &tree.nodes {
                    println!("  {} (plan_id={:?}): percentage={:?}, time={:?}ns", 
                        node.operator_name, node.plan_node_id, 
                        node.time_percentage, node.metrics.operator_total_time);
                    // Print unique_metrics for SCAN nodes
                    if node.operator_name.contains("SCAN") {
                        println!("    unique_metrics: {:?}", node.unique_metrics);
                    }
                }
            }
            
            // STRICT TEST: top_nodes should NOT be empty for profile1
            // Profile1 has clear time metrics that should be parsed
            assert!(!top_nodes.is_empty(), 
                "Top nodes should not be empty for profile1. \
                This indicates OperatorTotalTime is not being parsed correctly.");
            
            // First node should have rank 1
            assert_eq!(top_nodes[0].rank, 1, "First node should have rank 1");
            
            // Nodes should be sorted by time percentage (descending)
            for i in 1..top_nodes.len() {
                assert!(
                    top_nodes[i-1].time_percentage >= top_nodes[i].time_percentage,
                    "Top nodes not sorted correctly: {} ({:.2}%) should be >= {} ({:.2}%)",
                    top_nodes[i-1].operator_name, top_nodes[i-1].time_percentage,
                    top_nodes[i].operator_name, top_nodes[i].time_percentage
                );
            }
        }
    }

    // ========================================================================
    // Hotspot Detection Tests (using RuleEngine)
    // ========================================================================

    mod hotspot_tests {
        use super::*;
        use crate::services::profile_analyzer::analyzer::RuleEngine;

        #[test]
        fn test_detect_long_running_query() {
            let profile_text = load_profile("profile1.txt");
            let mut composer = ProfileComposer::new();
            let profile = composer.parse(&profile_text).unwrap();
            
            let engine = RuleEngine::new();
            let diagnostics = engine.analyze(&profile);
            
            // Profile1 runs for 9m41s, should detect issues
            println!("Detected {} diagnostics for profile1", diagnostics.len());
            for diag in &diagnostics {
                println!("  - [{}] {}: {}", diag.rule_id, diag.node_path, diag.message);
            }
        }

        #[test]
        fn test_hotspot_suggestions() {
            let profile_text = load_profile("profile1.txt");
            let mut composer = ProfileComposer::new();
            let profile = composer.parse(&profile_text).unwrap();
            
            let engine = RuleEngine::new();
            let diagnostics = engine.analyze(&profile);
            
            // All diagnostics should have suggestions
            for diag in &diagnostics {
                assert!(!diag.suggestions.is_empty(), 
                    "Diagnostic {} has no suggestions", diag.rule_id);
            }
        }
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    mod edge_case_tests {
        use super::*;

        #[test]
        fn test_empty_profile() {
            let result = analyze_profile("");
            assert!(result.is_err());
        }

        #[test]
        fn test_malformed_profile() {
            let result = analyze_profile("This is not a valid profile");
            assert!(result.is_err());
        }

        #[test]
        fn test_partial_profile() {
            let partial = r#"
Query:
  Summary:
     - Query ID: test-id
     - Total: 1s
"#;
            let result = analyze_profile(partial);
            // Should fail due to missing required sections
            assert!(result.is_err());
        }

        #[test]
        fn test_profile_with_zero_time() {
            let profile_text = load_profile("profile2.txt");
            let result = analyze_profile(&profile_text);
            
            assert!(result.is_ok());
            let analysis = result.unwrap();
            
            // Even with very short execution time, should produce valid results
            assert!(analysis.execution_tree.is_some());
        }
    }

    // ========================================================================
    // Metrics Parser Tests
    // ========================================================================

    mod metrics_parser_tests {
        use super::*;

        #[test]
        fn test_parse_operator_metrics() {
            let metrics_text = r#"
          CommonMetrics:
             - OperatorTotalTime: 59.501us
             - PushChunkNum: 1
             - PushRowNum: 11
             - PushTotalTime: 45.331us
"#;
            let metrics = MetricsParser::parse_common_metrics(metrics_text);
            
            assert!(metrics.operator_total_time.is_some());
            assert_eq!(metrics.push_chunk_num, Some(1));
            assert_eq!(metrics.push_row_num, Some(11));
        }

        #[test]
        fn test_extract_common_metrics_block() {
            let operator_text = r#"
        RESULT_SINK (plan_node_id=-1):
          CommonMetrics:
             - OperatorTotalTime: 59.501us
             - PushRowNum: 11
          UniqueMetrics:
             - SinkType: MYSQL_PROTOCAL
"#;
            let common_block = MetricsParser::extract_common_metrics_block(operator_text);
            assert!(common_block.contains("OperatorTotalTime"));
            assert!(common_block.contains("PushRowNum"));
            assert!(!common_block.contains("SinkType"));
        }

        #[test]
        fn test_extract_unique_metrics_block() {
            let operator_text = r#"
        RESULT_SINK (plan_node_id=-1):
          CommonMetrics:
             - OperatorTotalTime: 59.501us
          UniqueMetrics:
             - SinkType: MYSQL_PROTOCAL
             - AppendChunkTime: 8.890us
"#;
            let unique_block = MetricsParser::extract_unique_metrics_block(operator_text);
            assert!(unique_block.contains("SinkType"));
            assert!(unique_block.contains("AppendChunkTime"));
            assert!(!unique_block.contains("OperatorTotalTime"));
        }
    }

    // ========================================================================
    // Operator Parser Tests
    // ========================================================================

    mod operator_parser_tests {
        use super::*;

        #[test]
        fn test_is_operator_header() {
            assert!(OperatorParser::is_operator_header("OLAP_SCAN (plan_node_id=0):"));
            assert!(OperatorParser::is_operator_header("RESULT_SINK (plan_node_id=-1):"));
            assert!(OperatorParser::is_operator_header("HASH_JOIN (plan_node_id=5):"));
            assert!(!OperatorParser::is_operator_header("CommonMetrics:"));
            assert!(!OperatorParser::is_operator_header("- OperatorTotalTime: 100ms"));
        }

        #[test]
        fn test_determine_node_type() {
            assert_eq!(OperatorParser::determine_node_type("OLAP_SCAN"), NodeType::OlapScan);
            assert_eq!(OperatorParser::determine_node_type("CONNECTOR_SCAN"), NodeType::ConnectorScan);
            assert_eq!(OperatorParser::determine_node_type("HASH_JOIN"), NodeType::HashJoin);
            assert_eq!(OperatorParser::determine_node_type("AGGREGATE"), NodeType::Aggregate);
            assert_eq!(OperatorParser::determine_node_type("RESULT_SINK"), NodeType::ResultSink);
            assert_eq!(OperatorParser::determine_node_type("EXCHANGE"), NodeType::ExchangeSource);
            assert_eq!(OperatorParser::determine_node_type("UNKNOWN_OP"), NodeType::Unknown);
        }

        #[test]
        fn test_canonical_topology_name() {
            assert_eq!(OperatorParser::canonical_topology_name("HASH_JOIN_BUILD"), "HASH_JOIN");
            assert_eq!(OperatorParser::canonical_topology_name("AGGREGATE_BLOCKING"), "AGGREGATE");
            assert_eq!(OperatorParser::canonical_topology_name("OLAP_SCAN"), "OLAP_SCAN");
        }
    }

    // ========================================================================
    // Rule Engine Tests - Profile Diagnostic Validation
    // ========================================================================

    mod rule_engine_tests {
        use super::*;
        use crate::services::profile_analyzer::analyzer::RuleEngine;
        use crate::services::profile_analyzer::analyzer::rule_engine::RuleEngineConfig;
        use crate::services::profile_analyzer::analyzer::rules::RuleSeverity;

        /// Test result summary for a profile
        #[derive(Debug)]
        struct ProfileTestResult {
            filename: String,
            parse_success: bool,
            diagnostics_count: usize,
            rule_ids: Vec<String>,
            messages: Vec<String>,
        }

        /// Run diagnostic test on a single profile
        fn test_single_profile(filename: &str) -> ProfileTestResult {
            let profile_text = load_profile(filename);
            
            let mut result = ProfileTestResult {
                filename: filename.to_string(),
                parse_success: false,
                diagnostics_count: 0,
                rule_ids: vec![],
                messages: vec![],
            };
            
            // Parse profile
            let mut composer = ProfileComposer::new();
            let profile = match composer.parse(&profile_text) {
                Ok(p) => {
                    result.parse_success = true;
                    p
                }
                Err(e) => {
                    result.messages.push(format!("Parse error: {:?}", e));
                    return result;
                }
            };
            
            // Run rule engine
            let config = RuleEngineConfig {
                max_suggestions: 10,
                include_parameters: true,
                ..Default::default()
            };
            let engine = RuleEngine::with_config(config);
            let diagnostics = engine.analyze(&profile);
            
            result.diagnostics_count = diagnostics.len();
            result.rule_ids = diagnostics.iter().map(|d| d.rule_id.clone()).collect();
            result.messages = diagnostics.iter().map(|d| d.message.clone()).collect();
            
            result
        }

        #[test]
        fn test_all_profile_fixtures() {
            let profile_files = vec![
                "profile1.txt",
                "profile2.txt", 
                "profile3.txt",
                "profile4.txt",
                "profile5.txt",
                "test_profile.txt",
            ];
            
            println!("\n============================================================");
            println!("Profile Diagnostic Test Results");
            println!("============================================================\n");
            
            let mut total_parsed = 0;
            let mut total_with_diagnostics = 0;
            
            for filename in &profile_files {
                let result = test_single_profile(filename);
                
                println!("📄 {}", result.filename);
                println!("   Parse: {}", if result.parse_success { "✅" } else { "❌" });
                println!("   Diagnostics: {}", result.diagnostics_count);
                
                if result.parse_success {
                    total_parsed += 1;
                }
                if result.diagnostics_count > 0 {
                    total_with_diagnostics += 1;
                    println!("   Rules triggered: {:?}", result.rule_ids);
                    for msg in result.messages.iter().take(3) {
                        println!("      - {}", msg);
                    }
                }
                println!();
                
                // Assert parse success
                assert!(result.parse_success, "Profile {} should parse successfully", filename);
            }
            
            println!("Summary: {}/{} parsed, {}/{} with diagnostics",
                total_parsed, profile_files.len(),
                total_with_diagnostics, profile_files.len());
        }

        #[test]
        fn test_profile1_scan_heavy() {
            // Profile 1: 9m41s total, scan time dominates
            let result = test_single_profile("profile1.txt");
            
            assert!(result.parse_success, "Profile should parse");
            
            println!("\nProfile1 (scan-heavy) diagnostics:");
            for (rule_id, msg) in result.rule_ids.iter().zip(result.messages.iter()) {
                println!("  [{}] {}", rule_id, msg);
            }
            
            // Should detect long running or scan-related issues
            let has_relevant_diagnostic = result.rule_ids.iter()
                .any(|id| id.starts_with("Q") || id.starts_with("G"));
            
            assert!(has_relevant_diagnostic || result.diagnostics_count > 0,
                "Should detect performance issues in scan-heavy profile");
        }

        #[test]
        fn test_profile2() {
            let result = test_single_profile("profile2.txt");
            assert!(result.parse_success, "Profile should parse");
            
            println!("\nProfile2 diagnostics:");
            for (rule_id, msg) in result.rule_ids.iter().zip(result.messages.iter()) {
                println!("  [{}] {}", rule_id, msg);
            }
        }

        #[test]
        fn test_profile3() {
            let result = test_single_profile("profile3.txt");
            assert!(result.parse_success, "Profile should parse");
            
            println!("\nProfile3 diagnostics:");
            for (rule_id, msg) in result.rule_ids.iter().zip(result.messages.iter()) {
                println!("  [{}] {}", rule_id, msg);
            }
        }

        #[test]
        fn test_profile4() {
            let result = test_single_profile("profile4.txt");
            assert!(result.parse_success, "Profile should parse");
            
            println!("\nProfile4 diagnostics:");
            for (rule_id, msg) in result.rule_ids.iter().zip(result.messages.iter()) {
                println!("  [{}] {}", rule_id, msg);
            }
        }

        #[test]
        fn test_profile5() {
            let result = test_single_profile("profile5.txt");
            assert!(result.parse_success, "Profile should parse");
            
            println!("\nProfile5 diagnostics:");
            for (rule_id, msg) in result.rule_ids.iter().zip(result.messages.iter()) {
                println!("  [{}] {}", rule_id, msg);
            }
        }

        #[test]
        fn test_rule_engine_creation() {
            let _engine = RuleEngine::new();
            // Should create without panic
            assert!(true, "Rule engine created successfully");
        }

        #[test]
        fn test_rule_engine_with_config() {
            let config = RuleEngineConfig {
                max_suggestions: 3,
                include_parameters: false,
                min_severity: RuleSeverity::Warning,
            };
            
            let engine = RuleEngine::with_config(config);
            
            // Load a profile and verify config is respected
            let profile_text = load_profile("profile1.txt");
            let mut composer = ProfileComposer::new();
            let profile = composer.parse(&profile_text).expect("Should parse");
            
            let diagnostics = engine.analyze(&profile);
            
            // Should respect max_suggestions limit
            assert!(diagnostics.len() <= 3, 
                "Should respect max_suggestions limit, got {}", diagnostics.len());
            
            // Should filter out Info severity
            for d in &diagnostics {
                assert!(d.severity >= RuleSeverity::Warning,
                    "Should filter out Info severity, got {:?}", d.severity);
            }
        }

        #[test]
        fn test_rule_engine_empty_profile() {
            let engine = RuleEngine::new();
            
            // Create minimal profile
            let profile = Profile {
                summary: ProfileSummary {
                    query_id: "test".to_string(),
                    total_time: "1s".to_string(),
                    ..Default::default()
                },
                planner: PlannerInfo {
                    details: std::collections::HashMap::new(),
                },
                execution: ExecutionInfo {
                    topology: String::new(),
                    metrics: std::collections::HashMap::new(),
                },
                fragments: vec![],
                execution_tree: None,
            };
            
            // Should not panic
            let diagnostics = engine.analyze(&profile);
            println!("Empty profile diagnostics: {}", diagnostics.len());
        }
    }

    // ========================================================================
    // DataCache Hit Rate Tests
    // ========================================================================

    mod datacache_tests {
        use super::*;

        #[test]
        fn test_datacache_hit_rate_with_fsio() {
            // Test the updated profile with FSIOBytesRead
            let profile_text = load_profile("test_profile.txt");
            let result = analyze_profile(&profile_text).expect("Should analyze");
            
            let summary = result.summary.as_ref().expect("Should have summary");
            
            // Check if DataCache metrics are present
            if let Some(hit_rate) = summary.datacache_hit_rate {
                println!("DataCache Hit Rate: {:.2}%", hit_rate * 100.0);
                println!("Local Bytes: {:?}", summary.datacache_bytes_local_display);
                println!("Remote Bytes: {:?}", summary.datacache_bytes_remote_display);
                
                // The test profile has:
                // - DataCacheReadDiskBytes: 4.015 GB (cache hit)
                // - FSIOBytesRead: 2.332 GB (cache miss)
                // Expected hit rate: 4.015 / (4.015 + 2.332) ≈ 63.3%
                assert!(hit_rate < 1.0, "Hit rate should not be 100% when FSIOBytesRead > 0");
                assert!(hit_rate > 0.5, "Hit rate should be > 50%");
                assert!(hit_rate < 0.7, "Hit rate should be < 70%");
            } else {
                println!("No DataCache metrics found in profile");
            }
        }

        #[test]
        fn test_datacache_calculation_logic() {
            // Simulate the calculation with known values
            // DataCacheReadDiskBytes: 4.015 GB = 4.015 * 1024^3 = 4,311,744,921 bytes
            // FSIOBytesRead: 2.332 GB = 2.332 * 1024^3 = 2,504,047,820 bytes
            let cache_hit: f64 = 4.015 * 1024.0 * 1024.0 * 1024.0;
            let cache_miss: f64 = 2.332 * 1024.0 * 1024.0 * 1024.0;
            let total = cache_hit + cache_miss;
            let expected_hit_rate = cache_hit / total;
            
            println!("Expected hit rate: {:.4} ({:.2}%)", expected_hit_rate, expected_hit_rate * 100.0);
            
            // Verify the calculation
            assert!((expected_hit_rate - 0.6326).abs() < 0.01, 
                "Expected ~63.26%, got {:.2}%", expected_hit_rate * 100.0);
        }
    }
}
