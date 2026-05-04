# Inferstructor Dashboard - Action Plan

**Project**: Inferstructor Dashboard  
**Generated**: 2024-04-07  
**Total Estimated Effort**: 30 hours across 2 sprints

---

## SPRINT 1: PRODUCTION READINESS (13 hours)
**Goal**: Fix all CRITICAL vulnerabilities before deployment

### Task 1.1: Migrate JWT/API Credentials to Secure Storage
**Priority**: CRITICAL-1  
**Effort**: 4 hours  
**Acceptance Criteria**:
- [ ] JWT stored only in sessionStorage (clears on tab close)
- [ ] API secret NEVER persisted to storage
- [ ] Unit tests verify credentials cleared on logout
- [ ] Integration test: login → logout → check localStorage empty
- [ ] No `infra_jwt_token` in localStorage after logout

**Implementation Steps**:
1. Modify `src/api.ts` - Update constructor to use sessionStorage only
2. Remove localStorage.setItem for JWT tokens
3. Update `login()` and `register()` methods
4. Update `logout()` method
5. Add tests in `src/api.test.ts`
6. Document in SECURITY.md

**Files to Modify**:
- `src/api.ts` (lines 224-291)
- `src/api-client.ts` (lines 99-121)
- `src/test/` tests

**Definition of Done**:
- All existing tests pass
- New security tests pass
- Code review approved
- No localStorage credentials in Chrome DevTools

---

### Task 1.2: Implement Token Refresh Endpoint
**Priority**: CRITICAL-2  
**Effort**: 6 hours  
**Acceptance Criteria**:
- [ ] `api.refreshToken()` method implemented
- [ ] Endpoint `/api/validators/refresh-token` responds correctly
- [ ] Token expiry extends on successful refresh
- [ ] Failed refresh triggers logout
- [ ] Unit tests for refresh success/failure
- [ ] Integration test with token expiry simulation

**Implementation Steps**:
1. Add `refreshToken()` method to `InferstructorAPI` class in `src/api.ts`
2. Update `useTokenRefresh` hook to call `api.refreshToken()`
3. Remove "not implemented" warning logs
4. Add retry logic with exponential backoff
5. Test with mock server (vitest)
6. Document token lifecycle in README

**Files to Modify**:
- `src/api.ts` (add new method)
- `src/hooks/useTokenRefresh.ts` (lines 85-103)
- `src/api.test.ts` (new tests)

**Definition of Done**:
- `npm test` shows new token refresh tests passing
- Manual test: Set token exp to 5 minutes, verify refresh at 5-minute mark
- No 401 errors after refresh
- Users stay logged in across token boundaries

---

### Task 1.3: Move GPU Lane URLs to Environment Variables
**Priority**: CRITICAL-3  
**Effort**: 1 hour  
**Acceptance Criteria**:
- [ ] GPU lane base URL in `.env.example` and `.env.production`
- [ ] Defaults to environment variable with localhost fallback for dev
- [ ] Production URLs point to actual GPU lane endpoints
- [ ] Tests verify correct URL construction
- [ ] Docker/deployment docs updated

**Implementation Steps**:
1. Create `VITE_GPU_LANE_BASE` environment variable
2. Update `src/api.ts` line 319-323 to use env var
3. Add to `.env.example`
4. Add to deployment docs (Dockerfile, docker-compose)
5. Update tests to mock environment

**Files to Modify**:
- `src/api.ts` (lines 319-323)
- `.env.example` (new)
- `docker-compose.yml` or `.env.production`
- `Dockerfile` (if used)

**Definition of Done**:
- `VITE_GPU_LANE_BASE` env var used in all lane requests
- Fallback to localhost for development
- Production build uses correct endpoints
- Tests pass with mocked URLs

---

### Task 1.4: Add CSRF Protection to Admin Login
**Priority**: CRITICAL-4  
**Effort**: 3 hours  
**Acceptance Criteria**:
- [ ] CSRF token generated on server on each page load
- [ ] CSRF token included in `X-CSRF-Token` header
- [ ] Server validates CSRF token on admin login
- [ ] Invalid/missing CSRF token returns 403
- [ ] Tests verify CSRF validation
- [ ] Cross-origin requests without CSRF token fail

