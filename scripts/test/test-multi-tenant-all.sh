#!/bin/bash

# Multi-Tenant Complete Test Suite
# Execute all multi-tenant tests in sequence

set -e

# Color output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "${CYAN}================================================${NC}"
echo "${CYAN}Multi-Tenant Complete Test Suite${NC}"
echo "${CYAN}================================================${NC}"
echo ""
echo "This script will run all multi-tenant tests:"
echo "  1. Super Admin Test"
echo "  2. Organization Admin Test"
echo "  3. Regular User Test"
echo ""
echo "${YELLOW}Make sure the backend is running on port 8081${NC}"
echo "${YELLOW}Press Enter to continue or Ctrl+C to cancel...${NC}"
read

START_TIME=$(date +%s)

# ===========================
# Test 1: Super Admin
# ===========================
echo ""
echo "${CYAN}================================================${NC}"
echo "${CYAN}TEST 1/3: Super Admin${NC}"
echo "${CYAN}================================================${NC}"
echo ""

if bash "${SCRIPT_DIR}/test-multi-tenant-super-admin.sh"; then
  echo "${GREEN}✓ Super Admin Test PASSED${NC}"
  TEST1_RESULT="PASS"
else
  echo "${RED}✗ Super Admin Test FAILED${NC}"
  TEST1_RESULT="FAIL"
  exit 1
fi

echo ""
echo "${YELLOW}Waiting 3 seconds before next test...${NC}"
sleep 3

# ===========================
# Test 2: Organization Admin
# ===========================
echo ""
echo "${CYAN}================================================${NC}"
echo "${CYAN}TEST 2/3: Organization Admin${NC}"
echo "${CYAN}================================================${NC}"
echo ""

if bash "${SCRIPT_DIR}/test-multi-tenant-org-admin.sh"; then
  echo "${GREEN}✓ Organization Admin Test PASSED${NC}"
  TEST2_RESULT="PASS"
else
  echo "${RED}✗ Organization Admin Test FAILED${NC}"
  TEST2_RESULT="FAIL"
  exit 1
fi

echo ""
echo "${YELLOW}Waiting 3 seconds before next test...${NC}"
sleep 3

# ===========================
# Test 3: Regular User
# ===========================
echo ""
echo "${CYAN}================================================${NC}"
echo "${CYAN}TEST 3/3: Regular User${NC}"
echo "${CYAN}================================================${NC}"
echo ""

if bash "${SCRIPT_DIR}/test-multi-tenant-regular-user.sh"; then
  echo "${GREEN}✓ Regular User Test PASSED${NC}"
  TEST3_RESULT="PASS"
else
  echo "${RED}✗ Regular User Test FAILED${NC}"
  TEST3_RESULT="FAIL"
  exit 1
fi

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# ===========================
# Final Summary
# ===========================
echo ""
echo "${CYAN}================================================${NC}"
echo "${CYAN}COMPLETE TEST SUITE SUMMARY${NC}"
echo "${CYAN}================================================${NC}"
echo ""
echo "Test Results:"
echo "  1. Super Admin Test:       ${TEST1_RESULT}"
echo "  2. Organization Admin Test: ${TEST2_RESULT}"
echo "  3. Regular User Test:       ${TEST3_RESULT}"
echo ""
echo "Total Duration: ${DURATION} seconds"
echo ""

if [ "$TEST1_RESULT" = "PASS" ] && [ "$TEST2_RESULT" = "PASS" ] && [ "$TEST3_RESULT" = "PASS" ]; then
  echo "${GREEN}================================================${NC}"
  echo "${GREEN}✓ ALL TESTS PASSED${NC}"
  echo "${GREEN}================================================${NC}"
  echo ""
  echo "${BLUE}Multi-Tenant Feature Verification:${NC}"
  echo "  ✓ Organizations created and isolated"
  echo "  ✓ Super admin can manage all resources"
  echo "  ✓ Org admins are scoped to their organization"
  echo "  ✓ Regular users have limited permissions"
  echo "  ✓ Cross-org access is blocked"
  echo "  ✓ Role-based access control working"
  echo ""
  echo "${GREEN}The multi-tenant system is working correctly!${NC}"
else
  echo "${RED}================================================${NC}"
  echo "${RED}✗ SOME TESTS FAILED${NC}"
  echo "${RED}================================================${NC}"
  exit 1
fi
