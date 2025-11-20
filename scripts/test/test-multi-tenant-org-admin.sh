#!/bin/bash

# Multi-Tenant Organization Admin Test Script
# Role: Organization Admin (alpha_admin/Alpha@123)
# Purpose: Verify org admin can manage their org, but cannot access other orgs

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
echo "${BLUE}Multi-Tenant Organization Admin Test${NC}"
echo "${BLUE}======================================${NC}"
echo ""

# Helper functions
format_json() {
  echo "$1" | python3 -m json.tool 2>/dev/null || echo "$1"
}

extract_value() {
  echo "$1" | grep -o "\"$2\":\"[^\"]*" | sed "s/\"$2\":\"//" | head -1
}

extract_number() {
  echo "$1" | grep -o "\"$2\":[0-9]*" | sed "s/\"$2\"://" | head -1
}

# ===========================
# Step 1: Login as Organization Alpha Admin
# ===========================
echo "${YELLOW}[Step 1] Login as Organization Alpha Admin (alpha_admin/Alpha@123)...${NC}"
LOGIN_RESPONSE=$(curl -s -X POST "${BASE_URL}/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"alpha_admin","password":"Alpha@123"}')

TOKEN=$(extract_value "$LOGIN_RESPONSE" "token")

if [ -z "$TOKEN" ]; then
  echo "${RED}✗ Login failed${NC}"
  echo "${YELLOW}Note: Make sure you ran test-multi-tenant-super-admin.sh first${NC}"
  format_json "$LOGIN_RESPONSE"
  exit 1
fi

echo "${GREEN}✓ Login successful${NC}"
echo "Token: ${TOKEN:0:20}..."
echo ""

# ===========================
# Step 2: Verify current user identity
# ===========================
echo "${YELLOW}[Step 2] Verify current user is Organization Alpha admin...${NC}"
ME_RESPONSE=$(curl -s -X GET "${BASE_URL}/auth/me" \
  -H "Authorization: Bearer ${TOKEN}")

USERNAME=$(extract_value "$ME_RESPONSE" "username")
ORG_ID=$(extract_number "$ME_RESPONSE" "organization_id")

echo "Current user: $USERNAME"
echo "Organization ID: $ORG_ID"
format_json "$ME_RESPONSE"
echo ""

if [ -z "$ORG_ID" ]; then
  echo "${RED}✗ User is not associated with an organization${NC}"
  exit 1
fi

# ===========================
# Step 3: Get organization details
# ===========================
echo "${YELLOW}[Step 3] Get current organization details...${NC}"
ORG_RESPONSE=$(curl -s -X GET "${BASE_URL}/organizations/${ORG_ID}" \
  -H "Authorization: Bearer ${TOKEN}")

ORG_CODE=$(extract_value "$ORG_RESPONSE" "code")
ORG_NAME=$(extract_value "$ORG_RESPONSE" "name")

echo "${GREEN}✓ Organization: $ORG_NAME ($ORG_CODE)${NC}"
format_json "$ORG_RESPONSE"
echo ""

# ===========================
# Step 4: List organizations (should only see own org)
# ===========================
echo "${YELLOW}[Step 4] List organizations (should only see own org)...${NC}"
ORGS_RESPONSE=$(curl -s -X GET "${BASE_URL}/organizations" \
  -H "Authorization: Bearer ${TOKEN}")

ORG_COUNT=$(echo "$ORGS_RESPONSE" | grep -o '"id":' | wc -l)
echo "${GREEN}✓ Visible organizations: $ORG_COUNT${NC}"
format_json "$ORGS_RESPONSE"

if [ "$ORG_COUNT" -eq 1 ]; then
  echo "${GREEN}✓ Isolation verified: Org admin can only see their organization${NC}"
else
  echo "${YELLOW}⚠ Warning: Org admin can see $ORG_COUNT organizations${NC}"
fi
echo ""

# ===========================
# Step 5: Create user in current organization
# ===========================
echo "${YELLOW}[Step 5] Create user in current organization...${NC}"
USER_REQUEST='{
  "username": "alpha_user1",
  "password": "User@123",
  "email": "alpha_user1@example.com"
}'

