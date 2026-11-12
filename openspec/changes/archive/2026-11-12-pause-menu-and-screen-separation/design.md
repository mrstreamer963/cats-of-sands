# Design

## Context

See `proposal.md` for motivation. The current page has one launch form and one Canvas; the form remains visible after launch. `src/main.js` owns the animation loop and advances a Rust/WASM `Session` every frame. The current `Session` API has no state export/import operation. The game uses YAML and Tiled files to construct a world, with live unit positions and mutable wall state held by the simulation.

## Goals / Non-Goals

**Goals:**
- Keep the existing launch form as the main menu and show only the map and units during play.
- Make pause a distinct application state that blocks commands and does not advance simulation time.
- Export and import complete, portable game snapshots as files.

**Non-Goals:**
- Add a separate title screen, HUD elements, combat, settings, or cloud synchronization.
- Persist transient input state such as an active pointer drag or open browser file picker.

## Decisions

1. **Use explicit UI states.** Keep the existing launch form as the main menu; do not introduce a separate title screen. Switch between `menu`, `loading`, `playing`, `paused`, and save/load dialog states. The gameplay view contains the Canvas with the map and units; pause is a DOM overlay above it. Returning to the menu releases the active session and restores the launch form.

2. **Freeze the simulation, not just its appearance.** While paused, the render loop may continue drawing, but it must not call `Session.advance`. Reset the frame-time baseline on resume so the paused duration is not applied as a large simulation step. Ignore map clicks, selection changes, and movement orders while paused. Esc from a nested save/load screen returns to the pause menu; Esc from the pause menu resumes play.

3. **Add versioned state serialization at the simulation boundary.** A browser-only reconstruction from YAML cannot restore moving units, queued movement intent, or destroyed walls. Add a versioned snapshot export/import contract to the Rust/WASM session and validate the snapshot before replacing the active session. The snapshot includes the effective map/addon configuration, all simulation state needed to continue deterministically, selected units, and camera center/zoom. It excludes transient pointer and dialog state. Unknown versions and invalid data fail without discarding the current paused session.

4. **Use portable JSON snapshot files.** Saving serializes the snapshot as a versioned JSON file and invokes the browser download flow. Loading uses a file picker, validates the version and contents, then restores the snapshot. This keeps saves portable and user-controlled; browser-local slots and IndexedDB are unnecessary.

5. **Always confirm return to the main menu.** The confirmation appears every time the player chooses «В главное меню», whether or not the current game was saved. Loading a file over an active game also requires confirmation before replacing that game.

## Risks / Trade-offs

- [The WASM session may contain state that is not currently exposed for serialization] → Define the snapshot from authoritative simulation components and movement state, and add round-trip scenarios before connecting the UI.
- [Save formats can outlive code versions] → Include a format version and reject unsupported snapshots with a recoverable message; migration between versions is not part of this change.
- [Large or malformed files can fail to load] → Validate the complete snapshot before replacing the active session, and keep the paused game intact on failure.

## Migration Plan

No existing save data needs migration. Keep selected add-on preferences in `localStorage`. On successful launch, switch from the menu to gameplay; on return, ask for confirmation, dispose of the active session only after confirmation, and show the existing launch form. If snapshot restoration fails, retain the current paused session and report the error.
