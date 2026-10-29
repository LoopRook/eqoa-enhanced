-- Native spike fixture; uses the pinned UiForge memory-texture and ImGui contracts.
if not fixture_texture then
    fixture_texture = UiForge.CreateTextureFromMemory(string.char(0,255,0,255), 1, 1)
end
if ImGui.Begin("Forge widget fixture", true, ImGuiWindowFlags.AlwaysAutoResize) then
    local text, entered = ImGui.InputText("Text", "hello", ImGuiInputTextFlags.EnterReturnsTrue)
    assert(type(text) == "string" and type(entered) == "boolean")
    ImGui.Image(fixture_texture, 32, 32)
end
ImGui.End()
