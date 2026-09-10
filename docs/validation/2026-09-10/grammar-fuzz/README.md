# External DTD grammar fuzz inputs

A marked `value_family` payload selects twelve bounded external-DTD grammar templates. They cover external references around entity values and identifiers, between ATTLIST attributes and enumeration members, and before ELEMENT/NOTATION completion. Seventy seeds combine these templates with handler changes, skipped/read/retained children, allocation failure, parent lifetime, UTF-16 and custom converter controls.

The target SHA256 is `ccc605c3f0ae465a65e3cb1ce8151c646dd022d071a40516498545c79b14dbb3`. All 32 existing control bytes retain their meaning for unmarked inputs. None of 89 earlier named seeds, 1,385 evolved inputs or 60 earlier initial inputs contains the new marker. The existing fixed parser slots, recursion/request/feed bounds, selected-allocator assertions and encoding-release checks remain active. Content model callbacks inspect a bounded number of nodes and free the owned model before changing parser state.

Separate source reviews and strict Clippy pass. The archive retains the exact patch, baseline, generator, seed hashes and review notes. Sustained sanitizer results belong to the subsequent frozen combined campaign; this layer does not claim that the new seeds have already passed sanitizer replay.

The archive also retains 1,824 combined foreign-DTD integration observations against library `ff5d86621114e464a0b20f108b6308fe155d9e8c87bd764bc00872cf124750f8`. Parent/child outcomes and non-Default semantic events match Expat, including its shared read-marker behavior when the final nested external reference is declined. The reports retain existing root closing-bracket Default ordering differences.
