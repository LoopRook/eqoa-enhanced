# UiForge Media Contract: Font Atlas & Audio Playback Architecture

**Status**: Specification / Prep (Read-Only Stage)
**Target Path**: `target/forge-preview-prep/media-contract.md`
**Referenced Worktrees**:
- Forge Implementation: `C:\Users\hocka\.paseo\worktrees\1usktejc\forge-compatibility`
- Pinned FrontiersForge C++: `run/forge-compat-reference/UiForge`
- Pinned Script: `run/forge-compat-reference/scripts/uiforge_example/uiforge_example.lua`
- Pinned ImGui Header: `imgui-37b7a7a9.h`

---

## 1. Executive Summary

This document specifies the exact contract and native runtime architecture for **Fonts** and **Audio** required by unmodified `uiforge_example.lua` and future UiForge scripts.

Key findings from source verification:
1. **Fonts**:
   - `UiForge.LoadFont` in FrontiersForge returns an `ImFont*` (Lua userdata), NOT an invented integer handle.
   - **Context Requirement**: If called before an ImGui context is initialized (`!ImGui::GetCurrentContext()`), it logs a warning and returns `nil` (`nullptr`).
   - **Fallback Rule**: Once a context exists, any missing file or decode failure falls back to Dear ImGui's default font (`io.FontDefault` or `Fonts[0]`), so subsequent `ImGui.PushFont(font)` calls never crash.
   - **Memory Safety**: `dear-imgui-rs` 0.18 provides a fully safe, validated font API via `StbTrueTypeFontData::from_bytes` / `from_slice` and `FontSource::stb_truetype_with_size(validated, size)` (`src/fonts/atlas/validated/mod.rs`), which validates all stb reads and composite glyph bounds. Unsafe raw TTF pointers and unverified headers are strictly avoided.
   - **Atlas Lifecycle**: Mid-frame atlas mutation is forbidden by `dear-imgui-rs` (`assert_no_open_font_atlas_frames`). The architecture uses **queued pre-frame registration** with stable context-scoped userdata: frame 0 resolves transparently to the real default font; between frames, the manager flushes pending loads into the atlas; frame 1+ automatically resolves to the real requested 20px / 36px font without script changes.
   - **PushFont Sizing**: Per pinned `imgui-37b7a7a9.h`, ImGui's inline `PushFont(font)` calls `PushFont(font, font ? font->LegacySize : 0.0f)`. Passing `0.0f` to `sys::igPushFont` keeps the *current* font size and breaks 20px vs 36px distinction; the bridge must push with the font's reference size.
2. **Audio**: `UiForge.LoadSound` in FrontiersForge returns an integer sound handle or **`nil`** if missing or invalid. Every other audio API (`PlaySound`, `StopSound`, `IsSoundPlaying`, `SetSoundVolume`, `ReleaseSound`) is nil-safe. A source-faithful Windows MCI port (`sound.rs`) avoids touching core game audio (`audio.rs`), linking `winmm.dll` directly. Non-Windows platforms explicitly return `nil` with an `UnsupportedPlatform` diagnostic.

---

## 2. Font Contract & Runtime Architecture

### 2.1 FrontiersForge C++ Authoritative Contract

Inspected from `run/forge-compat-reference/UiForge/src/core/core.cpp` (lines 682–735) and `sol_ImGui.h`:

```cpp
lua.new_usertype<ImFont>("ImFont", sol::no_constructor);

uiforge_table["LoadFont"] = [](const std::string& path, sol::optional<float> size_px) -> ImFont*
{
    if (!ImGui::GetCurrentContext()) {
        PLOG_WARNING << "LoadFont called before ImGui was initialized: " << path;
        return nullptr; // Returns Lua nil
    }

    ImGuiIO& io = ImGui::GetIO();
    ImFont* fallback = io.FontDefault ? io.FontDefault
                     : (io.Fonts->Fonts.empty() ? nullptr : io.Fonts->Fonts[0]);

    const float size = (size_px && *size_px > 0.0f) ? *size_px : 0.0f;
    const std::filesystem::path font_path = ResolveResourcePath(path);
    const std::string cache_key = font_path.string() + "|" + std::to_string(size);

    auto cached = loaded_fonts.find(cache_key);
    if (cached != loaded_fonts.end()) {
        return cached->second;
    }

    if (!std::filesystem::exists(font_path, ec)) {
        PLOG_WARNING << "LoadFont could not find \"" << font_path.string() << "\". Falling back to the default font.";
        loaded_fonts[cache_key] = fallback;
        return fallback;
    }

    ImFontConfig font_config;
    font_config.Flags |= ImFontFlags_NoLoadError;
    ImFont* font = io.Fonts->AddFontFromFileTTF(font_path.string().c_str(), size, &font_config);
    if (!font) {
        PLOG_WARNING << "LoadFont failed to load \"" << font_path.string() << "\". Falling back to the default font.";
        loaded_fonts[cache_key] = fallback;
        return fallback;
    }

    PLOG_DEBUG << "Loaded font \"" << font_path.string() << "\" at size " << size;
    loaded_fonts[cache_key] = font;
    return font;
};
```