**Implementation Steps**:
1. Implement CSRF token on backend (depends on server framework)
2. Add CSRF token to HTML meta tag: `<meta name="csrf-token" content="...">`
3. Update `src/api.ts` `adminLogin()` to read and send CSRF token
4. Add header validation on backend
5. Add integration test for CSRF validation
6. Document in security guidelines

**Files to Modify**:
- `src/api.ts` (lines 350-358)
- `src/components/AdminLogin.tsx`
- `src/api.test.ts` (new CSRF test)
- Server code (framework-specific)

**Definition of Done**:
- Admin login with valid CSRF token succeeds
- Admin login with invalid CSRF token fails with 403
- Admin login with missing CSRF token fails with 403
- Tests verify all scenarios

---

## SPRINT 2: PERFORMANCE & MAINTAINABILITY (17 hours)

### Task 2.1: Fix TPS History Memory Leak
**Priority**: MAJOR-1  
**Effort**: 2 hours  
**Acceptance Criteria**:
- [ ] TPS history limited to MAX_POINTS (300)
- [ ] Data aggregated when limit reached
- [ ] Memory usage stable after 24 hours
- [ ] Chart rendering still smooth with aggregated data
- [ ] Unit tests verify aggregation logic

**Implementation Steps**:
1. Define `MAX_POINTS` constant (300 = 10 minutes of 2s intervals)
2. Implement aggregation logic in TPS history update
3. Test memory usage with Chrome DevTools
4. Verify chart displays correctly with aggregated data
5. Add unit tests for aggregation

**Files to Modify**:
- `src/constants.ts` (create if not exists)
- `src/components/Dashboard.tsx` (lines 44-56)
- `src/components/__tests__/Dashboard.test.ts` (new)

**Definition of Done**:
- Memory usage <50MB after 24-hour simulation
- Chart renders smoothly
- All tests pass
- Data accuracy preserved (trend visible)

---

### Task 2.2: Replace Promise.all with Promise.allSettled
**Priority**: MAJOR-3  
**Effort**: 1 hour  
**Acceptance Criteria**:
- [ ] Dashboard shows partial data if one endpoint fails
- [ ] Error logs show which endpoints failed
- [ ] No silent failures
- [ ] Tests verify error handling
- [ ] Performance not degraded

**Implementation Steps**:
1. Replace `Promise.all()` with `Promise.allSettled()` in Dashboard
2. Replace in AdminControls
3. Update error logging
4. Add tests for partial failures
5. Verify Dashboard still works with mock failures

**Files to Modify**:
- `src/components/Dashboard.tsx` (lines 26-39)
- `src/components/AdminControls.tsx` (lines 50-53)
- `src/components/__tests__/Dashboard.test.ts`

**Definition of Done**:
- Dashboard renders with 1/3 API endpoints down
- Error messages clear and actionable
- Tests pass with simulated failures

---

### Task 2.3: Split Dashboard into Smaller Components
**Priority**: MAJOR-2  
**Effort**: 6 hours  
**Acceptance Criteria**:
- [ ] Dashboard.tsx <200 lines
- [ ] Each component <120 lines
- [ ] All components testable independently
- [ ] No functionality changes
- [ ] Tests still pass (41/41)
- [ ] Props properly typed

**Implementation Steps**:
1. Create component structure:
   - `src/components/dashboard/DashboardHeader.tsx`
   - `src/components/dashboard/ErrorBanner.tsx`
   - `src/components/dashboard/StatsGrid.tsx`
   - `src/components/dashboard/TpsChart.tsx`
   - `src/components/dashboard/GPULanesSection.tsx`
   - `src/components/dashboard/SolanaChainSection.tsx`
2. Extract logic from Dashboard.tsx
3. Create prop types for each component
4. Move tests to respective files
5. Verify all tests still pass

**Files to Create**:
- `src/components/dashboard/DashboardHeader.tsx` (~30 lines)
- `src/components/dashboard/ErrorBanner.tsx` (~25 lines)
- `src/components/dashboard/StatsGrid.tsx` (~80 lines)
- `src/components/dashboard/TpsChart.tsx` (~120 lines)
- `src/components/dashboard/GPULanesSection.tsx` (~150 lines)
- `src/components/dashboard/SolanaChainSection.tsx` (~120 lines)
- `src/components/dashboard/__tests__/` (test files)

