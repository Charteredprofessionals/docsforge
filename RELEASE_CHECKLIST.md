# DocForge v2.0.0 Release Checklist

**Target Release:** v2.0.0 (Bundle + Matter Domain Model)  
**Status:** 🔴 **BLOCKED** - Awaiting TASK-119 completion  
**Last Updated:** August 28, 2026

---

## 🔴 Critical Blockers (Must Complete Before Release)

### 1. TASK-119: v2 UI Implementation
**Status:** ❌ INCOMPLETE  
**Priority:** P0 - RELEASE BLOCKER  
**Estimated Effort:** 40-60 hours

#### Components to Implement

- [ ] **MatterForm.tsx** (~15-20 hours)
  - Implement grouped form rendering (shared vs document-specific)
  - Wire up `render_matter_form_cmd` Tauri command
  - Add field validation UI with inline error display
  - Support 13 field types (text, multiline_text, number, currency, percentage, date, datetime, boolean, email, phone, url, select, multiselect)
  - Auto-save draft matter data
  - Reference: REQ-027, REQ-031, AC-031

- [ ] **GenerationHistory.tsx** (~12-15 hours)
  - Display generation runs with metadata (timestamp, bundle version, engine version)
  - Show generation preview (documents to generate + skipped-with-reason)
  - Wire up `list_runs_cmd`, `evaluate_preview_cmd`
  - Download generated documents (DOCX/PDF)
  - Rerun historical generation with same inputs
  - Reference: REQ-033, REQ-034, REQ-035, REQ-036, AC-035, AC-036

- [ ] **BundlesScreen.tsx** (~13-18 hours)
  - Replace existing `Bundles.tsx` with v2 implementation
  - Bundle CRUD (create, list, edit, delete)
  - Version management UI (draft → review → publish → archive)
  - .dfpkg import/export buttons
  - Bundle health check display (unmapped placeholders, validation errors)
  - Publish version workflow with confirmation
  - Wire up: `create_bundle_v2_cmd`, `publish_version_cmd`, `export_bundle_dfpkg_cmd`, `import_bundle_dfpkg_cmd`
  - Reference: REQ-023, REQ-024, REQ-025, REQ-038, AC-023, AC-024, AC-025, AC-038

- [ ] **App.tsx Navigation Refactor** (~5-7 hours)
  - Current: Templates / New Template / Bundles / Admin
  - New: Dashboard / Bundles / Matters / Generated Documents
  - Add routing logic for Matter detail view
  - Add routing for Generation preview/history
  - Breadcrumb navigation for nested views
  - Reference: REQ-031, REQ-035, REQ-036

#### Testing Requirements
- [ ] Unit tests for each new component (vitest)
- [ ] Integration tests for form submission flows
- [ ] E2E test: Create Bundle → Create Matter → Fill Form → Generate Documents

### 2. contract_v2.py Test Suite
**Status:** ❌ MISSING  
**Priority:** P0 - RELEASE BLOCKER  
**Estimated Effort:** 8-12 hours

- [ ] Bundle contract tests
  - [ ] Create bundle with multiple documents
  - [ ] Publish bundle version (immutability check)
  - [ ] Export/import .dfpkg round-trip
  
- [ ] Field mapping contract tests
  - [ ] Create fields with all 13 types
  - [ ] Set mappings (placeholder → field)
  - [ ] Resolve values from matter data
  
- [ ] Matter contract tests
  - [ ] Create matter bound to bundle version
  - [ ] Set matter data (single source for all documents)
  - [ ] Validate matter (3-level validation)
  
- [ ] Rules contract tests
  - [ ] Add conditional document rule
  - [ ] Evaluate rules (Include/Exclude with reason)
  - [ ] Reject invalid expressions
  
- [ ] Generation contract tests
  - [ ] Execute run (generate all documents)
  - [ ] Execute run (generate selected documents)
  - [ ] Verify output naming follows bundle config
  - [ ] Verify immutability (historical runs never mutated)

---

## 🟡 High Priority (Pre-Release)

### 3. Security & Dependencies
**Status:** ⚠️ PARTIAL

- [x] **DOMPurify Security Update** - COMPLETED
  - [x] Updated from v3.0.6 to v3.4.14 (CVE-2026-41238 fixed)
  - [ ] Run `npm install` to apply update to node_modules
  - [ ] Verify DOMPurify version in production build

