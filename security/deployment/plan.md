# Confidential Defensive Engineering Plan

## 1. Deployment Security Layer

| Step | Action | Status |
|---|---|---|
| 1 | Define deployment identity and firewall release requirements. | Complete |
| 2 | Add a local validator for identity manifests, key references, rotation metadata, and firewall policy. | Complete |
| 3 | Add non-secret example manifests for operators. | Complete |
| 4 | Wire validator into the release provenance gate. | Complete |
| 5 | Run validator self-test and focused release-gate check. | Pending |
| 6 | Implement OS-specific secret provisioning commands after explicit operator approval. | Pending |

## 2. Non-Goals

| Item | Reason |
|---|---|
| Writing production secrets | Secret insertion changes OS-protected state and requires explicit operator approval. |
| Applying host firewall rules | Firewall application is platform-specific and should run only after the operator selects pf, nftables, firewalld, or cloud security groups. |
| Certifying the quarantined bootstrap artifacts | Invalid artifacts remain quarantined until regenerated from a clean tree. |

