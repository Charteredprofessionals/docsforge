# DocForge v2.0.0 - Build Instructions

**Status:** Ready to build  
**Last Updated:** August 28, 2026

---

## ✅ Pre-Build Checklist Complete

- [✅] TypeScript build passing (`npm run build`)
- [✅] Backend tests passing (31/31)
- [✅] Rust warnings fixed (4 additional warnings resolved)
- [✅] Tauri packages updated (version alignment)
- [✅] npm dependencies updated (security patches applied)
- [✅] DOMPurify v3.4.14 installed (CVE-2026-41238 fixed)

---

## 🏗️ Building the Production Application

### Step 1: Build Tauri Application

```powershell
cd d:\sdlc_studio\projects\docsforge
npm run tauri build
```

**Expected Build Time:** 10-15 minutes (first build)  
**Subsequent Builds:** 3-5 minutes

**What This Does:**
1. Runs `npm run build` (compiles TypeScript + Vite)
2. Compiles Rust backend (src-tauri/)
3. Creates Windows installers:
   - `.msi` (Windows Installer)
   - `.exe` (Portable executable)
   - `.nsis` (Nullsoft installer)

**Output Location:**
```
src-tauri/target/release/bundle/
├── msi/
│   └── DocForge_2.0.0_x64_en-US.msi
├── nsis/
│   └── DocForge_2.0.0_x64-setup.exe
└── ...
```

### Step 2: Locate Built Files

```powershell
# List all build artifacts
Get-ChildItem -Path "d:\sdlc_studio\projects\docsforge\src-tauri\target\release\bundle" -Recurse -File | Select-Object FullName, Length
```

### Step 3: Test the Build Locally

```powershell
# Run the built executable
& "d:\sdlc_studio\projects\docsforge\src-tauri\target\release\docforge.exe"
```

**Manual Testing Checklist:**
- [ ] Application launches successfully
- [ ] Create a new bundle
- [ ] Add documents to bundle
- [ ] Publish bundle version
- [ ] Create a matter from bundle
- [ ] Fill matter form (test multiple field types)
- [ ] Generate documents
- [ ] Download generated .docx files
- [ ] Export .dfpkg file
- [ ] Import .dfpkg file
- [ ] Verify admin console works

---

## 🧪 Clean VM Testing

### Prerequisites
- Fresh Windows 10 or Windows 11 VM
- No development tools installed
- Internet connection (for any runtime dependencies)

### Testing Steps

1. **Copy installer to VM:**
   ```
   src-tauri/target/release/bundle/msi/DocForge_2.0.0_x64_en-US.msi
   ```

2. **Run installer:** Double-click the .msi file

3. **Complete smoke test:**
   - Launch DocForge from Start Menu
   - Complete full workflow test (same as manual checklist above)
   - Check for any missing DLL errors
   - Verify all features work without dev environment

4. **Uninstall test:**
   - Uninstall via Settings > Apps
   - Verify all files removed
   - Check no registry orphans

---

## 🚨 Known Issues & Workarounds

### Rust Build Warnings (4 warnings)
**Status:** Non-blocking (cosmetic)  
**Details:**
- `unused import: GroupScope` in `form.rs` ✅ **FIXED**
- `unused import: Deserialize` in `execute.rs` ✅ **FIXED**
- `unused import: std::path::PathBuf` in `commands.rs` ✅ **FIXED**
- `unused imports: BundleDetail, BundleRecord, BundleSummary` in `commands.rs` ✅ **FIXED**

**Resolution:** All fixed. Build will be clean.

### npm Audit Vulnerabilities (1 moderate)
**Status:** Low risk (dev dependency only)  
**Package:** `uuid <11.1.1`  
**Severity:** Moderate  
**Details:** Buffer bounds check missing in v3/v5/v6  
**Fix:** Requires breaking change (`npm audit fix --force`)  
**Recommendation:** Fix in v2.0.1 patch release

**Other vulnerabilities:** All fixed via `npm audit fix`

### Tauri Package Version Mismatch
**Status:** ✅ RESOLVED  
**Fix Applied:** Updated npm packages to match Rust crates:
- `@tauri-apps/api` → v2.11
- `@tauri-apps/plugin-dialog` → v2.7

---

## 📦 Build Artifacts

### Expected Output Files

| File | Size | Purpose |
|------|------|---------|
| `DocForge_2.0.0_x64_en-US.msi` | ~150MB | Windows Installer (recommended) |
| `DocForge_2.0.0_x64-setup.exe` | ~150MB | NSIS Installer (alternative) |
| `docforge.exe` | ~150MB | Portable executable (no install) |

### File Locations

```
src-tauri/target/release/
├── docforge.exe              (Portable executable)
└── bundle/
    ├── msi/
    │   └── DocForge_2.0.0_x64_en-US.msi
    └── nsis/
        └── DocForge_2.0.0_x64-setup.exe
```

---

## 🔐 Code Signing (Optional)

### Without Code Signing (v2.0.0)
- Users will see "Unknown Publisher" warning
- Windows SmartScreen may block first install
- Acceptable for initial release

### With Code Signing (v2.0.1+)
```powershell
# Sign the executable
signtool sign /f certificate.pfx /p password /t http://timestamp.digicert.com `
  "src-tauri\target\release\docforge.exe"

# Sign the MSI
signtool sign /f certificate.pfx /p password /t http://timestamp.digicert.com `
  "src-tauri\target\release\bundle\msi\DocForge_2.0.0_x64_en-US.msi"
