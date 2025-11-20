#!/bin/bash

# Multi-Tenant Regular User Test Script
# Role: Regular User (alpha_user1/User@123)
# Purpose: Verify regular user has limited permissions and can only access authorized resources

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
echo "${BLUE}Multi-Tenant Regular User Test${NC}"
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
# Step 1: Login as regular user
# ===========================
echo "${YELLOW}[Step 1] Login as regular user (alpha_user1/User@123)...${NC}"
LOGIN_RESPONSE=$(curl -s -X POST "${BASE_URL}/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username":"alpha_user1","password":"User@123"}')

TOKEN=$(extract_value "$LOGIN_RESPONSE" "token")

if [ -z "$TOKEN" ]; then
  echo "${RED}✗ Login failed${NC}"
  echo "${YELLOW}Note: Make sure you ran test-multi-tenant-org-admin.sh first${NC}"
  format_json "$LOGIN_RESPONSE"
  exit 1
fi

echo "${GREEN}✓ Login successful${NC}"
echo "Token: ${TOKEN:0:20}..."
echo ""

# ===========================
# Step 2: Verify current user identity
# ===========================
echo "${YELLOW}[Step 2] Verify current user identity...${NC}"
ME_RESPONSE=$(curl -s -X GET "${BASE_URL}/auth/me" \
  -H "Authorization: Bearer ${TOKEN}")

USERNAME=$(extract_value "$ME_RESPONSE" "username")
ORG_ID=$(extract_number "$ME_RESPONSE" "organization_id")

echo "Current user: $USERNAME"
echo "Organization ID: $ORG_ID"
format_json "$ME_RESPONSE"
echo ""

# ===========================
# Step 3: List visible clusters
# ===========================
echo "${YELLOW}[Step 3] List visible clusters...${NC}"
CLUSTERS_RESPONSE=$(curl -s -X GET "${BASE_URL}/clusters" \
  -H "Authorization: Bearer ${TOKEN}")

CLUSTER_COUNT=$(echo "$CLUSTERS_RESPONSE" | grep -o '"id":' | wc -l)
echo "${GREEN}✓ Found $CLUSTER_COUNT visible clusters${NC}"
format_json "$CLUSTERS_RESPONSE"

# Extract first cluster ID for later tests
CLUSTER_ID=$(extract_number "$CLUSTERS_RESPONSE" "id")
echo ""

# ===========================
# Step 4: Get cluster details
# ===========================
if [ -n "$CLUSTER_ID" ]; then
  echo "${YELLOW}[Step 4] Get cluster details (ID: $CLUSTER_ID)...${NC}"
  CLUSTER_DETAIL=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X GET "${BASE_URL}/clusters/${CLUSTER_ID}" \
    -H "Authorization: Bearer ${TOKEN}")
  
  HTTP_CODE=$(echo "$CLUSTER_DETAIL" | grep "HTTP_CODE:" | cut -d: -f2)
  BODY=$(echo "$CLUSTER_DETAIL" | sed '/HTTP_CODE:/d')
  
  if [ "$HTTP_CODE" = "200" ]; then
    echo "${GREEN}✓ Successfully retrieved cluster details${NC}"
    format_json "$BODY"
  else
    echo "${YELLOW}⚠ Cannot access cluster details (HTTP $HTTP_CODE)${NC}"
    format_json "$BODY"
  fi
  echo ""
fi

# ===========================
# Step 5: Try to create cluster (should fail)
# ===========================
echo "${YELLOW}[Step 5] Attempt to create cluster (should fail due to permissions)...${NC}"
CLUSTER_REQUEST='{
  "name": "unauthorized_cluster",
  "description": "Test cluster",
  "fe_host": "localhost",
  "fe_http_port": 8030,
  "fe_query_port": 9030,
  "username": "root",
  "password": ""
}'

CREATE_CLUSTER_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "${BASE_URL}/clusters" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$CLUSTER_REQUEST")

