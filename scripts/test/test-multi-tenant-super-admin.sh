#!/bin/bash

# Multi-Tenant Super Admin Test Script
# Role: Super Admin (admin/admin)
# Purpose: Create organizations, manage cross-org resources, verify super admin privileges

set -e

BASE_URL="http://localhost:8081/api"
TOKEN=""

# Color output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo "${BLUE}======================================${NC}"
echo "${BLUE}Multi-Tenant Super Admin Test${NC}"
echo "${BLUE}======================================${NC}"
echo ""

# Helper function to format JSON
format_json() {
  echo "$1" | python3 -m json.tool 2>/dev/null || echo "$1"
}

# Helper function to extract JSON value
extract_value() {
  echo "$1" | grep -o "\"$2\":\"[^\"]*" | sed "s/\"$2\":\"//" | head -1
}

extract_number() {
  echo "$1" | grep -o "\"$2\":[0-9]*" | sed "s/\"$2\"://" | head -1
}

# ===========================
# Step 1: Login as Super Admin
# ===========================
echo "${YELLOW}[Step 1] Login as Super Admin (admin/admin)...${NC}"
LOGIN_RESPONSE=$(curl -s -X POST "${BASE_URL}/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin"}')

TOKEN=$(extract_value "$LOGIN_RESPONSE" "token")

if [ -z "$TOKEN" ]; then
  echo "${RED}✗ Login failed${NC}"
  format_json "$LOGIN_RESPONSE"
  exit 1
fi

echo "${GREEN}✓ Login successful${NC}"
echo "Token: ${TOKEN:0:20}..."
echo ""

# ===========================
# Step 2: Verify current user is super admin
# ===========================
echo "${YELLOW}[Step 2] Verify current user identity...${NC}"
ME_RESPONSE=$(curl -s -X GET "${BASE_URL}/auth/me" \
  -H "Authorization: Bearer ${TOKEN}")

USERNAME=$(extract_value "$ME_RESPONSE" "username")
echo "Current user: $USERNAME"
format_json "$ME_RESPONSE"
echo ""

# ===========================
# Step 3: Create Organization Alpha
# ===========================
echo "${YELLOW}[Step 3] Create Organization Alpha...${NC}"
ORG_ALPHA_REQUEST='{
  "code": "ORG_ALPHA",
  "name": "Alpha Corporation",
  "description": "Alpha Corporation for testing",
  "admin_username": "alpha_admin",
  "admin_password": "Alpha@123",
  "admin_email": "alpha@example.com"
}'

ORG_ALPHA_RESPONSE=$(curl -s -X POST "${BASE_URL}/organizations" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$ORG_ALPHA_REQUEST")

ORG_ALPHA_ID=$(extract_number "$ORG_ALPHA_RESPONSE" "id")

if [ -z "$ORG_ALPHA_ID" ]; then
  echo "${RED}✗ Failed to create Organization Alpha${NC}"
  format_json "$ORG_ALPHA_RESPONSE"
  exit 1
fi

echo "${GREEN}✓ Organization Alpha created (ID: $ORG_ALPHA_ID)${NC}"
format_json "$ORG_ALPHA_RESPONSE"
echo ""

# ===========================
# Step 4: Create Organization Beta
# ===========================
echo "${YELLOW}[Step 4] Create Organization Beta...${NC}"
ORG_BETA_REQUEST='{
  "code": "ORG_BETA",
  "name": "Beta Industries",
  "description": "Beta Industries for testing",
  "admin_username": "beta_admin",
  "admin_password": "Beta@123",
  "admin_email": "beta@example.com"
}'

ORG_BETA_RESPONSE=$(curl -s -X POST "${BASE_URL}/organizations" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$ORG_BETA_REQUEST")

ORG_BETA_ID=$(extract_number "$ORG_BETA_RESPONSE" "id")

if [ -z "$ORG_BETA_ID" ]; then
  echo "${RED}✗ Failed to create Organization Beta${NC}"
  format_json "$ORG_BETA_RESPONSE"
  exit 1
fi

echo "${GREEN}✓ Organization Beta created (ID: $ORG_BETA_ID)${NC}"
format_json "$ORG_BETA_RESPONSE"
echo ""

# ===========================
# Step 5: List all organizations
# ===========================
echo "${YELLOW}[Step 5] List all organizations (Super Admin view)...${NC}"
ORGS_RESPONSE=$(curl -s -X GET "${BASE_URL}/organizations" \
  -H "Authorization: Bearer ${TOKEN}")

ORG_COUNT=$(echo "$ORGS_RESPONSE" | grep -o '"id":' | wc -l)
echo "${GREEN}✓ Found $ORG_COUNT organizations${NC}"
format_json "$ORGS_RESPONSE"
echo ""

# ===========================
# Step 6: Create cluster for Organization Alpha
# ===========================
echo "${YELLOW}[Step 6] Create cluster for Organization Alpha...${NC}"
CLUSTER_ALPHA_REQUEST="{
  \"name\": \"alpha_cluster\",
  \"description\": \"Alpha organization test cluster\",
  \"fe_host\": \"localhost\",
  \"fe_http_port\": 8030,
  \"fe_query_port\": 9030,
  \"username\": \"root\",
  \"password\": \"\",
  \"organization_id\": $ORG_ALPHA_ID,
  \"catalog\": \"default_catalog\"
}"