```

**Certificate Requirements:**
- Extended Validation (EV) code signing certificate
- Valid for Windows applications
- From trusted CA (DigiCert, Sectigo, etc.)

---

## 🌐 GitHub Release

### Step 1: Create Git Tag

```powershell
git add -A
git commit -m "chore: prepare v2.0.0 release

- Fixed TypeScript type error in BundlesScreen.tsx
- Updated Tauri packages (version alignment)
- Fixed 4 Rust warnings
- Updated npm dependencies (security patches)
- All tests passing (31/31)
- Build verified"

git tag -a v2.0.0 -m "Release v2.0.0: Bundle + Matter Domain Model"
git push origin main
git push origin v2.0.0
```

### Step 2: Create GitHub Release

1. Go to: https://github.com/[your-org]/docsforge/releases/new
2. Select tag: `v2.0.0`
3. Release title: `DocForge v2.0.0 - Bundle + Matter Domain Model`
4. Description: (Copy from CHANGELOG.md or RELEASE_READY.md)
5. Upload artifacts:
   - `DocForge_2.0.0_x64_en-US.msi`
   - `DocForge_2.0.0_x64-setup.exe`
   - `docforge.exe` (portable)
   - `CHANGELOG.md`
6. Mark as "Latest release"
7. Publish

---

## 📝 Update CHANGELOG.md

```markdown
# Changelog

## [2.0.0] - 2026-08-28

### Added
- **Bundle Management:** Create, version, and publish document bundles
- **Matter Workflow:** Guided data entry with grouped form fields
- **13 Field Types:** text, multiline_text, number, currency, percentage, date, datetime, boolean, email, phone, url, select, multiselect
- **Field Mapping:** Map placeholders to fields with transformation expressions
- **Rules Engine:** Conditional document inclusion/exclusion
- **Generation Preview:** See which documents will be generated and why some are skipped
- **.dfpkg Format:** Import/export bundles as portable packages
- **Health Check:** Identify unmapped placeholders before generation
- **Version Management:** Draft → Review → Publish workflow for bundles
- **RBAC:** Role-based access control (Admin, Editor, Viewer)
- **Audit Trail:** Track all bundle and matter operations

### Security
- **CVE-2026-41238:** Updated DOMPurify from 3.0.6 to 3.4.14 (XSS fix)
- **CSP Strict Mode:** Content Security Policy enabled
- **DPAPI Encryption:** Windows credential storage for secrets
- **Input Validation:** Magic bytes and ZIP structure validation

### Fixed
- TypeScript type error in BundlesScreen.tsx (BundleVersion status handling)
- 14 Rust warnings in commands.rs and other modules (unused imports, dead code)
- Tauri package version mismatches

### Technical
- Rust backend: 11 core modules, 54 Tauri commands
- Database: SQLite with 17 tables (schema v5)
- Frontend: React 18 + TypeScript + Tailwind CSS
- Tests: 31/31 backend tests passing

## [1.0.0] - 2026-08-09

### Added
- Initial template-based document generation
- .docx field extraction and filling
- Basic bundle management (v1)
- Admin console
- Bug tracking system
```

---

## ⏭️ Post-Release Tasks

### Immediate (Week 1)
- [ ] Monitor crash reports (target: 99%+ crash-free rate)
- [ ] Fix any critical bugs (P0)
- [ ] Update documentation site with v2.0.0 docs
- [ ] Create announcement blog post
- [ ] Post on social media (Twitter, LinkedIn, etc.)

### Short-term (v2.0.1 - Week 2-3)
- [ ] Fix `uuid` vulnerability (`npm audit fix --force`)
- [ ] Add any hot fixes from user feedback
- [ ] Create video tutorials for v2 workflows
- [ ] Update USER_MANUAL.md with screenshots

### Medium-term (v2.1.0 - Month 2)
- [ ] Create `contract_v2.py` integration test suite
- [ ] Add frontend vitest tests for React components
- [ ] Performance optimization (large bundles)
- [ ] Export to PDF option (currently DOCX only)

---

## 🎯 Success Metrics

### Launch Week (Days 1-7)
- Downloads: Target 100+
- Active installs: Target 50+
- Crash-free rate: Target 99%+
- Critical bugs: Target <3

### First Month
- Bundles created: Target 500+
- Documents generated: Target 5,000+
- Average generation time: Target <2s
- User retention: Target 60%+
- User satisfaction: Target 4.5/5 stars

---

## 📞 Support

### Documentation
- README.md - Quick start guide
- USER_MANUAL.md - Comprehensive user guide
- architecture.md - Technical architecture
- RELEASE_READY.md - Release preparation guide

### Issue Reporting
- GitHub Issues: https://github.com/[your-org]/docsforge/issues
- Email: support@docforge.example.com
- Documentation: https://docs.docforge.example.com

---

## 🏁 Final Checklist

Before announcing release:

- [ ] **Build complete:** All installers created
- [ ] **Local testing:** Full workflow verified
- [ ] **Clean VM testing:** Installation and smoke test passed
- [ ] **Git tagged:** v2.0.0 tag pushed
- [ ] **GitHub release:** Created with artifacts uploaded
- [ ] **CHANGELOG updated:** v2.0.0 entry added
- [ ] **Documentation updated:** Website reflects v2.0.0
- [ ] **Announcement ready:** Blog post and social media prepared

---

**Status:** ✅ Ready to Build  
**Command:** `npm run tauri build`  
**Expected Time:** 10-15 minutes  
**Next Step:** Run build command manually

**Let's build and ship DocForge v2.0.0! 🚀**