- [ ] **Dependency Audit**
  - [ ] Run `npm audit` and fix high/critical issues
  - [ ] Run `cargo audit` and address vulnerabilities
  - [ ] Update outdated dependencies (non-breaking)

- [ ] **Security Scan**
  - [ ] Run SAST scan on Rust code
  - [ ] Review all `unsafe` blocks (currently: 0)
  - [ ] Verify CSP headers in production build

### 4. Code Quality
**Status:** ✅ COMPLETED (with follow-ups)

- [x] **Fix Rust Warnings** - COMPLETED
  - [x] Removed 10 warnings in commands.rs
  - [ ] Verify clean build: `cargo build --release 2>&1 | grep warning`
  - [ ] Verify no clippy warnings: `cargo clippy --workspace --all-targets`

- [ ] **Code Review**
  - [x] AC-040: No docx manipulation in src/ - VERIFIED
  - [ ] Review all new v2 code for best practices
  - [ ] Check for TODO/FIXME comments requiring action

### 5. Documentation
**Status:** ⚠️ PARTIAL

- [x] **Audit Report** - COMPLETED
  - [x] Generated comprehensive audit report
  
- [ ] **User Manual Updates**
  - [ ] Update USER_MANUAL.md for v2 workflows
  - [ ] Add Bundle creation guide
  - [ ] Add Matter workflow guide
  - [ ] Add Generation preview screenshots
  
- [ ] **Developer Documentation**
  - [ ] Update architecture.md if changes made
  - [ ] Document v2 API changes
  - [ ] Add migration guide (v1 templates → v2 bundles)

- [ ] **CHANGELOG**
  - [ ] Document breaking changes from v1.0.0
  - [ ] List new features (Bundle, Matter, Rules, Generation)
  - [ ] List bug fixes and security updates
  - [ ] Migration notes for existing users

---

## 🟢 Nice to Have (Post-Release Candidates)

### 6. Testing & Quality Assurance

- [ ] **Automated Tests**
  - [ ] Increase test coverage to 80%+
  - [ ] Add mutation testing for critical paths
  - [ ] Performance benchmarks (large documents)

- [ ] **Manual Testing**
  - [ ] Full user journey on clean Windows VM
  - [ ] Test with 100+ document bundle
  - [ ] Test with 1000+ matters
  - [ ] Accessibility testing (screen readers)

### 7. Performance & Optimization

- [ ] **Profiling**
  - [ ] Profile large document generation (memory usage)
  - [ ] Benchmark fill operation performance
  - [ ] Check SQLite query performance
  
- [ ] **Optimization**
  - [ ] Optimize React re-renders
  - [ ] Add pagination for large matter lists
  - [ ] Lazy-load bundle documents

### 8. DevOps & CI/CD

- [ ] **Build Pipeline**
  - [ ] Set up GitHub Actions / CI pipeline
  - [ ] Automated testing on push
  - [ ] Automated security scanning
  
- [ ] **Release Automation**
  - [ ] Automated version bumping
  - [ ] Automated changelog generation
  - [ ] Signed binaries for all platforms

### 9. Installer & Distribution

- [ ] **Windows Installer**
  - [ ] Test MSI installer on clean VM
  - [ ] Test NSIS installer
  - [ ] Test MSIX package (requires code signing cert)
  - [ ] Verify silent install via policy file
  
- [ ] **Code Signing**
  - [ ] Sign Windows executables
  - [ ] Sign installer packages
  - [ ] Document signing process

---

## ✅ Completed Items

### Backend Implementation
- [x] TASK-101: Schema migration v4 → v5
- [x] TASK-102: Bundle manifest + persistence
- [x] TASK-103: Bundle versioning (immutable published versions)
- [x] TASK-104: .dfpkg v2 import/export
- [x] TASK-105: Output configuration
- [x] TASK-106: Canonical field schema (13 types)
- [x] TASK-107: Field groups (shared vs document-specific)
- [x] TASK-108: Explicit mapping layer
- [x] TASK-109: Placeholder extraction
- [x] TASK-110: Matter CRUD
- [x] TASK-111: Matter data (single source)
- [x] TASK-112: Grouped form assembly
- [x] TASK-113: Three-level validation
- [x] TASK-114: Rules DSL parser
- [x] TASK-115: Conditional documents + preview
- [x] TASK-116: Generation run records
- [x] TASK-117: Generate all/selected orchestration
- [x] TASK-118: v1 template flow regression tests
- [x] TASK-121: Mail-merge workflow realignment