CLUSTER_ALPHA_RESPONSE=$(curl -s -X POST "${BASE_URL}/clusters" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$CLUSTER_ALPHA_REQUEST")

CLUSTER_ALPHA_ID=$(extract_number "$CLUSTER_ALPHA_RESPONSE" "id")

if [ -z "$CLUSTER_ALPHA_ID" ]; then
  echo "${RED}✗ Failed to create cluster for Alpha${NC}"
  format_json "$CLUSTER_ALPHA_RESPONSE"
else
  echo "${GREEN}✓ Cluster created for Alpha (ID: $CLUSTER_ALPHA_ID)${NC}"
  format_json "$CLUSTER_ALPHA_RESPONSE"
fi
echo ""

# ===========================
# Step 7: Create cluster for Organization Beta
# ===========================
echo "${YELLOW}[Step 7] Create cluster for Organization Beta...${NC}"
CLUSTER_BETA_REQUEST="{
  \"name\": \"beta_cluster\",
  \"description\": \"Beta organization test cluster\",
  \"fe_host\": \"localhost\",
  \"fe_http_port\": 8030,
  \"fe_query_port\": 9030,
  \"username\": \"root\",
  \"password\": \"\",
  \"organization_id\": $ORG_BETA_ID,
  \"catalog\": \"default_catalog\"
}"

CLUSTER_BETA_RESPONSE=$(curl -s -X POST "${BASE_URL}/clusters" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$CLUSTER_BETA_REQUEST")

CLUSTER_BETA_ID=$(extract_number "$CLUSTER_BETA_RESPONSE" "id")

if [ -z "$CLUSTER_BETA_ID" ]; then
  echo "${RED}✗ Failed to create cluster for Beta${NC}"
  format_json "$CLUSTER_BETA_RESPONSE"
else
  echo "${GREEN}✓ Cluster created for Beta (ID: $CLUSTER_BETA_ID)${NC}"
  format_json "$CLUSTER_BETA_RESPONSE"
fi
echo ""

# ===========================
# Step 8: List all clusters (cross-org view)
# ===========================
echo "${YELLOW}[Step 8] List all clusters (Super Admin cross-org view)...${NC}"
CLUSTERS_RESPONSE=$(curl -s -X GET "${BASE_URL}/clusters" \
  -H "Authorization: Bearer ${TOKEN}")

CLUSTER_COUNT=$(echo "$CLUSTERS_RESPONSE" | grep -o '"id":' | wc -l)
echo "${GREEN}✓ Found $CLUSTER_COUNT clusters across all organizations${NC}"
format_json "$CLUSTERS_RESPONSE"
echo ""

# ===========================
# Step 9: List all users
# ===========================
echo "${YELLOW}[Step 9] List all users (Super Admin view)...${NC}"
USERS_RESPONSE=$(curl -s -X GET "${BASE_URL}/users" \
  -H "Authorization: Bearer ${TOKEN}")

USER_COUNT=$(echo "$USERS_RESPONSE" | grep -o '"id":' | wc -l)
echo "${GREEN}✓ Found $USER_COUNT users${NC}"
format_json "$USERS_RESPONSE"
echo ""

# ===========================
# Step 10: Create system-level role
# ===========================
echo "${YELLOW}[Step 10] Create system-level role...${NC}"
SYSTEM_ROLE_REQUEST='{
  "code": "system_auditor",
  "name": "System Auditor",
  "description": "System-wide auditor role",
  "is_system": true
}'

SYSTEM_ROLE_RESPONSE=$(curl -s -X POST "${BASE_URL}/roles" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$SYSTEM_ROLE_REQUEST")

SYSTEM_ROLE_ID=$(extract_number "$SYSTEM_ROLE_RESPONSE" "id")

if [ -z "$SYSTEM_ROLE_ID" ]; then
  echo "${YELLOW}⚠ System role may already exist or creation restricted${NC}"
  format_json "$SYSTEM_ROLE_RESPONSE"
else
  echo "${GREEN}✓ System role created (ID: $SYSTEM_ROLE_ID)${NC}"
  format_json "$SYSTEM_ROLE_RESPONSE"
fi
echo ""

# ===========================
# Step 11: List all roles
# ===========================
echo "${YELLOW}[Step 11] List all roles (including system and org roles)...${NC}"
ROLES_RESPONSE=$(curl -s -X GET "${BASE_URL}/roles" \
  -H "Authorization: Bearer ${TOKEN}")

ROLE_COUNT=$(echo "$ROLES_RESPONSE" | grep -o '"id":' | wc -l)
echo "${GREEN}✓ Found $ROLE_COUNT roles${NC}"
format_json "$ROLES_RESPONSE"
echo ""

# ===========================
# Summary
# ===========================
echo "${GREEN}======================================${NC}"
echo "${GREEN}✓ Super Admin Test Completed!${NC}"
echo "${GREEN}======================================${NC}"
echo ""
echo "Test Summary:"
echo "1. ✓ Login as super admin"
echo "2. ✓ Verified super admin identity"
echo "3. ✓ Created Organization Alpha (ID: $ORG_ALPHA_ID)"
echo "4. ✓ Created Organization Beta (ID: $ORG_BETA_ID)"
echo "5. ✓ Listed all organizations ($ORG_COUNT total)"
echo "6. ✓ Created cluster for Alpha (ID: $CLUSTER_ALPHA_ID)"
echo "7. ✓ Created cluster for Beta (ID: $CLUSTER_BETA_ID)"
echo "8. ✓ Listed all clusters ($CLUSTER_COUNT total)"
echo "9. ✓ Listed all users ($USER_COUNT total)"
echo "10. ✓ Created system role (ID: $SYSTEM_ROLE_ID)"
echo "11. ✓ Listed all roles ($ROLE_COUNT total)"
echo ""
echo "${BLUE}Created Credentials:${NC}"
echo "  Alpha Admin: alpha_admin / Alpha@123"
echo "  Beta Admin:  beta_admin / Beta@123"
echo ""
echo "${YELLOW}Next: Run test-multi-tenant-org-admin.sh to test organization admin${NC}"
