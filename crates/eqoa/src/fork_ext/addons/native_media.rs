use super::{
    native::{bind, describe, error},
    textures::{Handle, Textures},
};
use mlua::{AnyUserData, Function, Lua, Result, Table, UserData, Value};
use std::{cell::RefCell, rc::Rc};

pub(crate) fn imgui(lua: &Lua, imgui: &Table) -> Result<()> {
    let root: Table = lua.named_registry_value("eqoa_root")?;
    root.set("imgui", imgui.clone())?;
    for name in ["vec2", "vec4"] {
        let constructor: Table =
            lua.globals()
                .get(if name == "vec2" { "ImVec2" } else { "ImVec4" })?;
        imgui.set(name, constructor.get::<Function>("new")?)?;
    }
    for pair in imgui.clone().pairs::<String, Value>() {
        let (name, v) = pair?;
        if v.is_function() {
            for prefix in ["eqoa.imgui", "ImGui"] {
                describe(lua, &format!("{prefix}.{name}"), "supported", Some(2), "");
            }
        }
    }
    describe(
        lua,
        "eqoa.imgui",
        "partial",
        Some(2),
        "bounded native bridge; unbound members are unsupported",
    );
    for name in [
        "BeginTooltip",
        "EndTooltip",
        "BeginItemTooltip",
        "GetMousePos",
        "GetMousePosOnOpeningCurrentPopup",
        "GetMouseWheel",
        "GetMouseDragDelta",
        "ResetMouseDragDelta",
        "IsMouseDown",
        "IsMouseClicked",
        "IsMouseReleased",
        "IsMouseDoubleClicked",
        "IsMousePosValid",
        "SetMousePos",
        "SetMouseCursor",
        "PushStyleVar",
        "PopStyleVar",
        "PushItemWidth",
        "PopItemWidth",
        "SetNextWindowFocus",
        "SetWindowFocus",
        "GetForegroundDrawList",
    ] {
        if imgui.raw_get::<Value>(name)?.is_nil() {
            let path = format!("ImGui.{name}");
            describe(lua, &path, "not_yet", Some(4), "scheduled for round 4");
            describe(
                lua,
                &format!("eqoa.imgui.{name}"),
                "not_yet",
                Some(4),
                "scheduled for round 4",
            );
            imgui.set(
                name,
                lua.create_function(move |lua, _: mlua::MultiValue| {
                    error(lua, &path, "not_yet", "scheduled for round 4")
                })?,
            )?;
        }
    }
    let mt = lua.create_table()?;
    mt.set(
        "__index",
        lua.create_function(|lua, (_, key): (Table, String)| {
            super::native::diagnose(
                lua,
                &format!("ImGui.{key}"),
                "unknown_symbol",
                "unbound ImGui member",
            );
            Ok(Value::Nil)
        })?,
    )?;
    imgui.set_metatable(Some(mt))?;
    let package: Table = lua.globals().get("package")?;
    package
        .get::<Table>("loaded")?
        .set("eqoa.imgui", imgui.clone())?;
    bind(lua,"client","display",lua.create_function(|lua,()| {
        unsafe {if dear_imgui_rs::sys::igGetCurrentContext().is_null(){return error(lua,"eqoa.client.display","unavailable","no ImGui display context");}
            let io=dear_imgui_rs::sys::igGetIO_Nil();lua.pack_multi((super::native::record(lua,"eqoa.client.DisplayMetrics",&serde_json::json!({"width":(*io).DisplaySize.x as u32,"height":(*io).DisplaySize.y as u32,"render_scale":crate::settings::client().render_scale}))?,Value::Nil))}
    })?)?;
    Ok(())
}
pub(crate) fn textures(lua: &Lua, bank: &Rc<RefCell<Textures>>) -> Result<()> {
    if lua.named_registry_value::<Table>("eqoa_root").is_err() {
        return Ok(());
    }
    let forge: Table = lua.globals().get("UiForge")?;
    let load: Function = forge.get("LoadTexture")?;
    bind(
        lua,
        "textures",
        "load",
        lua.create_function(move |lua, path: Value| {
            let Some(path) = path
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
            else {
                return error(
                    lua,
                    "eqoa.textures.load",
                    "invalid_argument",
                    "path must be a relative string",
                );
            };
            if path.starts_with(['/', '\\']) || path.contains(':') {
                return error(
                    lua,
                    "eqoa.textures.load",
                    "permission_denied",
                    "native paths must be relative",
                );
            }
            match load.call::<Value>(path)? {
                Value::UserData(ud) => lua.pack_multi((texture_info(lua, ud)?, Value::Nil)),
                _ => error(
                    lua,
                    "eqoa.textures.load",
                    "not_found",
                    "PNG resource could not be loaded; see resource diagnostic",
                ),
            }
        })?,
    )?;
    let create: Function = forge.get("CreateTextureFromMemory")?;
    bind(
        lua,
        "textures",
        "rgba",
        lua.create_function(move |lua, (bytes, w, h): (Value, Value, Value)| {
            let Some((w, h)) = super::native_state::uint(&w, 2048)
                .zip(super::native_state::uint(&h, 2048))
                .filter(|(w, h)| *w > 0 && *h > 0)
            else {
                return error(
                    lua,
                    "eqoa.textures.rgba",
                    "out_of_range",
                    "dimensions must be integers in 1..2048",
                );
            };
            let Some(bytes) = bytes.as_string() else {
                return error(
                    lua,
                    "eqoa.textures.rgba",
                    "invalid_argument",
                    "bytes must be a binary string",
                );
            };
            if bytes.as_bytes().len() != w as usize * h as usize * 4 {
                return error(
                    lua,
                    "eqoa.textures.rgba",
                    "invalid_argument",
                    "RGBA byte count differs from width*height*4",
                );
            }
            match create.call::<AnyUserData>((bytes.clone(), w, h)) {
                Ok(ud) => lua.pack_multi((texture_info(lua, ud)?, Value::Nil)),
                Err(e) => error(lua, "eqoa.textures.rgba", "failed", &e.to_string()),
            }
        })?,
    )?;
    let owner = bank.clone();
    bind(
        lua,
        "textures",
        "release",
        lua.create_function(move |lua, value: Value| {
            if value.is_nil() {
                return lua.pack_multi((true, Value::Nil));
            }
            let Some(ud) = value.as_userdata() else {
                return error(
                    lua,
                    "eqoa.textures.release",
                    "stale_handle",
                    "expected owned Texture",
                );
            };
            let Ok(h) = ud.borrow::<Handle>() else {
                return error(
                    lua,
                    "eqoa.textures.release",
                    "stale_handle",
                    "expected owned Texture",
                );
            };
            if !Rc::ptr_eq(&owner, &h.owner) {
                return error(
                    lua,
                    "eqoa.textures.release",
                    "stale_handle",
                    "texture belongs to another VM",
                );
            }
            owner.borrow_mut().mark_dead(h.id);
            lua.pack_multi((true, Value::Nil))
        })?,
    )?;
    describe(lua, "eqoa.textures.formats.png", "supported", Some(2), "");
    describe(lua, "eqoa.textures.formats.rgba", "supported", Some(2), "");
    Ok(())
}
fn texture_info(lua: &Lua, ud: AnyUserData) -> Result<Table> {
    let (w, h) = {
        let handle = ud.borrow::<Handle>()?;
        super::textures::dimensions(&handle)?
    };
    let t = lua.create_table()?;
    t.set("texture", ud)?;
    t.set("width", w)?;
    t.set("height", h)?;
    Ok(t)
}

