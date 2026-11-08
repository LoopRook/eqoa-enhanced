# EQOA Example Hearts

Enable **EQOA Example Hearts** in the Addon Manager after logging in. It draws
ten hearts using only `eqoa/1`, and temporarily claims the native health bar.
Disable it to restore the bar. It waits for real health data and handles zero HP.

The entry runs once per enable. `render` draws; lifecycle events release claims.
The manager saves and loads the small position/size settings table in the active
mod profile. No Forge modules or external textures are required.