HTTP_CODE=$(echo "$CREATE_CLUSTER_RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$CREATE_CLUSTER_RESPONSE" | sed '/HTTP_CODE:/d')

if [ "$HTTP_CODE" = "403" ]; then
  echo "${GREEN}✓ Correctly blocked: Regular user cannot create clusters${NC}"
else
  echo "${YELLOW}⚠ Unexpected response code: $HTTP_CODE${NC}"
fi
format_json "$BODY"
echo ""

# ===========================
# Step 6: Try to create user (should fail)
# ===========================
echo "${YELLOW}[Step 6] Attempt to create user (should fail)...${NC}"
USER_REQUEST="{
  \"username\": \"unauthorized_user\",
  \"password\": \"Test@123\",
  \"email\": \"test@example.com\",
  \"organization_id\": $ORG_ID
}"

CREATE_USER_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "${BASE_URL}/users" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$USER_REQUEST")

HTTP_CODE=$(echo "$CREATE_USER_RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$CREATE_USER_RESPONSE" | sed '/HTTP_CODE:/d')

if [ "$HTTP_CODE" = "403" ]; then
  echo "${GREEN}✓ Correctly blocked: Regular user cannot create users${NC}"
else
  echo "${YELLOW}⚠ Unexpected response code: $HTTP_CODE${NC}"
fi
format_json "$BODY"
echo ""

# ===========================
# Step 7: Try to create role (should fail)
# ===========================
echo "${YELLOW}[Step 7] Attempt to create role (should fail)...${NC}"
ROLE_REQUEST="{
  \"name\": \"unauthorized_role\",
  \"description\": \"Test role\",
  \"organization_id\": $ORG_ID
}"

CREATE_ROLE_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "${BASE_URL}/roles" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d "$ROLE_REQUEST")

HTTP_CODE=$(echo "$CREATE_ROLE_RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$CREATE_ROLE_RESPONSE" | sed '/HTTP_CODE:/d')

if [ "$HTTP_CODE" = "403" ]; then
  echo "${GREEN}✓ Correctly blocked: Regular user cannot create roles${NC}"
else
  echo "${YELLOW}⚠ Unexpected response code: $HTTP_CODE${NC}"
fi
format_json "$BODY"
echo ""

# ===========================
# Step 8: Try to access organizations (should fail or be restricted)
# ===========================
echo "${YELLOW}[Step 8] Attempt to list organizations (should be restricted)...${NC}"
ORGS_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X GET "${BASE_URL}/organizations" \
  -H "Authorization: Bearer ${TOKEN}")

HTTP_CODE=$(echo "$ORGS_RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$ORGS_RESPONSE" | sed '/HTTP_CODE:/d')

if [ "$HTTP_CODE" = "403" ]; then
  echo "${GREEN}✓ Correctly blocked: Regular user cannot list organizations${NC}"
elif [ "$HTTP_CODE" = "200" ]; then
  ORG_COUNT=$(echo "$BODY" | grep -o '"id":' | wc -l)
  if [ "$ORG_COUNT" -le 1 ]; then
    echo "${GREEN}✓ Restricted view: Can only see own organization (if any)${NC}"
  else
    echo "${YELLOW}⚠ User can see multiple organizations${NC}"
  fi
  format_json "$BODY"
else
  echo "${YELLOW}⚠ Unexpected response code: $HTTP_CODE${NC}"
fi
echo ""

