# EQOA Creature Journal

Enabled by default. Records creature discoveries in the FrontiersForge journal
format. Saves after 30 seconds without changes, every 120 seconds while changing,
and on disable. Import/export also work from its own Mods & Addons option rows.
The private journal is shared read-only with enabled map-search mods.

Ported from EQOA Enhanced `map/journal.rs`; AGPL-3.0-or-later. Import cap: 50,000
valid entries; total: 80,000. Native file API bounds loads to 16 MiB and saves to
14 MiB. A malformed existing journal is preserved until the player repairs it.
