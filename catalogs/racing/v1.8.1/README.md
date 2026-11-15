# Racing 1.8.1 — explicit service-control compatibility

The simulation/presentation payloads and indexes are byte-identical to1.8.0.
This new immutable manifest explicitly supports component/V9 model0.11.0
(service control) and timeline0.14.0 (existing signed practices). No coefficients,
circuits, sample frequency, instructions or vehicle capabilities are changed.
Old1.8.0/1.7.0 identities are unchanged; LATEST and embedded defaults stay pinned.
The service policy and exact runtime remain separately signed.
