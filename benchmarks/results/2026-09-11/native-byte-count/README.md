# Native source byte counts

Inlining native byte counts reduces native project time by 3.1% and actual CPython time by 1.5% against PR #115. Generated controls improve by 4.0%. Oriole still takes 1.59× Expat's native time and 1.27× its CPython time. One CPython condition regresses by 0.59%; all native conditions improve against the parent.

See the [complete report](../../../../docs/validation/2026-09-11/native-byte-count/) for all conditions, raw measurements, source/compiler/library identities, compatibility gates and independent reviews. Both Oriole builds use verified C-only ThinLTO without PGO or alternate allocators on the shared host.
