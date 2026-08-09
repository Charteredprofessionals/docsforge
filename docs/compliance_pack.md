# DocForge Enterprise Compliance Pack

> Product: DocForge | Version: 1.0.0 | Vendor: DocForge, Inc.

## 1. Compliance Architecture Overview
DocForge is designed from the ground up as an offline-first, deterministic document automation platform.
Document contents never leave the user's workstation or customer-managed infrastructure.

## 2. GDPR Compliance
- **Data Residency:** 100% Local / On-Prem. No cloud processing.
- **Data Minimization:** No PII or document text is included in telemetry.
- **Right to Erasure:** Deleting local application data removes all local records and template storage.

## 3. Security Controls
- **At-Rest Encryption:** Windows DPAPI protected local storage.
- **Tamper-Evident Binaries:** EV Code Signed Windows MSIX/MSI/EXE packages.
- **Zero-Knowledge Licensing:** Air-gapped offline license activation with file signature verification.
- **Append-Only Audit Ledger:** SQLite triggers block UPDATE and DELETE queries on `generation_log`.