### Testing
- [x] 31/31 pytest tests passing
- [x] Schema v5 verified
- [x] Foreign key constraints enabled
- [x] 50-fixture fidelity gate passing

### Security
- [x] CSP strict mode verified
- [x] DOMPurify sanitization active
- [x] DPAPI encryption enabled
- [x] Input validation (DOCX magic bytes, zip structure)
- [x] Path traversal guards
- [x] SQL injection prevention (parameterized queries)

### Code Quality
- [x] Rust warnings fixed (10 in commands.rs)
- [x] AC-040 code review (no docx manipulation in src/)
- [x] Layered architecture compliance verified
- [x] Professional neutrality maintained (no hard-coded legal terms)

---

## Pre-Release Command Checklist

Run these commands before creating the release candidate:

```bash
# 1. Apply DOMPurify security update
cd d:\sdlc_studio\projects\docsforge
npm install

# 2. Verify frontend builds cleanly
npm run build
# Expected: No errors, dist/ folder created

# 3. Verify Rust builds without warnings
cd src-tauri
cargo build --release
# Expected: "Finished `release` profile [optimized] target(s)"

# 4. Run clippy for additional warnings
cargo clippy --workspace --all-targets --all-features
# Expected: No warnings

# 5. Run all tests
cd ..
python -m pytest tests/ -v
# Expected: 31 passed

# 6. Run Rust unit tests
cd src-tauri
cargo test --workspace
# Expected: All tests pass

# 7. Build Tauri app with embedded frontend
cd ..
npm run build:tauri
# Expected: Bundles created in src-tauri/target/release/bundle/

# 8. Test installer on clean VM
# Manual: Install MSI on Windows 10/11 VM
# Verify: App launches, no errors, all features work

# 9. Security audit
npm audit --production
cargo audit
# Expected: No high/critical vulnerabilities

# 10. Generate SBOM
# Tool: cargo-sbom or similar
# Output: SBOM.json for compliance
```

---

## Release Candidate Acceptance Criteria

Before promoting to RC:

- [ ] All 🔴 **Critical Blockers** completed
- [ ] All 🟡 **High Priority** items completed
- [ ] All automated tests passing (pytest + cargo test + vitest)
- [ ] Manual smoke test on clean Windows VM successful
- [ ] Security scan shows no high/critical issues
- [ ] Documentation updated for v2 features
- [ ] CHANGELOG.md complete
- [ ] Installers built and tested
- [ ] Code signing completed (if applicable)

---

## Release Process

Once checklist complete:

1. **Create Release Branch**
   ```bash
   git checkout -b release/v2.0.0
   git push -u origin release/v2.0.0
   ```

2. **Bump Version**
   - Update `package.json` version
   - Update `src-tauri/Cargo.toml` version
   - Update `src-tauri/tauri.conf.json` version

3. **Generate Release Notes**
   - Compile CHANGELOG.md entries
   - Highlight breaking changes
   - Document migration path from v1

4. **Build Release Artifacts**
   ```bash
   npm run build:tauri
   ```
   - MSI installer
   - NSIS installer
   - MSIX package (if signed)
   - Portable ZIP

5. **Tag Release**
   ```bash
   git tag -a v2.0.0 -m "Release v2.0.0: Bundle + Matter Domain Model"
   git push origin v2.0.0
   ```

6. **Create GitHub Release**
   - Upload installers
   - Attach SBOM
   - Include CHANGELOG
   - Add migration guide link

7. **Post-Release**
   - Monitor for critical issues (first 48 hours)
   - Update documentation site
   - Announce on communication channels

---

## Estimated Timeline

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| **TASK-119 Implementation** | 1-2 weeks | None (ready to start) |
| **contract_v2.py Tests** | 2-3 days | TASK-119 UI (for e2e) |
| **Documentation Updates** | 1 week | TASK-119 (screenshots) |
| **QA & Testing** | 3-5 days | All code complete |
| **Release Candidate** | 1-2 days | QA pass |
| **Production Release** | 1 day | RC validated |

**Total Estimated Time to Release:** 3-4 weeks from audit completion

---

## Contact & Escalation

**Release Manager:** [To be assigned]  
**Tech Lead:** [To be assigned]  
**QA Lead:** [To be assigned]

**Blocker Escalation:** If any critical blocker cannot be resolved within 2 business days, escalate to Tech Lead for prioritization/scope decision.

---

**Document Version:** 1.0  
**Last Reviewed:** August 28, 2026  
**Next Review:** Upon TASK-119 completion
