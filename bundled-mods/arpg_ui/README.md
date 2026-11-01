# arpg_ui

An ARPG style heads up display for EQOA, built on the itsmars art kit in
`resources/`. A UiForge ForgeScript mod. Every element is its own borderless
window, so you can drag each piece anywhere and the layout is saved into your
profile.

## What it draws

- **Health panel**. The `HealthPanel.png` frame holds the liquid **health orb**
  in its circular cutout on the left and the **ten ability slots** as two rows of
  five on the right. The orb fills from the bottom, tinted red, with a glass
  sheen, a dark ring, and a flowing liquid noise inside.
- **Power panel**. The `itsmars_orb_back1.png` frame holds the liquid **power
  orb**, tinted blue, matching the health orb.
- **Experience bar**. Shows your level and true progress to the next level using
  `Util.GetExpRequiredForLevel`, with the current and required experience and the
  percent rendered on the bar itself. Width and height scale independently.
- **Ability tray**. The game's first two hotbars as ten slots. Icons are decoded
  live from game memory. The slot the game will actually cast is highlighted, on
  cooldown slots dim and count down, out of range and not enough power slots dim,
  and the selected ability's name shows in a plate at the top of the panel.
- **Item pad**. The game's third hotbar as four item slots in a touching two by
  two square.
- **Buffs and debuffs**. A row of the active effects, icon plus hover name.
- **Target nameplate**. The `NamePanel.png` plate doubles as the target's health
  bar, filled and colored by disposition, with the name on the plate and the
  level and disposition under a divider.
- **Pet nameplate**. The same plate for your summoned pet, showing its name,
  health, and level.

## Orbs

Both orbs are their own draggable windows, independent of the panel art they sit
over, and both render from one shared base size so equal scale gives equal size.
Drag an orb to line it up inside its panel, then scale it under **Orbs** in the
settings.

The liquid inside each orb flows with a scrolling noise. Under **Orbs** you can
turn the flow on or off and tune its speed and strength. The flow drifts in one
steady direction and is masked to a smooth circle, so it never steps or spills
past the border.

## Fonts

Pick any font from the shared resources fonts folder, set a base size, and set a
per component size for the orbs, ability cooldowns, experience bar, target plate,
pet plate, and selected ability name under **Per component font size**. Per
component sizes only apply with a custom font chosen, since the built in font
cannot be resized.

## Game UI it replaces

By default this mod hides the game's own health, power, and experience bars, its
ability bar, its active effects, its target nameplate, and its pet panel, since
it draws its own. Each one has its own toggle under **Hide the game's own UI**,
and everything is restored when the mod is disabled.

## Rotating the dpad

The ability tray is horizontal, but the game walks its ability bar with up and
down. **Rotate dpad for the horizontal tray**, under **Controls**, turns the dpad
a quarter turn so left and right move along the tray and up and down do what left
and right normally would.

The rotation applies to the whole game, not just the tray, so while it is on your
dpad is turned everywhere gameplay input goes. It drops back to the normal dpad
on its own whenever a dialog, a menu, or the pause screen is open, and it is
removed once nothing is asking for it any more. Only the controller dpad is
affected, keyboard equivalents are left alone.

Because it is one setting for the whole game, other mods can ask for the same
rotation, and the dpad stays turned as long as any of them is asking. Turning
this off here only withdraws this mod's request.

## Layout

Open the UiForge settings panel, untick **Lock HUD position**, drag the pieces
where you want, then lock it again. Positions, sizes, colors, fonts, and which
elements show are all saved into the profile. **Reset layout to defaults** puts
every piece back.

## Resource kit

Orbs: `itsmars_orb_back2/fill/highlight/shadow`, `DarkOrbBorder`,
`itsmars_scroll_fill/lip` (the flowing liquid noise).
Panels: `HealthPanel`, `itsmars_orb_back1`.
Experience: `itsmars_exp_back/fill/highlight/shadow`.
Slots: `52x52 SpellSlotBorder`, `64x64 BagSlotBorder`, `BlankSlot`.
Trim: `Divider`, `NamePanel`, `TooltipBackground`.