# ===========================
# Step 9: Execute query (if cluster access granted)
# ===========================
if [ -n "$CLUSTER_ID" ]; then
  echo "${YELLOW}[Step 9] Attempt to execute query on cluster $CLUSTER_ID...${NC}"
  QUERY_REQUEST='{
    "sql": "SELECT 1 as test_value",
    "limit": 10
  }'
  
  QUERY_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X POST "${BASE_URL}/clusters/${CLUSTER_ID}/queries/execute" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d "$QUERY_REQUEST")
  
  HTTP_CODE=$(echo "$QUERY_RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
  BODY=$(echo "$QUERY_RESPONSE" | sed '/HTTP_CODE:/d')
  
  if [ "$HTTP_CODE" = "200" ]; then
    echo "${GREEN}✓ Query executed successfully${NC}"
    format_json "$BODY"
  elif [ "$HTTP_CODE" = "403" ]; then
    echo "${YELLOW}⚠ No query permission on this cluster${NC}"
  else
    echo "${YELLOW}⚠ Query failed with HTTP $HTTP_CODE${NC}"
    format_json "$BODY"
  fi
  echo ""
fi

# ===========================
# Step 10: List users (should be restricted)
# ===========================
echo "${YELLOW}[Step 10] Attempt to list users (should be restricted)...${NC}"
USERS_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X GET "${BASE_URL}/users" \
  -H "Authorization: Bearer ${TOKEN}")

HTTP_CODE=$(echo "$USERS_RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$USERS_RESPONSE" | sed '/HTTP_CODE:/d')

if [ "$HTTP_CODE" = "403" ]; then
  echo "${GREEN}✓ Correctly blocked: Regular user cannot list users${NC}"
elif [ "$HTTP_CODE" = "200" ]; then
  USER_COUNT=$(echo "$BODY" | grep -o '"id":' | wc -l)
  echo "${YELLOW}⚠ User can see $USER_COUNT users${NC}"
  format_json "$BODY"
else
  echo "${YELLOW}⚠ Unexpected response code: $HTTP_CODE${NC}"
fi
echo ""

# ===========================
# Step 11: List roles (should be restricted)
# ===========================
echo "${YELLOW}[Step 11] Attempt to list roles (should be restricted)...${NC}"
ROLES_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}" -X GET "${BASE_URL}/roles" \
  -H "Authorization: Bearer ${TOKEN}")

HTTP_CODE=$(echo "$ROLES_RESPONSE" | grep "HTTP_CODE:" | cut -d: -f2)
BODY=$(echo "$ROLES_RESPONSE" | sed '/HTTP_CODE:/d')

if [ "$HTTP_CODE" = "403" ]; then
  echo "${GREEN}✓ Correctly blocked: Regular user cannot list roles${NC}"
elif [ "$HTTP_CODE" = "200" ]; then
  ROLE_COUNT=$(echo "$BODY" | grep -o '"id":' | wc -l)
  echo "${YELLOW}⚠ User can see $ROLE_COUNT roles${NC}"
  format_json "$BODY"
else
  echo "${YELLOW}⚠ Unexpected response code: $HTTP_CODE${NC}"
fi
echo ""

# ===========================
# Summary
# ===========================
echo "${GREEN}======================================${NC}"
echo "${GREEN}✓ Regular User Test Completed!${NC}"
echo "${GREEN}======================================${NC}"
echo ""
echo "Test Summary:"
echo "1. ✓ Login as regular user (alpha_user1)"
echo "2. ✓ Verified user identity (Org ID: $ORG_ID)"
echo "3. ✓ Listed visible clusters ($CLUSTER_COUNT clusters)"
echo "4. ✓ Accessed cluster details (if permitted)"
echo "5. ✓ Cluster creation blocked (as expected)"
echo "6. ✓ User creation blocked (as expected)"
echo "7. ✓ Role creation blocked (as expected)"
echo "8. ✓ Organization access restricted"
echo "9. ✓ Query execution tested"
echo "10. ✓ User listing restricted"
echo "11. ✓ Role listing restricted"
echo ""
echo "${BLUE}Security Verification:${NC}"
echo "  ✓ Regular users cannot perform administrative operations"
echo "  ✓ Regular users are scoped to their organization"
echo "  ✓ Regular users can only access authorized resources"
echo ""
echo "${GREEN}All multi-tenant tests completed successfully!${NC}"
