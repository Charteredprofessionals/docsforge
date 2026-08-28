# DocForge v2.0.0 - Next Steps & Implementation Roadmap

**Document Version:** 1.0  
**Last Updated:** August 28, 2026  
**Project Status:** 🔴 Quality Gate - Blocked on TASK-119  
**Target Release:** v2.0.0 (Bundle + Matter Domain Model)

---

## Executive Summary

DocForge v2.0.0 has a **production-ready Rust backend** with comprehensive test coverage (31/31 passing) and proper security controls. The **frontend v2 UI is incomplete**, preventing release. This document outlines the implementation roadmap to unblock the release.

**Current State:**
- ✅ Backend: 100% complete (all 17 v2 tasks verified)
- ❌ Frontend: 30% complete (v1 UI exists, v2 components missing)
- ⚠️ Tests: Backend covered, frontend tests missing
- ✅ Security: All vulnerabilities addressed

**Timeline to Release:** 3-4 weeks (assumes immediate start on TASK-119)

---

## Phase 1: Critical Blockers (Week 1-2)

### TASK-119: v2 UI Implementation
**Priority:** 🔴 P0 - RELEASE BLOCKER  
**Status:** Blocked  
**Owner:** Frontend Developer (TBD)  
**Estimated Effort:** 40-60 hours

#### Week 1: Core Components

##### Day 1-3: MatterForm.tsx (15-20 hours)
**Goal:** Implement grouped data entry form for Matter workflow

**Implementation Steps:**

1. **Setup & API Integration** (3 hours)
   ```typescript
   // src/lib/ipc.ts - Add new commands
   export async function renderMatterForm(matterId: string): Promise<MatterForm> {
     return invoke('render_matter_form_cmd', { matterId });
   }
   
   export async function setMatterValue(matterId: string, fieldId: string, value: any): Promise<void> {
     return invoke('set_matter_value_cmd', { matterId, fieldId, value });
   }
   
   export async function validateMatter(matterId: string): Promise<ValidationReport> {
     return invoke('validate_matter_cmd', { matterId });
   }
   ```

2. **Component Structure** (4 hours)
   ```typescript
   // src/components/MatterForm.tsx
   interface MatterFormProps {
     matterId: string;
     onComplete: () => void;
     onCancel: () => void;
   }
   
   export default function MatterForm({ matterId, onComplete, onCancel }: MatterFormProps) {
     const [form, setForm] = useState<MatterForm | null>(null);
     const [values, setValues] = useState<Record<string, any>>({});
     const [errors, setErrors] = useState<ValidationReport | null>(null);
     const [saving, setSaving] = useState(false);
     
     // Fetch form schema on mount
     // Render grouped sections (shared first, then document-specific)
     // Handle field changes with auto-save debounce
     // Display validation errors inline
     // Submit and navigate to generation preview
   }
   ```

3. **Field Type Support** (6 hours)
   - Implement 13 field types: text, multiline_text, number, currency, percentage, date, datetime, boolean, email, phone, url, select, multiselect
   - Each field type gets its own input component with validation
   - Use React Hook Form or similar for form state management
   
   ```typescript
   // src/components/fields/FieldRenderer.tsx
   export function FieldRenderer({ field, value, onChange, error }: FieldProps) {
     switch (field.type) {
       case 'text': return <TextInput {...props} />;
       case 'multiline_text': return <TextArea {...props} />;
       case 'number': return <NumberInput {...props} />;
       case 'currency': return <CurrencyInput {...props} />;
       case 'date': return <DatePicker {...props} />;
       case 'select': return <Select options={field.options} {...props} />;
       // ... etc
     }
   }
   ```

4. **Grouped Sections UI** (4 hours)
   - Render shared fields in prominent top section
   - Document-specific fields in collapsible sections below
   - Visual distinction (colors, icons, labels)
   - Progress indicator showing completion %
   
5. **Validation & Error Display** (2 hours)
   - Real-time validation on blur
   - Inline error messages per field
   - Summary error panel at top
   - Prevent submit if validation fails

**Acceptance Test:**
```typescript
// tests/components/MatterForm.test.tsx
describe('MatterForm', () => {
  it('renders grouped sections (shared first)', async () => {
    // Test form structure
  });
  
  it('validates required fields', async () => {
    // Test validation
  });
  
  it('saves draft automatically', async () => {
    // Test auto-save
  });
  
  it('submits and navigates to preview', async () => {
    // Test submit flow
  });
});
```