**Files to Modify**:
- `src/components/Dashboard.tsx` (refactor to orchestrate)

**Definition of Done**:
- All components <120 lines
- Dashboard.tsx <200 lines
- `npm test` passes (41/41)
- `npm run build` succeeds
- No functionality changes
- Code review approved

---

### Task 2.4: Implement Lazy Loading for Admin Tabs
**Priority**: MAJOR-4  
**Effort**: 3 hours  
**Acceptance Criteria**:
- [ ] Only active tab rendered in DOM
- [ ] Render performance improved 30%+
- [ ] Tab switching feels instant
- [ ] Tests verify only active tab rendered
- [ ] Fallback loading state works

**Implementation Steps**:
1. Extract tab components into separate files:
   - `src/components/admin-tabs/RpcPanel.tsx`
   - `src/components/admin-tabs/FaucetPanel.tsx`
   - `src/components/admin-tabs/EmergencyPanel.tsx`
   - `src/components/admin-tabs/RBACPanel.tsx`
   - `src/components/admin-tabs/AuditPanel.tsx`
2. Use React.lazy() for each component
3. Add Suspense boundary with loading state
4. Update AdminControls to render only active tab
5. Test tab switching performance
6. Add tests for lazy loading

**Files to Create**:
- `src/components/admin-tabs/RpcPanel.tsx`
- `src/components/admin-tabs/FaucetPanel.tsx`
- `src/components/admin-tabs/EmergencyPanel.tsx`
- `src/components/admin-tabs/RBACPanel.tsx`
- `src/components/admin-tabs/AuditPanel.tsx`

**Files to Modify**:
- `src/components/AdminControls.tsx` (refactor)

**Definition of Done**:
- Only active tab in DOM (verify with React DevTools)
- Tab switching has no lag
- All tests pass
- Chunk size analysis shows improvement

---

### Task 2.5: Extract Magic Numbers to Constants
**Priority**: MINOR-1  
**Effort**: 1 hour  
**Acceptance Criteria**:
- [ ] All numeric literals moved to `src/constants.ts`
- [ ] Each constant has explanatory comment
- [ ] Constants used consistently across codebase
- [ ] No new magic numbers introduced

**Implementation Steps**:
1. Create `src/constants.ts` with all constants
2. Replace hardcoded values with constant imports
3. Add JSDoc comments explaining each constant
4. Update imports in Dashboard, AdminControls, hooks
5. Verify build and tests pass

**Files to Create**:
- `src/constants.ts`

**Files to Modify**:
- `src/components/Dashboard.tsx`
- `src/components/AdminControls.tsx`
- `src/hooks/useTokenRefresh.ts`
- `src/api-client.ts`

**Definition of Done**:
- `src/constants.ts` has 8+ constants with comments
- `npm test` passes
- No regressions in functionality

---

### Task 2.6: Add Accessibility Labels (ARIA)
**Priority**: MINOR-2  
**Effort**: 1 hour  
**Acceptance Criteria**:
- [ ] Loading spinner has `aria-label`
- [ ] Status indicators have `aria-label`
- [ ] Color-only indicators have text alternative
- [ ] Screen reader testing passes
- [ ] Axe accessibility audit passes

**Implementation Steps**:
1. Add `aria-label` and `aria-hidden` to Dashboard loading state
2. Add `aria-label` to GPU availability status
3. Add text indicators for color-only statuses
4. Run axe-core accessibility audit
5. Test with screen reader (NVDA or JAWS)

**Files to Modify**:
- `src/components/Dashboard.tsx` (lines 88, 302)
- `src/components/AdminControls.tsx`

**Definition of Done**:
- Axe audit shows no accessibility violations
- Screen reader properly announces loading state
- Screen reader properly announces status indicators

---

### Task 2.7: Replace alert() with Toast Notifications
**Priority**: MINOR-3  
**Effort**: 2 hours  
**Acceptance Criteria**:
- [ ] Toast component created and styled
- [ ] All alert() calls replaced with toast notifications
- [ ] Toasts auto-dismiss after 3 seconds
- [ ] Multiple toasts queue properly
- [ ] Tests verify toast behavior