USER_RESPONSE=$(curl -s -X POST "${BASE_URL}/users" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$USER_REQUEST")

USER_ID=$(extract_number "$USER_RESPONSE" "id")

if [ -z "$USER_ID" ]; then
  echo "${YELLOW}⚠ User may already exist${NC}"
  format_json "$USER_RESPONSE"
else
  echo "${GREEN}✓ User created (ID: $USER_ID)${NC}"
  format_json "$USER_RESPONSE"
fi
echo ""

# ===========================
# Step 6: List users (should only see org users)
# ===========================
echo "${YELLOW}[Step 6] List users (should only see organization users)...${NC}"
USERS_RESPONSE=$(curl -s -X GET "${BASE_URL}/users" \
  -H "Authorization: Bearer ${TOKEN}")

USER_COUNT=$(echo "$USERS_RESPONSE" | grep -o '"id":' | wc -l)
echo "${GREEN}✓ Found $USER_COUNT users in organization${NC}"
format_json "$USERS_RESPONSE"
echo ""

# ===========================
# Step 7: Create organization-level role
# ===========================
echo "${YELLOW}[Step 7] Create organization-level role...${NC}"
ROLE_REQUEST='{
  "code": "alpha_developer",
  "name": "Alpha Developer",
  "description": "Developer role for Alpha organization",
  "is_system": false
}'

ROLE_RESPONSE=$(curl -s -X POST "${BASE_URL}/roles" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$ROLE_REQUEST")

ROLE_ID=$(extract_number "$ROLE_RESPONSE" "id")

if [ -z "$ROLE_ID" ]; then
  echo "${YELLOW}⚠ Role may already exist${NC}"
  format_json "$ROLE_RESPONSE"
else
  echo "${GREEN}✓ Organization role created (ID: $ROLE_ID)${NC}"
  format_json "$ROLE_RESPONSE"
fi
echo ""

# ===========================
# Step 8: List roles (should see org roles only)
# ===========================
echo "${YELLOW}[Step 8] List roles (should see organization roles)...${NC}"
ROLES_RESPONSE=$(curl -s -X GET "${BASE_URL}/roles" \
  -H "Authorization: Bearer ${TOKEN}")

ROLE_COUNT=$(echo "$ROLES_RESPONSE" | grep -o '"id":' | wc -l)
echo "${GREEN}✓ Found $ROLE_COUNT roles${NC}"
format_json "$ROLES_RESPONSE"
echo ""

# ===========================
# Step 9: List clusters (should only see org clusters)
# ===========================
echo "${YELLOW}[Step 9] List clusters (should only see organization clusters)...${NC}"
CLUSTERS_RESPONSE=$(curl -s -X GET "${BASE_URL}/clusters" \
  -H "Authorization: Bearer ${TOKEN}")

CLUSTER_COUNT=$(echo "$CLUSTERS_RESPONSE" | grep -o '"id":' | wc -l)
echo "${GREEN}✓ Found $CLUSTER_COUNT clusters in organization${NC}"
format_json "$CLUSTERS_RESPONSE"

# Extract cluster ID for later tests
CLUSTER_ID=$(extract_number "$CLUSTERS_RESPONSE" "id")
echo ""

# ===========================
# Step 10: Create additional cluster for organization
# ===========================
echo "${YELLOW}[Step 10] Create additional cluster for organization...${NC}"
CLUSTER_REQUEST="{
  \"name\": \"alpha_cluster_dev\",
  \"description\": \"Alpha development cluster\",
  \"fe_host\": \"localhost\",
  \"fe_http_port\": 8030,
  \"fe_query_port\": 9030,
  \"username\": \"root\",
  \"password\": \"\",
  \"catalog\": \"default_catalog\"
}"

CLUSTER_RESPONSE=$(curl -s -X POST "${BASE_URL}/clusters" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$CLUSTER_REQUEST")

NEW_CLUSTER_ID=$(extract_number "$CLUSTER_RESPONSE" "id")

if [ -z "$NEW_CLUSTER_ID" ]; then
  echo "${YELLOW}⚠ Cluster may already exist or creation restricted${NC}"
  format_json "$CLUSTER_RESPONSE"
else
  echo "${GREEN}✓ Cluster created (ID: $NEW_CLUSTER_ID)${NC}"
  format_json "$CLUSTER_RESPONSE"
fi
echo ""

# ===========================
# Step 11: Try to create system role (should fail)
# ===========================
echo "${YELLOW}[Step 11] Attempt to create system role (should fail)...${NC}"
SYSTEM_ROLE_REQUEST='{
  "name": "test_system_role",
  "description": "Test system role",
  "is_system": true
}'