##### Day 4-5: GenerationHistory.tsx (12-15 hours)
**Goal:** Display generation runs with preview and download

**Implementation Steps:**

1. **API Integration** (2 hours)
   ```typescript
   // src/lib/ipc.ts
   export async function listGenerationRuns(matterId: string): Promise<GenerationRun[]> {
     return invoke('list_runs_cmd', { matterId });
   }
   
   export async function evaluatePreview(matterId: string, documentIds?: string[]): Promise<GenerationPreview> {
     return invoke('evaluate_preview_cmd', { matterId, documentIds });
   }
   
   export async function executeRun(matterId: string, documentIds?: string[]): Promise<ExecuteResult> {
     return invoke('execute_run_cmd', { matterId, documentIds });
   }
   ```

2. **Component Structure** (4 hours)
   ```typescript
   // src/components/GenerationHistory.tsx
   export default function GenerationHistory({ matterId }: Props) {
     const [runs, setRuns] = useState<GenerationRun[]>([]);
     const [preview, setPreview] = useState<GenerationPreview | null>(null);
     const [generating, setGenerating] = useState(false);
     
     // Display run history with metadata
     // Show generation preview (count + skipped-with-reason)
     // Execute new run (all / selected documents)
     // Download generated documents (DOCX/PDF)
   }
   ```

3. **Preview Display** (3 hours)
   - Show document count to generate
   - List skipped documents with reasons (rule evaluation)
   - Visual indicators (checkmarks for included, X for skipped)
   - Reason tooltips/popovers
   
4. **Run History Table** (3 hours)
   - Timestamp, bundle version, engine version
   - Status (success/warning/error)
   - Document count generated
   - Click to view/download artifacts
   
5. **Download & Rerun** (3 hours)
   - Bulk download as ZIP
   - Individual document download
   - Rerun button (creates new run with same inputs)
   - Warning if bundle version has changed

**Acceptance Test:**
```typescript
// tests/components/GenerationHistory.test.tsx
describe('GenerationHistory', () => {
  it('displays run history with metadata', async () => {});
  it('shows preview with skipped-document reasons', async () => {});
  it('executes new run', async () => {});
  it('downloads generated documents', async () => {});
});
```

#### Week 2: Bundle UI & Navigation

##### Day 6-8: BundlesScreen.tsx (13-18 hours)
**Goal:** Replace Bundles.tsx with full v2 implementation

**Implementation Steps:**

1. **API Integration** (3 hours)
   ```typescript
   // src/lib/ipc.ts - v2 bundle APIs
   export async function createBundleV2(name: string, description: string): Promise<Bundle> {
     return invoke('create_bundle_v2_cmd', { name, description });
   }
   
   export async function publishVersion(bundleId: string, note: string): Promise<BundleVersion> {
     return invoke('publish_version_cmd', { bundleId, note });
   }
   
   export async function exportBundleDfpkg(bundleId: string, versionId: string): Promise<Uint8Array> {
     return invoke('export_bundle_dfpkg_cmd', { bundleId, versionId });
   }
   
   export async function importBundleDfpkg(dfpkgBytes: Uint8Array): Promise<Bundle> {
     return invoke('import_bundle_dfpkg_cmd', { dfpkgBytes });
   }
   ```

2. **Bundle List View** (4 hours)
   - Card/table view of bundles
   - Status badges (draft/published/archived)
   - Version count indicator
   - Quick actions (publish, export, delete)
   
3. **Bundle Detail View** (4 hours)
   - Document list with field mappings preview
   - Version history timeline
   - Health check panel (unmapped placeholders, validation errors)
   - Field schema summary
   