#[derive(Clone)]
struct Sound {
    owner: Rc<RefCell<super::sound::SoundBank>>,
    id: super::sound::SoundId,
}
impl UserData for Sound {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::Eq, |_, this, other: AnyUserData| {
            Ok(other
                .borrow::<Sound>()
                .is_ok_and(|o| Rc::ptr_eq(&o.owner, &this.owner) && o.id == this.id))
        });
    }
}
pub(crate) fn sound_handle(
    lua: &Lua,
    owner: &Rc<RefCell<super::sound::SoundBank>>,
    id: super::sound::SoundId,
) -> Result<AnyUserData> {
    lua.create_userdata(Sound {
        owner: owner.clone(),
        id,
    })
}

pub(crate) fn sound(lua: &Lua, owner: &Rc<RefCell<super::sound::SoundBank>>) -> Result<()> {
    if lua.named_registry_value::<Table>("eqoa_root").is_err() {
        return Ok(());
    }
    let bank = owner.clone();
    bind(
        lua,
        "sound",
        "load",
        lua.create_function(move |lua, path: Value| {
            if !cfg!(windows) {
                return error(
                    lua,
                    "eqoa.sound.load",
                    "unsupported",
                    "no native sound backend on this platform",
                );
            }
            let Some(path) = path
                .as_string()
                .and_then(|s| s.to_str().ok().map(|s| s.to_string()))
            else {
                return error(
                    lua,
                    "eqoa.sound.load",
                    "invalid_argument",
                    "path must be a relative string",
                );
            };
            if path.starts_with(['/', '\\']) || path.contains(':') {
                return error(
                    lua,
                    "eqoa.sound.load",
                    "permission_denied",
                    "native paths must be relative",
                );
            }
            let forge: Table = lua.globals().get("UiForge")?;
            let load: Function = forge.get("LoadSound")?;
            let Some(id) = load.call::<Option<u32>>(path)? else {
                return error(
                    lua,
                    "eqoa.sound.load",
                    "failed",
                    "sound decoder/backend rejected resource",
                );
            };
            let id =
                super::sound::SoundId::new(id).map_err(|e| mlua::Error::runtime(e.to_string()))?;
            lua.pack_multi((
                lua.create_userdata(Sound {
                    owner: bank.clone(),
                    id,
                })?,
                Value::Nil,
            ))
        })?,
    )?;
    for name in ["play", "stop", "set_volume", "is_playing", "release"] {
        let bank = owner.clone();
        bind(
            lua,
            "sound",
            name,
            lua.create_function(move |lua, (value, option): (Value, Option<Value>)| {
                let symbol = format!("eqoa.sound.{name}");
                if !cfg!(windows) {
                    return error(
                        lua,
                        &symbol,
                        "unsupported",
                        "no native sound backend on this platform",
                    );
                }
                if value.is_nil() && matches!(name, "stop" | "release" | "is_playing") {
                    return lua.pack_multi((name != "is_playing", Value::Nil));
                }
                let Some(ud) = value.as_userdata() else {
                    return error(lua, &symbol, "stale_handle", "expected owned Sound");
                };
                let Ok(sound) = ud.borrow::<Sound>() else {
                    return error(lua, &symbol, "stale_handle", "expected owned Sound");
                };
                if !Rc::ptr_eq(&bank, &sound.owner) {
                    return error(lua, &symbol, "stale_handle", "sound belongs to another VM");
                }
                let mut bank = bank.borrow_mut();
                if !bank.contains(sound.id) && name != "release" {
                    return error(lua, &symbol, "stale_handle", "sound has been released");
                }
                let ok = match name {
                    "is_playing" => {
                        return lua.pack_multi((bank.is_sound_playing(Some(sound.id)), Value::Nil));
                    }
                    "stop" => bank.stop_sound(Some(sound.id)),
                    "release" => {
                        bank.release_sound(Some(sound.id));
                        !bank.contains(sound.id)
                    }
                    "set_volume" => {
                        let Some(volume) = option
                            .as_ref()
                            .and_then(super::native_state::number)
                            .filter(|v| (0.0..=1.0).contains(v))
                        else {
                            return error(
                                lua,
                                &symbol,
                                "invalid_argument",
                                "volume must be finite in 0..1",
                            );
                        };
                        bank.set_sound_volume(Some(sound.id), volume as f32)
                    }
                    _ => {
                        let options = match option {
                            None | Some(Value::Nil) => super::sound::PlayOptions::default(),
                            Some(Value::Table(t)) => {
                                let v = t.get::<Value>("volume")?;
                                let volume = if v.is_nil() {
                                    1.0
                                } else {
                                    let Some(v) = super::native_state::number(&v)
                                        .filter(|v| (0.0..=1.0).contains(v))
                                    else {
                                        return error(
                                            lua,
                                            &symbol,
                                            "invalid_argument",
                                            "volume must be finite in 0..1",
                                        );
                                    };
                                    v as f32
                                };
                                let looping = match t.get::<Value>("loop")? {
                                    Value::Nil => false,
                                    Value::Boolean(v) => v,
                                    _ => {
                                        return error(
                                            lua,
                                            &symbol,
                                            "invalid_argument",
                                            "loop must be a bool",
                                        );
                                    }
                                };
                                super::sound::PlayOptions { volume, looping }
                            }
                            _ => {
                                return error(
                                    lua,
                                    &symbol,
                                    "invalid_argument",
                                    "options must be a table",
                                );
                            }
                        };
                        bank.play_sound(Some(sound.id), Some(options))
                    }
                };
                if ok {
                    lua.pack_multi((true, Value::Nil))
                } else {
                    error(
                        lua,
                        &symbol,
                        "failed",
                        "native audio backend rejected operation",
                    )
                }
            })?,
        )?;
    }
    #[cfg(not(windows))]
    for name in [
        "load",
        "play",
        "stop",
        "set_volume",
        "is_playing",
        "release",
    ] {
        describe(
            lua,
            &format!("eqoa.sound.{name}"),
            "unsupported",
            None,
            "no native sound backend on this platform",
        );
    }
    Ok(())
}
