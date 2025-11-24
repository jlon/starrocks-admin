// Handler-level organization isolation tests
// Tests to verify that all handlers properly enforce organization-level data isolation

use crate::models::CreateClusterRequest;
use crate::services::{cluster_service::ClusterService, mysql_pool_manager::MySQLPoolManager};
use crate::tests::common::{create_test_db, setup_multi_tenant_test_data};
use std::sync::Arc;

/// Test: get_active_cluster_by_org returns correct cluster for each organization
#[tokio::test]
async fn test_get_active_cluster_by_org_isolation() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);
    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create cluster for org1
    let org1_cluster_req = CreateClusterRequest {
        name: "org1_test_cluster".to_string(),
        description: Some("Test cluster for org1".to_string()),
        fe_host: "org1-test.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let org1_cluster = cluster_service
        .create_cluster(
            org1_cluster_req,
            test_data.org1_admin_user_id,
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Should create org1 cluster");

    // Create cluster for org2
    let org2_cluster_req = CreateClusterRequest {
        name: "org2_test_cluster".to_string(),
        description: Some("Test cluster for org2".to_string()),
        fe_host: "org2-test.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let org2_cluster = cluster_service
        .create_cluster(
            org2_cluster_req,
            test_data.org2_admin_user_id,
            Some(test_data.org2_id),
            false,
        )
        .await
        .expect("Should create org2 cluster");

    // Test: Org1 user gets org1's active cluster
    let org1_active = cluster_service
        .get_active_cluster_by_org(Some(test_data.org1_id))
        .await
        .expect("Should get org1 active cluster");

    assert_eq!(org1_active.id, org1_cluster.id, "Org1 should get its own active cluster");
    assert_eq!(
        org1_active.organization_id,
        Some(test_data.org1_id),
        "Returned cluster should belong to org1"
    );
    assert_eq!(org1_active.name, "org1_test_cluster", "Should return org1's cluster");

    // Test: Org2 user gets org2's active cluster
    let org2_active = cluster_service
        .get_active_cluster_by_org(Some(test_data.org2_id))
        .await
        .expect("Should get org2 active cluster");

    assert_eq!(org2_active.id, org2_cluster.id, "Org2 should get its own active cluster");
    assert_eq!(
        org2_active.organization_id,
        Some(test_data.org2_id),
        "Returned cluster should belong to org2"
    );
    assert_eq!(org2_active.name, "org2_test_cluster", "Should return org2's cluster");

    // Test: Clusters are isolated - org1 cannot see org2's cluster
    assert_ne!(
        org1_active.id, org2_active.id,
        "Different organizations should have different active clusters"
    );
}

/// Test: get_active_cluster_by_org fails when organization has no active cluster
#[tokio::test]
async fn test_get_active_cluster_by_org_no_cluster() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);
    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Test: Getting active cluster for org with no clusters should fail
    let result = cluster_service
        .get_active_cluster_by_org(Some(test_data.org1_id))
        .await;

    assert!(result.is_err(), "Should fail when organization has no active cluster");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("No active cluster found"),
        "Error message should indicate no active cluster: {}",
        error_msg
    );
}

/// Test: get_active_cluster_by_org with None organization_id should fail
#[tokio::test]
async fn test_get_active_cluster_by_org_none_org_id() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);

    // Test: Calling with None org_id should fail
    let result = cluster_service.get_active_cluster_by_org(None).await;

    assert!(result.is_err(), "Should fail when organization_id is None");
}