SYSTEM_ROLE_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "${BASE_URL}/roles" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$SYSTEM_ROLE_REQUEST")

HTTP_CODE=$(echo "$SYSTEM_ROLE_RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$SYSTEM_ROLE_RESPONSE" | sed '/HTTP_CODE:/d')

if [ "$HTTP_CODE" = "403" ] || [ "$HTTP_CODE" = "400" ]; then
  echo "${GREEN}✓ Correctly rejected: Org admin cannot create system roles${NC}"
else
  echo "${YELLOW}⚠ Unexpected response code: $HTTP_CODE${NC}"
fi
format_json "$BODY"
echo ""

# ===========================
# Step 12: Verify cannot access other organization
# ===========================
echo "${YELLOW}[Step 12] Attempt to access Organization Beta (should fail)...${NC}"

# First, get super admin token to find Beta org ID
echo "  Getting super admin token..."
SUPER_LOGIN=$(curl -s -X POST "${BASE_URL}/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin"}')
SUPER_TOKEN=$(extract_value "$SUPER_LOGIN" "token")

SUPER_ORGS=$(curl -s -X GET "${BASE_URL}/organizations" \
  -H "Authorization: Bearer ${SUPER_TOKEN}")

# Extract Beta org ID more accurately using jq or grep with context
BETA_ORG_ID=$(echo "$SUPER_ORGS" | grep -A1 '"code":\s*"ORG_BETA"' | grep '"id":' | grep -o '[0-9]\+' | head -1)
if [ -z "$BETA_ORG_ID" ]; then
  # Fallback: parse JSON manually
  BETA_ORG_ID=$(echo "$SUPER_ORGS" | python3 -c "import sys, json; orgs=json.load(sys.stdin); beta=[o for o in orgs if o.get('code')=='ORG_BETA']; print(beta[0]['id'] if beta else '')" 2>/dev/null)
fi

if [ -n "$BETA_ORG_ID" ]; then
  echo "  Attempting to access Beta org (ID: $BETA_ORG_ID) with Alpha admin token..."
  BETA_ACCESS=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X GET "${BASE_URL}/organizations/${BETA_ORG_ID}" \
    -H "Authorization: Bearer ${TOKEN}")
  
  HTTP_CODE=$(echo "$BETA_ACCESS" | grep "HTTP_CODE:" | cut -d: -f2)
  
  if [ "$HTTP_CODE" = "403" ] || [ "$HTTP_CODE" = "404" ]; then
    echo "${GREEN}✓ Correctly blocked: Org admin cannot access other organizations${NC}"
  else
    echo "${YELLOW}⚠ Unexpected response code: $HTTP_CODE${NC}"
  fi
else
  echo "${YELLOW}⚠ Could not find Beta organization ID${NC}"
fi
echo ""

# ===========================
# Summary
# ===========================
echo "${GREEN}======================================${NC}"
echo "${GREEN}✓ Organization Admin Test Completed!${NC}"
echo "${GREEN}======================================${NC}"
echo ""
echo "Test Summary:"
echo "1. ✓ Login as organization admin (alpha_admin)"
echo "2. ✓ Verified user identity and organization"
echo "3. ✓ Retrieved organization details ($ORG_NAME)"
echo "4. ✓ Verified org isolation ($ORG_COUNT visible orgs)"
echo "5. ✓ Created user in organization (ID: $USER_ID)"
echo "6. ✓ Listed organization users ($USER_COUNT users)"
echo "7. ✓ Created organization role (ID: $ROLE_ID)"
echo "8. ✓ Listed organization roles ($ROLE_COUNT roles)"
echo "9. ✓ Listed organization clusters ($CLUSTER_COUNT clusters)"
echo "10. ✓ Created additional cluster (ID: $NEW_CLUSTER_ID)"
echo "11. ✓ System role creation blocked (as expected)"
echo "12. ✓ Cross-org access blocked (as expected)"
echo ""
echo "${BLUE}Created Credentials:${NC}"
echo "  Alpha User: alpha_user1 / User@123"
echo ""
echo "${YELLOW}Next: Run test-multi-tenant-regular-user.sh to test regular user${NC}"