4. **Version Management UI** (3 hours)
   - Draft → Review → Publish workflow
   - Version comparison view
   - Publish confirmation dialog with notes
   - Immutability warning (published versions can't be edited)
   
5. **.dfpkg Import/Export** (4 hours)
   - Export button with file dialog
   - Import drag-and-drop or file picker
   - Progress indicators for large bundles
   - Validation error display

**Acceptance Test:**
```typescript
// tests/components/BundlesScreen.test.tsx
describe('BundlesScreen', () => {
  it('lists bundles with status', async () => {});
  it('publishes bundle version', async () => {});
  it('exports .dfpkg package', async () => {});
  it('imports .dfpkg package', async () => {});
  it('shows health check warnings', async () => {});
});
```

##### Day 9-10: App.tsx Navigation Refactor (5-7 hours)
**Goal:** Implement v2 navigation: Dashboard / Bundles / Matters / Generated Documents

**Implementation Steps:**

1. **Routing Setup** (2 hours)
   ```typescript
   // src/App.tsx
   type AppView = 
     | 'dashboard'
     | 'bundles'
     | 'bundle-detail'
     | 'matters'
     | 'matter-form'
     | 'generation-history'
     | 'admin';
   
   // Implement view routing with state management
   // Add breadcrumb navigation
   // Handle deep linking (bundle → matter → generation)
   ```

2. **Dashboard View** (2 hours)
   - Quick stats (total bundles, matters, recent generations)
   - Recent activity feed
   - Quick action cards (New Bundle, New Matter)
   
3. **Navigation Bar Update** (2 hours)
   - Replace old nav: Templates / New Template / Bundles / Admin
   - New nav: Dashboard / Bundles / Matters / Generated Documents / Admin
   - Active state indicators
   - Mobile-responsive menu
   
4. **Breadcrumbs** (1 hour)
   - Home > Bundles > [Bundle Name] > Matter > [Matter Name] > Generation
   - Click to navigate up hierarchy
   - Consistent across all views

**Acceptance Test:**
```typescript
// tests/App.test.tsx
describe('App Navigation', () => {
  it('shows v2 navigation menu', async () => {});
  it('navigates between views', async () => {});
  it('displays breadcrumbs correctly', async () => {});
});
```

---

## Phase 2: Testing & Quality (Week 3)

### contract_v2.py Test Suite
**Priority:** 🔴 P0 - RELEASE BLOCKER  
**Status:** Not Started  
**Owner:** QA Engineer (TBD)  
**Estimated Effort:** 8-12 hours

#### Day 11-12: Backend Contract Tests (8-12 hours)

**Test Structure:**
```python
# tests/contract_v2.py
import pytest
from pathlib import Path

class TestBundleContract:
    def test_create_bundle_with_documents(self):
        """Create bundle, add documents, verify persistence"""
        pass
    
    def test_publish_bundle_immutability(self):
        """Publish bundle, verify version immutable, edits create new version"""
        pass
    
    def test_dfpkg_roundtrip(self):
        """Export bundle as .dfpkg, import, verify equality"""
        pass

class TestFieldMappingContract:
    def test_all_13_field_types(self):
        """Create fields of all 13 types, validate each"""
        pass
    
    def test_set_mapping_and_resolve(self):
        """Map placeholder to field, resolve value from matter data"""
        pass

class TestMatterContract:
    def test_create_matter_binds_version(self):
        """Create matter, verify bundle_version_id immutable"""
        pass
    
    def test_single_data_source_multiple_docs(self):
        """Set matter data once, verify used across all bundle documents"""
        pass
    
    def test_three_level_validation(self):
        """Test field-level, matter-level, bundle-level validation"""
        pass

class TestRulesContract:
    def test_conditional_document_evaluation(self):
        """Add rule, evaluate, verify Include/Exclude with reason"""
        pass
    
    def test_invalid_expression_rejected(self):
        """Submit invalid rule expression, verify parse error"""
        pass

class TestGenerationContract:
    def test_execute_run_all_documents(self):
        """Generate all documents, verify output naming, immutability"""
        pass
    
    def test_execute_run_selected_documents(self):
        """Generate subset of documents, verify correct selection"""
        pass
    
    def test_run_immutability(self):
        """Verify historical runs and documents never mutated"""
        pass
```

**Acceptance Criteria:**
- All contract tests passing
- Coverage of Bundle, Field Mapping, Matter, Rules, Generation APIs
- Integration with existing pytest suite

### Frontend Tests
**Priority:** 🟡 High  
**Status:** Not Started  
**Estimated Effort:** 8-12 hours

#### Day 13: Component Tests (8-12 hours)

```typescript
// tests/components/*.test.tsx
// Unit tests for MatterForm, GenerationHistory, BundlesScreen
// Integration tests for navigation flows
// Snapshot tests for UI consistency
```

**Tools:**
- Vitest for test runner
- React Testing Library for component testing
- MSW for API mocking

---

## Phase 3: Documentation & Polish (Week 3-4)

### Documentation Updates
**Priority:** 🟡 High  
**Status:** Not Started  
**Estimated Effort:** 16-20 hours

#### Day 14-15: User Manual (8-10 hours)

**Updates Required:**
1. **docs/architecture/USER_MANUAL.md**
   - Add v2 workflow section
   - Bundle creation guide with screenshots
   - Matter workflow step-by-step
   - Generation preview and history
   - Migration guide from v1 templates

2. **Tutorial Videos / GIFs**
   - Screen recordings of key workflows
   - Annotated screenshots
   - Quick start guide

#### Day 16: Developer Documentation (4-5 hours)

**Updates Required:**
1. **architecture.md** - Verify current, update if needed
2. **API.md** - Document v2 Tauri commands
3. **CONTRIBUTING.md** - Update for v2 codebase

#### Day 17: Release Documentation (4-5 hours)

**Documents to Create:**
1. **CHANGELOG.md** - Full v2 change list
2. **MIGRATION.md** - v1 to v2 upgrade guide
3. **KNOWN_ISSUES.md** - Document any known limitations

---

## Phase 4: QA & Release (Week 4)

### Manual QA Testing
**Priority:** 🟡 High  
**Status:** Not Started  
**Estimated Effort:** 16-20 hours

#### Day 18-19: Full Regression (8-10 hours)

**Test Scenarios:**
1. **Clean Install Test**
   - Install on fresh Windows 10/11 VM
   - Verify app launches without errors
   - Test offline activation (if applicable)

2. **v1 Template Flow** (Regression)
   - Create standalone template
   - Fill template
   - Export DOCX/PDF
   - Verify 50-fixture fidelity gate

3. **v2 Bundle Flow** (New Feature)
   - Create bundle with 5 documents
   - Define canonical fields (all 13 types)
   - Set mappings
   - Add conditional rules
   - Publish bundle version
   - Create matter
   - Fill matter form
   - Preview generation
   - Generate all documents
   - Verify output naming
   - Download DOCX/PDF

4. **Edge Cases**
   - Large bundle (100+ documents)
   - Complex rules (nested conditions)
   - All field types with validation
   - .dfpkg import/export roundtrip
   - Version rollback

#### Day 20: Performance & Security (4-5 hours)

**Performance Tests:**
- Large document generation (10MB+ DOCX)
- 1000+ matters in database
- Bundle with 100+ documents
- Measure memory usage, response times

**Security Tests:**
- Verify DOMPurify v3.4.14 active
- Test CSP strictness
- DOCX validation (malformed files)
- Path traversal attempts
- SQL injection attempts (should be impossible with parameterized queries)

#### Day 21: Release Candidate Build (4-5 hours)

**Build Process:**
1. Version bump (package.json, Cargo.toml, tauri.conf.json)
2. Update CHANGELOG.md with final entries
3. Run full test suite (pytest + cargo test + vitest)
4. Build Tauri app: `npm run build:tauri`
5. Sign binaries (if certificate available)
6. Create installers (MSI, NSIS, MSIX)
7. Test installers on clean VM
8. Generate SBOM
9. Create GitHub release draft
10. Tag release: `git tag -a v2.0.0 -m "Release v2.0.0"`

---

## Resource Allocation

### Required Team

| Role | Allocation | Duration | Key Responsibilities |
|------|-----------|----------|---------------------|
| **Frontend Developer** | Full-time | 2 weeks | TASK-119 implementation |
| **QA Engineer** | Half-time | 1 week | contract_v2.py, manual testing |
| **Technical Writer** | Part-time | 1 week | Documentation updates |
| **DevOps Engineer** | Part-time | 2 days | Build pipeline, release automation |
| **Project Manager** | Part-time | 4 weeks | Coordination, timeline tracking |

### Tools & Infrastructure

- **Development:**
  - VS Code with Rust Analyzer, ESLint, Prettier
  - Windows 10/11 development machines
  - Git for version control

- **Testing:**
  - Clean Windows VMs for testing (VirtualBox/Hyper-V)
  - pytest, cargo test, vitest test runners
  - Manual testing checklist

- **CI/CD:**
  - GitHub Actions (or equivalent)
  - Automated testing on push
  - Security scanning (npm audit, cargo audit)

---

## Risk Management

### High-Risk Items

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **TASK-119 takes longer than estimated** | Medium | High | Break into smaller PRs, parallelize work where possible |
| **Integration issues between UI and backend** | Low | Medium | Backend is well-tested; use contract tests early |
| **Performance issues with large bundles** | Low | Medium | Performance testing in QA phase |
| **Installer signing certificate unavailable** | Medium | Low | Can release unsigned initially, sign in patch |

### Contingency Plans

**If timeline slips beyond 4 weeks:**
1. **Option A:** Release v2.0.0-beta with known UI limitations
2. **Option B:** Reduce scope (drop non-critical features like .dfpkg import)
3. **Option C:** Extend timeline by 1-2 weeks (preferred if quality at risk)

---

## Success Metrics

### Release Readiness Criteria

- [ ] All 🔴 critical blockers resolved
- [ ] TASK-119 complete (all 4 components implemented)
- [ ] contract_v2.py test suite passing
- [ ] All frontend tests passing (unit + integration)
- [ ] Manual QA checklist 100% complete
- [ ] Documentation updated
- [ ] Release candidate tested on clean VM
- [ ] No high/critical security vulnerabilities
- [ ] Installers built and tested

### Post-Release Metrics (Track for 2 weeks)

- **Adoption:** Number of v2 bundles created
- **Stability:** Crash-free rate (target: 99%+)
- **Performance:** Average document generation time
- **Support:** Number of critical issues reported
- **Satisfaction:** User feedback scores

---

## Communication Plan

### Status Updates

**Daily Standups (During Implementation):**
- What was completed yesterday
- What will be completed today
- Any blockers

**Weekly Status Report:**
- Progress against timeline
- Updated risk assessment
- Decisions needed

### Stakeholder Communication

**Weekly Email Update:**
- Sent to: Project Sponsor, Product Owner, Tech Lead
- Content: Progress %, blockers, ETA

**Release Announcement:**
- Internal announcement 1 week before release
- External announcement on release day
- Include: What's new, migration guide link, support channels

---

## Appendix A: Quick Start Checklist

For the developer starting TASK-119 today:

### Day 1 Setup
- [ ] Clone repository: `git clone <repo>`
- [ ] Install dependencies: `npm install`
- [ ] Verify backend builds: `cd src-tauri && cargo build --release`
- [ ] Verify frontend builds: `npm run build`
- [ ] Run tests: `python -m pytest tests/ -v`
- [ ] Read architecture.md and AUDIT_REPORT.md
- [ ] Review existing Tauri commands in src-tauri/src/commands.rs
- [ ] Familiarize with React components in src/components/

### Day 1 First PR
- [ ] Create feature branch: `git checkout -b feature/matter-form`
- [ ] Add MatterForm.tsx stub
- [ ] Wire up basic API calls (renderMatterForm, setMatterValue)
- [ ] Render form with mock data
- [ ] Create PR for review

---

## Appendix B: Useful Commands

```bash
# Development
npm run dev                 # Start Vite dev server
npm run tauri dev           # Start Tauri in dev mode
cd src-tauri && cargo build # Build Rust backend

# Testing
python -m pytest tests/ -v  # Run pytest suite
cd src-tauri && cargo test  # Run Rust unit tests
npm run test                # Run vitest (once configured)

# Linting
cargo clippy --workspace    # Rust linter
npm run lint                # ESLint

# Production Build
npm run build:tauri         # Full Tauri build with embedded UI

# Debugging
cd src-tauri && cargo run   # Run with console output
npm run tauri -- --debug    # Tauri debug mode
```

---

## Contact & Support

**Project Lead:** [TBD]  
**Technical Questions:** Review AUDIT_REPORT.md, RELEASE_CHECKLIST.md  
**Blocker Escalation:** Report in daily standup or via project tracker

**Key Documents:**
- `AUDIT_REPORT.md` - Comprehensive audit findings
- `RELEASE_CHECKLIST.md` - Pre-release task list
- `architecture.md` - System architecture v2
- `requirements.md` - Functional requirements
- `acceptance_criteria.json` - 40 acceptance criteria

---

**Document Version:** 1.0  
**Created:** August 28, 2026  
**Next Review:** Weekly during implementation phase  
**Status:** Ready for execution - awaiting team assignment
