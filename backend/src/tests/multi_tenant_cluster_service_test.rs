// Multi-tenant cluster service tests

use crate::models::CreateClusterRequest;
use crate::services::{cluster_service::ClusterService, mysql_pool_manager::MySQLPoolManager};
use crate::tests::common::{create_test_db, setup_multi_tenant_test_data};
use std::sync::Arc;

#[tokio::test]
async fn test_cluster_organization_filtering() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create cluster in org1
    let org1_cluster_req = CreateClusterRequest {
        name: "org1_cluster".to_string(),
        description: Some("Cluster for organization 1".to_string()),
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
        .expect("Org1 admin should create cluster");

    assert_eq!(org1_cluster.name, "org1_cluster");
    assert_eq!(org1_cluster.organization_id, Some(test_data.org1_id));
    assert!(org1_cluster.is_active); // First cluster should be active

    // Create cluster in org2
    let org2_cluster_req = CreateClusterRequest {
        name: "org2_cluster".to_string(),
        description: Some("Cluster for organization 2".to_string()),
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
        .expect("Org2 admin should create cluster");

    assert_eq!(org2_cluster.name, "org2_cluster");
    assert_eq!(org2_cluster.organization_id, Some(test_data.org2_id));
    assert!(org2_cluster.is_active); // First cluster should be active

    // Test: Both clusters exist but in different organizations
    let all_clusters = cluster_service
        .list_clusters()
        .await
        .expect("Should list all clusters");

    let org1_clusters: Vec<_> = all_clusters
        .iter()
        .filter(|c| c.organization_id == Some(test_data.org1_id))
        .collect();
    let org2_clusters: Vec<_> = all_clusters
        .iter()
        .filter(|c| c.organization_id == Some(test_data.org2_id))
        .collect();

    assert_eq!(org1_clusters.len(), 1);
    assert_eq!(org2_clusters.len(), 1);
    assert_eq!(org1_clusters[0].name, "org1_cluster");
    assert_eq!(org2_clusters[0].name, "org2_cluster");
}

#[tokio::test]
async fn test_cluster_creation_organization_scoping() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Test: Super admin can create cluster with specific organization
    let super_admin_cluster_req = CreateClusterRequest {
        name: "super_admin_cluster".to_string(),
        description: Some("Cluster created by super admin".to_string()),
        fe_host: "super.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: Some(test_data.org1_id),
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let super_cluster = cluster_service
        .create_cluster(super_admin_cluster_req, test_data.super_admin_user_id, None, true)
        .await
        .expect("Super admin should create cluster with specific org");

    assert_eq!(super_cluster.organization_id, Some(test_data.org1_id));

    // Test: Org admin creates cluster in their organization
    let org_cluster_req = CreateClusterRequest {
        name: "org_admin_cluster".to_string(),
        description: Some("Cluster created by org admin".to_string()),
        fe_host: "orgadmin.example.com".to_string(),
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

    let org_cluster = cluster_service
        .create_cluster(
            org_cluster_req,
            test_data.org1_admin_user_id,
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Org admin should create cluster in their org");

    assert_eq!(org_cluster.organization_id, Some(test_data.org1_id));

    // Test: Org admin cannot create cluster without organization context
    let no_org_cluster_req = CreateClusterRequest {
        name: "no_org_cluster".to_string(),
        description: Some("Cluster without org".to_string()),
        fe_host: "noorg.example.com".to_string(),
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

    let result = cluster_service
        .create_cluster(no_org_cluster_req, test_data.org1_admin_user_id, None, false)
        .await;

    assert!(result.is_err(), "Org admin should not create cluster without organization context");
}

#[tokio::test]
async fn test_active_cluster_per_organization() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create first cluster in org1 (should be active)
    let cluster1_req = CreateClusterRequest {
        name: "org1_cluster1".to_string(),
        description: Some("First cluster".to_string()),
        fe_host: "cluster1.example.com".to_string(),
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

    let cluster1 = cluster_service
        .create_cluster(cluster1_req, test_data.org1_admin_user_id, Some(test_data.org1_id), false)
        .await
        .expect("Should create first cluster");

    assert!(cluster1.is_active);

    // Create second cluster in org1 (should NOT be active)
    let cluster2_req = CreateClusterRequest {
        name: "org1_cluster2".to_string(),
        description: Some("Second cluster".to_string()),
        fe_host: "cluster2.example.com".to_string(),
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

    let cluster2 = cluster_service
        .create_cluster(cluster2_req, test_data.org1_admin_user_id, Some(test_data.org1_id), false)
        .await
        .expect("Should create second cluster");

    assert!(!cluster2.is_active); // Should not be active

    // Verify only one active cluster in org1
    let org1_clusters: Vec<_> = sqlx::query_as::<_, (i64, bool)>(
        "SELECT id, is_active FROM clusters WHERE organization_id = ?",
    )
    .bind(test_data.org1_id)
    .fetch_all(&pool)
    .await
    .expect("Should fetch org1 clusters");

    let active_count = org1_clusters
        .iter()
        .filter(|(_, is_active)| *is_active)
        .count();
    assert_eq!(active_count, 1, "Should have exactly one active cluster");

    // Activate second cluster
    cluster_service
        .set_active_cluster(cluster2.id)
        .await
        .expect("Should activate second cluster");

    // Verify first cluster is now inactive
    let cluster1_updated = cluster_service
        .get_cluster(cluster1.id)
        .await
        .expect("Should get cluster1");
    assert!(!cluster1_updated.is_active);

    // Verify second cluster is now active
    let cluster2_updated = cluster_service
        .get_cluster(cluster2.id)
        .await
        .expect("Should get cluster2");
    assert!(cluster2_updated.is_active);

    // Verify still only one active cluster
    let org1_clusters_after: Vec<_> = sqlx::query_as::<_, (i64, bool)>(
        "SELECT id, is_active FROM clusters WHERE organization_id = ?",
    )
    .bind(test_data.org1_id)
    .fetch_all(&pool)
    .await
    .expect("Should fetch org1 clusters");

    let active_count_after = org1_clusters_after
        .iter()
        .filter(|(_, is_active)| *is_active)
        .count();
    assert_eq!(active_count_after, 1, "Should still have exactly one active cluster");
}

#[tokio::test]
async fn test_active_cluster_organization_isolation() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create clusters in both organizations
    let org1_cluster_req = CreateClusterRequest {
        name: "org1_active_cluster".to_string(),
        description: Some("Org1 cluster".to_string()),
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
        name: "org2_active_cluster".to_string(),
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

    // Both should be active (first cluster in their respective organizations)
    assert!(org1_cluster.is_active);
    assert!(org2_cluster.is_active);

    // Get active cluster for org1
    let org1_active = cluster_service
        .get_active_cluster_by_org(Some(test_data.org1_id))
        .await
        .expect("Should get org1 active cluster");

    assert_eq!(org1_active.id, org1_cluster.id);

    // Get active cluster for org2
    let org2_active = cluster_service
        .get_active_cluster_by_org(Some(test_data.org2_id))
        .await
        .expect("Should get org2 active cluster");

    assert_eq!(org2_active.id, org2_cluster.id);

    // Create second cluster in org1
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

    let org1_cluster2 = cluster_service
        .create_cluster(
            org1_cluster2_req,
            test_data.org1_admin_user_id,
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Should create second org1 cluster");

    // Activate second org1 cluster
    cluster_service
        .set_active_cluster(org1_cluster2.id)
        .await
        .expect("Should activate second org1 cluster");

    // Verify org2's active cluster is NOT affected
    let org2_active_after = cluster_service
        .get_cluster(org2_cluster.id)
        .await
        .expect("Should get org2 cluster");

    assert!(org2_active_after.is_active, "Org2's active cluster should remain active");

    // Verify org1's first cluster is now inactive
    let org1_cluster1_after = cluster_service
        .get_cluster(org1_cluster.id)
        .await
        .expect("Should get org1 first cluster");

    assert!(!org1_cluster1_after.is_active, "Org1's first cluster should be inactive");

    // Verify org1's second cluster is now active
    let org1_cluster2_after = cluster_service
        .get_cluster(org1_cluster2.id)
        .await
        .expect("Should get org1 second cluster");

    assert!(org1_cluster2_after.is_active, "Org1's second cluster should be active");
}

#[tokio::test]
async fn test_cluster_first_auto_activation() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create first cluster in organization
    let cluster_req = CreateClusterRequest {
        name: "first_cluster".to_string(),
        description: Some("First cluster in org".to_string()),
        fe_host: "first.example.com".to_string(),
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

    let first_cluster = cluster_service
        .create_cluster(cluster_req, test_data.org1_admin_user_id, Some(test_data.org1_id), false)
        .await
        .expect("Should create first cluster");

    // Verify it's automatically activated
    assert!(first_cluster.is_active, "First cluster should be automatically activated");

    // Verify we can get it as the active cluster
    let active_cluster = cluster_service
        .get_active_cluster_by_org(Some(test_data.org1_id))
        .await
        .expect("Should get active cluster");

    assert_eq!(active_cluster.id, first_cluster.id);
}

#[tokio::test]
async fn test_cluster_activation_without_organization() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Test: Org admin cannot create cluster without organization context
    let cluster_req = CreateClusterRequest {
        name: "no_org_cluster".to_string(),
        description: Some("Cluster without organization".to_string()),
        fe_host: "noorg.example.com".to_string(),
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

    // Org admin tries to create cluster without org context - should fail
    let no_org_cluster = cluster_service
        .create_cluster(cluster_req, test_data.org1_admin_user_id, None, false)
        .await;

    assert!(
        no_org_cluster.is_err(),
        "Non-super-admin without org context should not create cluster"
    );
}

#[tokio::test]
async fn test_cluster_duplicate_name_prevention() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create first cluster
    let cluster_req = CreateClusterRequest {
        name: "duplicate_cluster".to_string(),
        description: Some("First cluster".to_string()),
        fe_host: "first.example.com".to_string(),
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

    cluster_service
        .create_cluster(cluster_req, test_data.org1_admin_user_id, Some(test_data.org1_id), false)
        .await
        .expect("Should create first cluster");

    // Try to create cluster with same name in different organization
    let duplicate_req = CreateClusterRequest {
        name: "duplicate_cluster".to_string(),
        description: Some("Duplicate cluster".to_string()),
        fe_host: "second.example.com".to_string(),
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

    let result = cluster_service
        .create_cluster(duplicate_req, test_data.org2_admin_user_id, Some(test_data.org2_id), false)
        .await;

    assert!(result.is_err(), "Should not allow duplicate cluster names globally");
}

#[tokio::test]
async fn test_super_admin_cross_organization_cluster_access() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create clusters in different organizations
    let org1_cluster_req = CreateClusterRequest {
        name: "org1_cluster_super".to_string(),
        description: Some("Org1 cluster".to_string()),
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

    cluster_service
        .create_cluster(
            org1_cluster_req,
            test_data.org1_admin_user_id,
            Some(test_data.org1_id),
            false,
        )
        .await
        .expect("Should create org1 cluster");

    let org2_cluster_req = CreateClusterRequest {
        name: "org2_cluster_super".to_string(),
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

    cluster_service
        .create_cluster(
            org2_cluster_req,
            test_data.org2_admin_user_id,
            Some(test_data.org2_id),
            false,
        )
        .await
        .expect("Should create org2 cluster");

    // Super admin should see all clusters
    let all_clusters = cluster_service
        .list_clusters()
        .await
        .expect("Super admin should list all clusters");

    assert!(all_clusters.iter().any(|c| c.name == "org1_cluster_super"), "Should see org1 cluster");
    assert!(all_clusters.iter().any(|c| c.name == "org2_cluster_super"), "Should see org2 cluster");

    // Super admin can create cluster for specific organization
    let super_create_req = CreateClusterRequest {
        name: "super_created_cluster".to_string(),
        description: Some("Created by super admin for org1".to_string()),
        fe_host: "super.example.com".to_string(),
        fe_http_port: 8030,
        fe_query_port: 9030,
        username: "root".to_string(),
        password: "password".to_string(),
        enable_ssl: false,
        connection_timeout: 30,
        tags: None,
        catalog: "default_catalog".to_string(),
        organization_id: Some(test_data.org1_id),
        deployment_mode: crate::models::cluster::DeploymentMode::default(),
    };

    let super_cluster = cluster_service
        .create_cluster(super_create_req, test_data.super_admin_user_id, None, true)
        .await
        .expect("Super admin should create cluster for specific org");

    assert_eq!(super_cluster.organization_id, Some(test_data.org1_id));
}

#[tokio::test]
async fn test_cluster_activation_concurrency() {
    let pool = create_test_db().await;
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());
    let cluster_service = ClusterService::new(pool.clone(), mysql_pool_manager);

    let test_data = setup_multi_tenant_test_data(&pool).await;

    // Create multiple clusters
    let mut cluster_ids = Vec::new();
    for i in 1..=3 {
        let cluster_req = CreateClusterRequest {
            name: format!("concurrent_cluster_{}", i),
            description: Some(format!("Cluster {}", i)),
            fe_host: format!("cluster{}.example.com", i),
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

        let cluster = cluster_service
            .create_cluster(
                cluster_req,
                test_data.org1_admin_user_id,
                Some(test_data.org1_id),
                false,
            )
            .await
            .expect("Should create cluster");

        cluster_ids.push(cluster.id);
    }

    // Activate last cluster
    cluster_service
        .set_active_cluster(*cluster_ids.last().unwrap())
        .await
        .expect("Should activate last cluster");

    // Verify only one cluster is active
    let clusters_status: Vec<(i64, bool)> =
        sqlx::query_as("SELECT id, is_active FROM clusters WHERE organization_id = ? ORDER BY id")
            .bind(test_data.org1_id)
            .fetch_all(&pool)
            .await
            .expect("Should fetch clusters");

    let active_count = clusters_status
        .iter()
        .filter(|(_, is_active)| *is_active)
        .count();

    assert_eq!(active_count, 1, "Should have exactly one active cluster");

    let active_cluster_id = clusters_status
        .iter()
        .find(|(_, is_active)| *is_active)
        .map(|(id, _)| *id)
        .expect("Should have active cluster");

    assert_eq!(active_cluster_id, *cluster_ids.last().unwrap(), "Last cluster should be active");
}