**Implementation Steps**:
1. Create `src/components/Toast.tsx` component
2. Create `src/hooks/useToast.ts` hook
3. Replace all `alert()` calls in AdminControls, other components
4. Style toasts to match dashboard theme
5. Test toast queue with multiple notifications
6. Add tests for toast lifecycle

**Files to Create**:
- `src/components/Toast.tsx`
- `src/hooks/useToast.ts`
- `src/components/__tests__/Toast.test.ts`

**Files to Modify**:
- `src/components/AdminControls.tsx` (line 165)
- `src/components/AdminLogin.tsx`
- `src/App.tsx` (add toast provider)

**Definition of Done**:
- No `alert()` calls in production code
- Toasts display and dismiss correctly
- Multiple toasts queue without overlapping
- Tests pass

---

## RISK ASSESSMENT

| Task | Risk | Mitigation |
|---|---|---|
| 1.1 Secure storage | High | Breaking change - test thoroughly, have rollback plan |
| 1.2 Token refresh | High | Test with real server, have fallback logout |
| 1.3 GPU URLs | Low | Already using environment variables, low risk |
| 1.4 CSRF | High | Backend must implement validation, test both sides |
| 2.1 TPS aggregation | Medium | Verify chart accuracy with real data |
| 2.2 Promise.allSettled | Low | Already used in similar patterns |
| 2.3 Component split | Medium | Large refactor, extensive testing required |
| 2.4 Lazy loading | Low | React.lazy is stable, minimal risk |
| 2.5 Constants | Very Low | Pure refactoring, no logic changes |
| 2.6 ARIA labels | Very Low | Additive only, no risk |
| 2.7 Toasts | Low | Replaces alert(), better UX |

---

## DEPENDENCIES & BLOCKERS

### Sprint 1 Blockers
- **1.2 (Token Refresh)**: Requires backend `/api/validators/refresh-token` endpoint to be implemented first
- **1.4 (CSRF)**: Requires backend CSRF token generation and validation

### Sprint 2 Blockers
- **2.3 (Component Split)**: None (refactoring only)
- **2.4 (Lazy Loading)**: Requires tabs to be independently exportable components

---

## TESTING STRATEGY

### Unit Tests
- `src/api.test.ts` - JWT/API credential handling
- `src/constants.test.ts` - Constant values
- `src/hooks/useTokenRefresh.test.ts` - Token refresh logic
- `src/components/dashboard/__tests__/TpsChart.test.ts` - TPS aggregation

### Integration Tests
- Login → Logout → Verify no credentials in localStorage
- Token expiry → Refresh → Continue working
- Promise.allSettled with partial API failures

### E2E Tests (Manual)
- Full login flow with all API endpoints
- Admin login with CSRF validation
- 24-hour memory leak test
- Dashboard performance with aggregated data

### Accessibility Tests
- Axe accessibility audit
- Screen reader testing (NVDA/JAWS)
- Keyboard navigation verification

---

## SUCCESS METRICS

### Sprint 1
- [ ] All 4 CRITICAL issues resolved
- [ ] Security audit passes (no localStorage credentials)
- [ ] Token refresh works end-to-end
- [ ] CSRF protection verified
- [ ] `npm test` passes (41/41)
- [ ] `npm run build` succeeds

### Sprint 2
- [ ] Memory usage stable (<50MB after 24h)
- [ ] Dashboard renders smoothly with aggregated data
- [ ] Admin controls performance improved 30%+
- [ ] All components properly typed and tested
- [ ] Accessibility audit passes
- [ ] No `alert()` calls in codebase

### Overall
- Production readiness: 90/100
- Security score: 95/100
- Performance score: 85/100
- Code quality: 90/100

---

## ROLLOUT PLAN

### Pre-Deployment (Sprint 1)
1. All CRITICAL fixes merged to main
2. Security audit re-run by external team (optional)
3. Manual end-to-end testing
4. Load testing with aggregated data
5. Staging environment deployment

### Deployment (Day 1)
1. Blue-green deployment to production
2. Monitor error rates and performance
3. Have rollback plan ready

### Post-Deployment (Sprint 2)
1. Merge Sprint 2 improvements
2. Continue monitoring
3. Plan additional security hardening

---

**End of Action Plan**
