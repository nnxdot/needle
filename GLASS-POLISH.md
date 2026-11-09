# Glass polish: all apps

Written 2026-11-09 from the code on main (2.0.4, d2bc73e5). Facts only. The user picks what to build.

## Windows and Linux (GPUI, crates/needle/src/ui)

How it is now:
- Window material: Mica, Acrylic, Clear or Solid (`glass.rs`). Windows 10 gets Acrylic for Mica.
- Linux: no glass. `appearance.rs` `glass_chosen()` is only true on Windows and macOS.
- Glass only thins the back layer: the title bar and the sidebar (`theme.rs` `Palette::glass`, reach 0.55 to 0.8).
- The page is a solid sheet with one round corner. It touches the window's edges (`mod.rs` around line 3620).
- The floating player and the side panel use `player2::frosted`: 95% solid, a 1 px line, a shadow. No edge light.
- Menus, the palette, dialogs and the What's new pane use `glass::pane`. Off the Mac, this is a solid `popover` fill.
- Tooltips, dropdown lists and the toolkit's own dialogs use the theme's `popover`. That is solid off the Mac (`theme.rs` line 556).
- The Ambient look turns glass off (`ambient.rs` `material()`).
- GPUI cannot blur Needle's own content. Only the desktop behind the window gets blur (from Windows, or from KDE).

Changes:
1. The sidebar floats on glass: inset 8 px, corners 20 px, a soft shadow. This is the same as the Mac (`chrome.rs` sidebar).
2. The cover glow: Needle paints the playing cover, blurred, behind its own back layer, as the Mac's `glass_backdrop` does. This also gives Linux a glass look without system blur.
3. The floating player and the side panel become tinted glass, not 95% grey (`player2::frosted`, `panel.rs` line 57). The cover colour shows from the left.
4. Menus, dialogs and panes become tinted glass panes off the Mac too (`glass::pane`, about 20 places).
5. The edge light: a bright 1 px line on the top edge and a faint outline. Put it in one helper, so all parts match.
6. The page floats as a sheet: inset on every side, corners 14 px.
7. Linux gets glass: system blur on KDE with Wayland (the patched GPUI already has `org_kde_kwin_blur`). Other desktops get the cover glow with tinted panes.
8. Tooltips, dropdown lists and the toolkit's dialogs become tinted glass too (the theme's `popover` off the Mac).
9. A soft scroll edge: the page fades out under the title bar and under the floating player. Today it is a hard cut.
10. One set of corner sizes. Today panes use 6, 8, 9, 10, 12, 14, 16 and 18 px (and 10 to 26 px on glass). Use three sizes.
11. Light mode: the edge light and the tint need their own values. Light glass washes out text sooner.
12. Settings › Appearance: change the Glass amount slider and the "page too" switch so they fit the new look.
13. Ambient and glass: today Ambient turns glass off. The cover glow (2) is close to Ambient, so the two could become one.
14. Selected and hovered rows on glass: use a light capsule (as the Mac sidebar does), not the solid accent fill.

Limit: over the page, the player and menus must stay about 80% solid. GPUI cannot blur what is under them.

## Android (apps/android, Compose, Haze 2.0.1)

How it is now:
- `Modifier.glass` in `ui/Dock.kt`: real blur (Haze) on Android 12 and newer, with a solid fill before that.
- Only two parts use it: the phone's dock and the tablet's player bar (`Dock.kt` lines 99 and 146).

Remaining polish:
- A1. The tablet side pane (lyrics and queue, `Player.kt` `SidePane`) is a solid cover-coloured card, not glass.
- A2. Sheets (17 `ModalBottomSheet`), menus (6 `DropdownMenu`) and dialogs (18 `AlertDialog`, 12 `Dialog`) use plain Material fills.
- A3. The side bar on tablets and TVs (`SideBar.kt`) is not glass.
- A4. No scroll edge at the top: when a page scrolls, its content goes under the status bar with no glass or fade.
- A5. The edge is a flat 0.5 dp line. It has no top light like the desktop's (5).
- A6. The glass tint is the page colour (`surfaceContainer`). It does not take the cover colour as the desktop's will (2, 3).
- A7. Android 8 to 11 get no blur. That is a system limit. Keep the solid fill.

## iPhone (apps/ios, SwiftUI, iOS 26)

How it is now:
- It uses Apple's own Liquid Glass: the tab bar, the mini player as `tabViewBottomAccessory`, the tab bar that shrinks on scroll, 34 glass buttons, and 13 `glassEffect` panes.

Remaining polish:
- I1. Some chips and cards use plain white fills, not glass: `Search.swift` lines 135 and 223, `PlayerTools.swift` lines 51 and 74, `Player.swift` line 1342.
- I2. The soft scroll edge is set on only two pages (`Pages.swift` lines 500 and 659). Other pages use the default.
- I3. `GlassEffectContainer` is used only twice, and `glassEffectID` is not used. Groups of glass buttons (player controls, Play and Shuffle) do not merge or morph.
- I4. Only one glass pane is tinted (`.regular.tint`). The others do not take the cover colour.
- I5. `Discover.swift` line 210: the cards use a solid card colour.