#### PushFont & PopFont Semantics (`imgui-37b7a7a9.h` & `sol_ImGui.h`)
In Dear ImGui 1.92, the obsolete single-argument `ImGui::PushFont(font)` is defined as:
```cpp
inline void PushFont(ImFont* font) { PushFont(font, font ? font->LegacySize : 0.0f); }
```
**Critical Sizing Rule**: Passing `0.0f` to Dear ImGui 1.92 dynamic fonts means *"keep the current font size"*. Calling `sys::igPushFont(font, 0.0f)` when the active font size is 13px will keep 13px, failing to apply the requested 20px or 36px! The bridge must pass `font ? font->LegacySize : default_size` (or the font's reference size) so that 20px and 36px font requests render at their exact baked sizes.

#### Exact Semantics Breakdown:
- **Return Type**: `ImFont*` (userdata in Lua), never an invented integer handle or synthetic table.
- **Context Guard**: If called before an ImGui context exists, returns `nil` (`nullptr`), matching C++.
- **Fallback Rule**: If the file is missing or invalid, caches and returns `fallback` (`io.FontDefault` or `Fonts[0]`). Subsequent requests return the cached fallback without re-reading the disk or repeating warning logs.
- **Size Argument**: Optional float. If `<= 0.0` or omitted, passed as `0.0f` to ImGui, which retains the default reference size.

---

### 2.2 Safe Font Validation via `StbTrueTypeFontData`

Rather than relying on unverified `ab_glyph` header checks or unsafe raw TTF memory slices, `dear-imgui-rs` 0.18 provides a primary, fully validated safe font source:

- **Source Module**: `dear_imgui_rs::fonts::atlas::validated::StbTrueTypeFontData` (`src/fonts/atlas/validated/mod.rs`).
- **Validation**:
  - `StbTrueTypeFontData::from_file(path)`
  - `StbTrueTypeFontData::from_bytes(bytes)`
  - `StbTrueTypeFontData::from_slice(slice)`
  These methods fully validate all font tables, offsets, standalone font indices, and composite glyph bounds against stb_truetype buffer over-reads.
- **Safe Font Source Construction**:
  ```rust
  let validated_data = StbTrueTypeFontData::from_file(&canonical_path)?;
  let font_source = FontSource::stb_truetype_with_size(validated_data, size_pixels);
  ```
  This guarantees 100% memory safety at the Rust type level, completely eliminating raw pointer risk during atlas compilation.

---

### 2.3 Queued Pre-Frame Atlas Registration (One-Frame Adaptation)

#### Why No Mid-Frame FFI Atlas Bypass:
In `dear-imgui-rs` 0.18 (`src/fonts/atlas/state.rs` L842):
```rust
assert!(!unsafe { (*context).WithinFrameScope },
    "{caller} cannot modify a font atlas while a registered ImGui context is mid-frame");
```
Attempting to bypass this via raw FFI `ImFontAtlas_AddFontFromFileTTF` while a frame scope is open causes font texture synchronization conflicts and violates Rust safety invariants.

#### Chosen Architecture: Stable Context-Scoped Userdata with Queued Pre-Frame Flushing
Scripts like `uiforge_example.lua` invoke `LoadFont` at top-level chunk execution during a frame:
```lua
local times_font = UiForge.LoadFont("C:\\Windows\\Fonts\\times.ttf", 20.0)
local times_font_large = UiForge.LoadFont("C:\\Windows\\Fonts\\times.ttf", 36.0)
local missing_font = UiForge.LoadFont("C:\\Windows\\Fonts\\does_not_exist.ttf", 20.0)
```
To support scripts storing `times_font` once in a local variable without script-side reload logic:
1. **Userdata Handle Representation (`ImFontHandle`)**:
   - Lua userdata wrapping `Rc<RefCell<FontHandleState>>`.
   - States:
     - `Pending { path: PathBuf, size: f32 }`: Initial state when requested mid-frame.
     - `Ready { font_id: FontId, size: f32 }`: Populated once baked into the atlas.
     - `Fallback`: Populated if the file is missing or invalid.
2. **Frame 0 (First Frame Execution)**:
   - `UiForge.LoadFont` enqueues the request into the manager's `pending_fonts` queue.
   - Returns an `ImFontHandle` in the `Pending` state.
   - When the script immediately calls `ImGui.PushFont(times_font)` during Frame 0, the bridge resolves `Pending` to the context's **real default font** (`io.FontDefault`). The frame renders safely with standard text.
3. **Pre-Frame Flush (Before Frame 1 `context.try_frame()`)**:
   - The manager inspects `pending_fonts`.
   - For each pending request:
     - Validates the font file using `StbTrueTypeFontData::from_file`.
     - Calls `context.font_atlas().add_font(&[FontSource::stb_truetype_with_size(data, size)])`.
     - Updates the shared `FontHandleState` in the `ImFontHandle` from `Pending` to `Ready { font_id, size }`.
     - If validation fails or the file is missing, updates the state to `Fallback`.
4. **Frame 1+ (Subsequent Frames Execution)**:
   - The script calls `ImGui.PushFont(times_font)` using the same variable.
   - The bridge resolves `Ready { font_id, size }` and calls `ui.push_font(font_id)` (passing `(*font_ptr).LegacySize` per `imgui-37b7a7a9.h`).
   - The text immediately renders at the real 20px / 36px font!
   - Missing fonts (`does_not_exist.ttf`) resolve to `Fallback` and are cached, so they always use the default font and never queue again.

---

### 2.4 Bridge Safety, Lifetime, and Error Boundaries

1. **Typed Reverse Font Unwind**:
   - The bridge maintains a per-frame stack of active font tokens.
   - If a Lua script encounters an error or returns without matching `ImGui.PopFont()`, the bridge unwinds the open font tokens in reverse order before closing the frame, preventing ImGui font stack underflow or overflow.
2. **Context Teardown & Invalidation**:
   - Each `ImFontHandle` records the context epoch.
   - When the ImGui context is destroyed or recreated (e.g. on window resize or reset), the epoch increments.
   - Any attempt to push a stale or foreign font handle generates a clean Lua runtime error rather than dereferencing a dangling native pointer.
3. **Strict Resource Bounds**:
   - Max 16 queued fonts per addon.
   - Max 16 MiB per font asset.
   - Max 64 MiB aggregate font memory across the bank.
   - Font cache shared across the context per owner handle.

---

### 2.5 Font Testing & Verification Plan

1. **Text Measurement & Pixel Sizing**:
   - Verify that after the pre-frame flush, text rendered with `times_font` (20px) vs `times_font_large` (36px) yields distinct, non-zero proportional bounding boxes (asserting that size is not kept at default 13px).
2. **Missing Font Graceful Fallback**:
   - Verify that loading a nonexistent font resolves to `Fallback` and renders with the default font without panics or repeated disk reads.
3. **Imbalance Reverse Unwind**:
   - Verify that executing a script that calls `PushFont` without `PopFont` causes the bridge to unwind the stack safely, leaving the context healthy for subsequent frames.
4. **No Fake Font IDs**:
   - Handles map directly to real Dear ImGui `FontId` references; no synthetic integer maps or mock IDs.

---

## 3. Audio Contract (Summary)

The audio contract is implemented by `target/forge-preview-prep/sound.rs`:
- Direct Windows MCI port (`winmm.dll`, `mciSendStringW`) matching `audio_manager.cpp`.
- Finite volume clamping to `[0.0, 1.0]` matching C++ `ToMciVolume`; non-finite values (`NaN`, `Inf`) rejected.
- Relative paths contained in `script_root`; absolute paths accepted for `script_root`, `resource_dirs`, or system media (`C:\Windows\Media`).
- Sound IDs strictly bounded to positive `i32` (`1..=0x7FFF_FFFF`).
- Full nil safety, path caching, and conservative close failure retention.