/// Test: Super admin can use get_active_cluster (global) while regular users must use get_active_cluster_by_org
#[tokio::test]
async fn test_super_admin_vs_regular_user_cluster_access() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);
    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create cluster for org1
    let org1_cluster_req = CreateClusterRequest {
        name: "org1_cluster_for_admin_test".to_string(),
        description: Some("Test cluster".to_string()),
        fe_host: "test.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let org1_cluster = cluster_service
        .create_cluster(
            org1_cluster_req,
            test_data.org1_admin_user_id,
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Should create cluster");

    // Test: Super admin can use global get_active_cluster
    let global_active = cluster_service
        .get_active_cluster()
        .await
        .expect("Super admin should get active cluster globally");

    assert_eq!(global_active.id, org1_cluster.id, "Global query should return the active cluster");

    // Test: Regular user should use get_active_cluster_by_org
    let org_scoped_active = cluster_service
        .get_active_cluster_by_org(Some(test_data.org1_id))
        .await
        .expect("Regular user should get active cluster by org");

    assert_eq!(
        org_scoped_active.id, org1_cluster.id,
        "Org-scoped query should return the same cluster for org1"
    );
}

/// Test: Multiple active clusters across organizations don't interfere
#[tokio::test]
async fn test_multiple_orgs_multiple_active_clusters() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);
    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create multiple clusters for org1
    let org1_cluster1_req = CreateClusterRequest {
        name: "org1_cluster1".to_string(),
        description: Some("First org1 cluster".to_string()),
        fe_host: "org1-1.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let org1_cluster1 = cluster_service
        .create_cluster(
            org1_cluster1_req,
            test_data.org1_admin_user_id,
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Should create org1 cluster1");

    let org1_cluster2_req = CreateClusterRequest {
        name: "org1_cluster2".to_string(),
        description: Some("Second org1 cluster".to_string()),
        fe_host: "org1-2.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let _org1_cluster2 = cluster_service
        .create_cluster(
            org1_cluster2_req,
            test_data.org1_admin_user_id,
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Should create org1 cluster2");

    // Create cluster for org2
    let org2_cluster_req = CreateClusterRequest {
        name: "org2_cluster".to_string(),
        description: Some("Org2 cluster".to_string()),
        fe_host: "org2.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let org2_cluster = cluster_service
        .create_cluster(
            org2_cluster_req,
            test_data.org2_admin_user_id,
            Some(test_data.org2_id),
            false,
        )
        .await
        .expect("Should create org2 cluster");

    // Test: Org1 has first cluster active (only one active per org)
    let org1_active = cluster_service
        .get_active_cluster_by_org(Some(test_data.org1_id))
        .await
        .expect("Should get org1 active cluster");

    assert_eq!(org1_active.id, org1_cluster1.id, "Org1 should have first cluster active");

    // Test: Org2 has its cluster active
    let org2_active = cluster_service
        .get_active_cluster_by_org(Some(test_data.org2_id))
        .await
        .expect("Should get org2 active cluster");

    assert_eq!(org2_active.id, org2_cluster.id, "Org2 should have its cluster active");

    // Test: Both organizations can have active clusters simultaneously
    assert!(org1_active.is_active, "Org1 cluster should be active");
    assert!(org2_active.is_active, "Org2 cluster should be active");
    assert_ne!(
        org1_active.id, org2_active.id,
        "Different orgs should have different active clusters"
    );
}

/// Test: Switching active cluster in one org doesn't affect other orgs
#[tokio::test]
async fn test_switching_active_cluster_isolation() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);
    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create 2 clusters for org1
    let org1_cluster1_req = CreateClusterRequest {
        name: "org1_cluster1_switch_test".to_string(),
        description: Some("First cluster".to_string()),
        fe_host: "org1-1.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let org1_cluster1 = cluster_service
        .create_cluster(
            org1_cluster1_req,
            test_data.org1_admin_user_id,
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Should create org1 cluster1");

    let org1_cluster2_req = CreateClusterRequest {
        name: "org1_cluster2_switch_test".to_string(),
        description: Some("Second cluster".to_string()),
        fe_host: "org1-2.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let org1_cluster2 = cluster_service
        .create_cluster(
            org1_cluster2_req,
            test_data.org1_admin_user_id,
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Should create org1 cluster2");

    // Create cluster for org2
    let org2_cluster_req = CreateClusterRequest {
        name: "org2_cluster_switch_test".to_string(),
        description: Some("Org2 cluster".to_string()),
        fe_host: "org2.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let org2_cluster = cluster_service
        .create_cluster(
            org2_cluster_req,
            test_data.org2_admin_user_id,
            Some(test_data.org2_id),
            false,
        )
        .await
        .expect("Should create org2 cluster");

    // Verify initial state: org1_cluster1 is active for org1
    let org1_active_before = cluster_service
        .get_active_cluster_by_org(Some(test_data.org1_id))
        .await
        .expect("Should get org1 active");
    assert_eq!(org1_active_before.id, org1_cluster1.id);

    // Verify initial state: org2_cluster is active for org2
    let org2_active_before = cluster_service
        .get_active_cluster_by_org(Some(test_data.org2_id))
        .await
        .expect("Should get org2 active");
    assert_eq!(org2_active_before.id, org2_cluster.id);

    // Switch org1's active cluster to cluster2
    cluster_service
        .set_active_cluster(org1_cluster2.id)
        .await
        .expect("Should switch org1 active cluster");

    // Verify org1's active cluster changed
    let org1_active_after = cluster_service
        .get_active_cluster_by_org(Some(test_data.org1_id))
        .await
        .expect("Should get org1 active after switch");
    assert_eq!(
        org1_active_after.id, org1_cluster2.id,
        "Org1 active cluster should be cluster2 now"
    );

    // Verify org2's active cluster is NOT affected
    let org2_active_after = cluster_service
        .get_active_cluster_by_org(Some(test_data.org2_id))
        .await
        .expect("Should get org2 active after org1 switch");
    assert_eq!(
        org2_active_after.id, org2_cluster.id,
        "Org2 active cluster should remain unchanged"
    );
    assert!(org2_active_after.is_active, "Org2 cluster should still be active");
}

/// Test: Data isolation - verify SQL queries include organization_id filter
#[tokio::test]
async fn test_sql_query_organization_filter() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);
    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create clusters for both orgs
    let org1_cluster_req = CreateClusterRequest {
        name: "org1_sql_test".to_string(),
        description: Some("SQL test cluster".to_string()),
        fe_host: "org1.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let org1_cluster = cluster_service
        .create_cluster(
            org1_cluster_req,
            test_data.org1_admin_user_id,
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Should create org1 cluster");

    let org2_cluster_req = CreateClusterRequest {
        name: "org2_sql_test".to_string(),
        description: Some("SQL test cluster".to_string()),
        fe_host: "org2.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: None,
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let org2_cluster = cluster_service
        .create_cluster(
            org2_cluster_req,
            test_data.org2_admin_user_id,
            Some(test_data.org2_id),
            false,
        )
        .await
        .expect("Should create org2 cluster");

    // Test: Query with org1 filter returns only org1 cluster
    let org1_result = cluster_service
        .get_active_cluster_by_org(Some(test_data.org1_id))
        .await
        .expect("Should get org1 cluster");

    assert_eq!(org1_result.id, org1_cluster.id);
    assert_eq!(org1_result.organization_id, Some(test_data.org1_id));
    assert_ne!(org1_result.id, org2_cluster.id, "Should not return org2 cluster");

    // Test: Query with org2 filter returns only org2 cluster
    let org2_result = cluster_service
        .get_active_cluster_by_org(Some(test_data.org2_id))
        .await
        .expect("Should get org2 cluster");

    assert_eq!(org2_result.id, org2_cluster.id);
    assert_eq!(org2_result.organization_id, Some(test_data.org2_id));
    assert_ne!(org2_result.id, org1_cluster.id, "Should not return org1 cluster");
}
