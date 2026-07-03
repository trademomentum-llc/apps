# NINL Simulation Harness — Scope and Initial Definition (Apps Working Copy)

**Document ID:** NEURODIOS-NINL-SIM-001  
**Version:** 0.1  
**Date:** 2026-05-30  
**Related:** Canonical version in engine/nnos/docs/

---

(Full authoritative text is in engine/nnos/docs/2026-05-30-NINL-Simulation-Harness-Scope.md.)

**Primary Target Architecture (confirmed):** Google Pixel 9 Pro class Android edge devices. The simulation harness is being designed from the ground up against realistic constraints of this class of device (Tensor on-device ML, power, memory, thermal, sensor access).

This document registers the simulation harness workstream for visibility in the apps/JStar root. The harness will serve as the main validation environment while parallel sub-agents research non-invasive modalities suitable for mobile and define the Neural Token Protocol.

**End of Scope Definition (Apps Variant)**

---

## 7. v1.0 Modality Stack Integration (Locked 2026-05-30)

**Primary Hardware Stack to Model:**
Dry/Semi-dry EEG (4–14 ch flexible headband, ear-EEG, glasses/tattoo/flex variants) + capacitive/electric-field augmentation, targeted at Google Pixel 9 Pro class constraints.

The Simulation Harness (apps visibility copy) must now treat this specific stack as the concrete signal source for all v1.0 development. See the canonical version in engine/nnos/docs/ for the detailed integration requirements (signal modeling, Efficiency Mandate fixed-point paths, Pixel 9 Pro power/latency/thermal model, differentiated NeuroBalance + Roller Coaster governance for rich outputs, and success criteria).

This is the highest-priority focus for harness work going forward. All related agent activity must emit KDBs.