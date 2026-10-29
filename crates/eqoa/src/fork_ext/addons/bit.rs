//! Native LuaJIT BitOp 1.0.2 compatible `bit` library implementation.
//!
//! PREPARED ARTIFACT - UNCOMPILED / INACTIVE.
//! Stored in ignored `target/stdlib-prep/` per root instructions.
//! Do NOT link into active crate modules until compiler ownership is scheduled.
//!
//! Scoped functions for this slice:
//! - 7 core: `tobit`, `bnot`, `band`, `bor`, `bxor`, `lshift`, `rshift`
//! - 1 verified extension: `arshift`
//! Note: `tohex`, `bswap`, `rol`, `ror` are omitted from this slice.
//!
//! Authoritative references:
//! - `luajit-src-210.7.4+c6ffc14/luajit2/src/host/minilua.c:7693-7720`
//! - `luajit-src-210.7.4+c6ffc14/luajit2/src/lib_bit.c:40-52, 104-110, 134-142`
//! - `luajit-src-210.7.4+c6ffc14/luajit2/src/vm_x86.dasc:470-471, 3236-3243`
//! - `luajit-src-210.7.4+c6ffc14/luajit2/src/vm_arm.dasc:2572-2583`

use mlua::{Error, Lua, MultiValue, Result, Table};

/// Floating-point bias (2^52 + 2^51) synthesized in LuaJIT vm_x86/vm_arm
/// and minilua.c:7696 (`bn.n = lua_tonumber(L, idx) + 6755399441055744.0`).
pub const BIAS: f64 = 6755399441055744.0;

/// Normalizes a Lua double-precision float (`f64`) into a signed 32-bit integer (`i32`).
///
/// In standard IEEE-754 arithmetic, adding `BIAS` shifts the integer part of the double
/// to align integer units with bit 0 of the 52-bit mantissa. Fractional parts are rounded
/// according to IEEE-754 round-to-nearest (ties to even), matching LuaJIT JIT and
/// interpreter behavior across x86 and ARM platforms.
///
/// Classification Caveats:
/// 1. Fractional inputs: In the official Lua BitOp specification, fractional behavior
///    is classified as implementation-defined platform compatibility rather than a
///    universally defined BitOp standard.
/// 2. Non-finite & out-of-range inputs: Values outside [-2^51, 2^51] and non-finite values
///    (NaN, +inf, -inf) are explicitly undefined in the Lua BitOp specification. In this
///    native adaptation, non-finite values are safely mapped to `0` to prevent NaN poisoning.
#[inline]
pub fn to_i32(val: f64) -> i32 {
    if !val.is_finite() {
        return 0;
    }
    ((val + BIAS).to_bits() as u32) as i32
}

/// Normalizes a Lua double into an unsigned 32-bit integer (`u32`).
#[inline]
pub fn to_u32(val: f64) -> u32 {
    to_i32(val) as u32
}

/// Creates and populates the native `bit` module table with 8 scoped operations.
pub fn create_module(lua: &Lua) -> Result<Table> {
    let bit = lua.create_table()?;

    // bit.tobit(x, ...) -> i32
    bit.set(
        "tobit",
        lua.create_function(|lua, args: MultiValue| {
            if args.is_empty() {
                return Err(Error::runtime(
                    "bad argument #1 to 'tobit' (number expected, got no value)",
                ));
            }
            let val: f64 = lua.unpack(args[0].clone())?;
            Ok(to_i32(val))
        })?,
    )?;

    // bit.bnot(x, ...) -> i32
    bit.set(
        "bnot",
        lua.create_function(|lua, args: MultiValue| {
            if args.is_empty() {
                return Err(Error::runtime(
                    "bad argument #1 to 'bnot' (number expected, got no value)",
                ));
            }
            let val: f64 = lua.unpack(args[0].clone())?;
            Ok(!to_i32(val))
        })?,
    )?;

    // bit.band(x1, ...) -> i32 (variadic, minimum arity 1)
    bit.set(
        "band",
        lua.create_function(|lua, args: MultiValue| {
            if args.is_empty() {
                return Err(Error::runtime(
                    "bad argument #1 to 'band' (number expected, got no value)",
                ));
            }
            let mut acc = 0xFFFF_FFFFu32;
            for arg in args {
                let val: f64 = lua.unpack(arg)?;
                acc &= to_u32(val);
            }
            Ok(acc as i32)
        })?,
    )?;

    // bit.bor(x1, ...) -> i32 (variadic, minimum arity 1)
    bit.set(
        "bor",
        lua.create_function(|lua, args: MultiValue| {
            if args.is_empty() {
                return Err(Error::runtime(
                    "bad argument #1 to 'bor' (number expected, got no value)",
                ));
            }
            let mut acc = 0u32;
            for arg in args {
                let val: f64 = lua.unpack(arg)?;
                acc |= to_u32(val);
            }
            Ok(acc as i32)
        })?,
    )?;

    // bit.bxor(x1, ...) -> i32 (variadic, minimum arity 1)
    bit.set(
        "bxor",
        lua.create_function(|lua, args: MultiValue| {
            if args.is_empty() {
                return Err(Error::runtime(
                    "bad argument #1 to 'bxor' (number expected, got no value)",
                ));
            }
            let mut acc = 0u32;
            for arg in args {
                let val: f64 = lua.unpack(arg)?;
                acc ^= to_u32(val);
            }
            Ok(acc as i32)
        })?,
    )?;

    // bit.lshift(x, n, ...) -> i32 (binary, minimum arity 2, shift count masked with & 31)
    bit.set(
        "lshift",
        lua.create_function(|lua, args: MultiValue| {
            if args.len() < 2 {
                let idx = if args.is_empty() { 1 } else { 2 };
                return Err(Error::runtime(format!(
                    "bad argument #{idx} to 'lshift' (number expected, got no value)"
                )));
            }
            let x: f64 = lua.unpack(args[0].clone())?;
            let n: f64 = lua.unpack(args[1].clone())?;
            let b = to_u32(x);
            let sh = to_u32(n) & 31;
            Ok((b << sh) as i32)
        })?,
    )?;

    // bit.rshift(x, n, ...) -> i32 (logical right shift, zero-filling)
    bit.set(
        "rshift",
        lua.create_function(|lua, args: MultiValue| {
            if args.len() < 2 {
                let idx = if args.is_empty() { 1 } else { 2 };
                return Err(Error::runtime(format!(
                    "bad argument #{idx} to 'rshift' (number expected, got no value)"
                )));
            }
            let x: f64 = lua.unpack(args[0].clone())?;
            let n: f64 = lua.unpack(args[1].clone())?;
            let b = to_u32(x);
            let sh = to_u32(n) & 31;
            Ok((b >> sh) as i32)
        })?,
    )?;

    // bit.arshift(x, n, ...) -> i32 (arithmetic right shift, sign-extending)
    bit.set(
        "arshift",
        lua.create_function(|lua, args: MultiValue| {
            if args.len() < 2 {
                let idx = if args.is_empty() { 1 } else { 2 };
                return Err(Error::runtime(format!(
                    "bad argument #{idx} to 'arshift' (number expected, got no value)"
                )));
            }
            let x: f64 = lua.unpack(args[0].clone())?;
            let n: f64 = lua.unpack(args[1].clone())?;
            let b = to_i32(x);
            let sh = to_u32(n) & 31;
            Ok(b >> sh)
        })?,
    )?;

    Ok(bit)
}

#[cfg(test)]
mod proposed_tests {
    use super::*;
    use mlua::{Lua, Value};

    #[test]
    fn test_to_i32_direct_boundary_and_rounding() {
        // Basic signed integers
        assert_eq!(to_i32(0.0), 0);
        assert_eq!(to_i32(1.0), 1);
        assert_eq!(to_i32(-1.0), -1);
        assert_eq!(to_i32(2147483647.0), 2147483647);
        assert_eq!(to_i32(-2147483648.0), -2147483648);

        // Modular 32-bit wrap
        assert_eq!(to_i32(4294967295.0), -1); // 0xFFFFFFFF
        assert_eq!(to_i32(4294967296.0), 0); // 0x100000000
        assert_eq!(to_i32(2147483648.0), -2147483648); // 0x80000000

        // IEEE-754 mantissa-shift round-to-nearest (ties to even), NOT simple truncation:
        assert_eq!(to_i32(1.2), 1);
        assert_eq!(to_i32(1.6), 2);
        assert_eq!(to_i32(1.5), 2); // Ties to even (2 is even)
        assert_eq!(to_i32(2.5), 2); // Ties to even (2 is even)
        assert_eq!(to_i32(-1.6), -2);

        // Non-finite guards (safely normalized to 0 in this native adaptation)
        assert_eq!(to_i32(f64::NAN), 0);
        assert_eq!(to_i32(f64::INFINITY), 0);
        assert_eq!(to_i32(f64::NEG_INFINITY), 0);
    }

    #[test]
    fn test_lua_surface_operations_and_multireturns() {
        let lua = Lua::new();
        let bit = create_module(&lua).unwrap();
        lua.globals().set("bit", bit).unwrap();

        // 1. Single return value check (not multiple return values)
        lua.load(
            r#"
            local a, b = bit.tobit(42)
            assert(a == 42 and b == nil, "tobit must return exactly 1 value")
            local c, d = bit.band(6, 2)
            assert(c == 2 and d == nil, "band must return exactly 1 value")
            "#,
        )
        .exec()
        .unwrap();

        // 2. Return types are Lua numbers representing signed 32-bit integers
        lua.load(
            r#"
            assert(type(bit.tobit(0xFFFFFFFF)) == "number")
            assert(bit.tobit(0xFFFFFFFF) == -1)
            assert(bit.tobit(0x80000000) == -2147483648)
            assert(bit.bnot(0) == -1)
            assert(bit.bnot(-1) == 0)
            assert(bit.band(6, 2) == 2)
            assert(bit.bor(1, 2, 4) == 7)
            assert(bit.bxor(1, 3, 2) == 0)
            assert(bit.lshift(1, 4) == 16)
            assert(bit.lshift(1, 32) == 1)   -- shift count masking (& 31)
            assert(bit.lshift(1, 33) == 2)
            assert(bit.rshift(-1, 1) == 2147483647) -- logical zero-fill
            assert(bit.arshift(-1, 1) == -1)         -- arithmetic sign-extend
            assert(bit.arshift(-16, 2) == -4)
            "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn test_lua_surface_variadics_and_extra_arguments() {
        let lua = Lua::new();
        let bit = create_module(&lua).unwrap();
        lua.globals().set("bit", bit).unwrap();

        // Variadics with 1, 2, 3+ arguments
        lua.load(
            r#"
            assert(bit.band(0xFFFFFFFF) == -1, "single-arg band is identity tobit")
            assert(bit.bor(0xFFFFFFFF) == -1, "single-arg bor is identity tobit")
            assert(bit.bxor(0xFFFFFFFF) == -1, "single-arg bxor is identity tobit")

            assert(bit.band(0xFF, 0x0F, 0x03) == 3)
            assert(bit.bor(1, 2, 4, 8) == 15)
            assert(bit.bxor(1, 3, 2, 0) == 0)
            "#,
        )
        .exec()
        .unwrap();

        // Extra trailing arguments are ignored on unary and binary operations (matching C BitOp)
        lua.load(
            r#"
            assert(bit.tobit(1, 999, "extra") == 1)
            assert(bit.bnot(0, 999) == -1)
            assert(bit.lshift(1, 2, 999) == 4)
            assert(bit.rshift(4, 2, 999) == 1)
            assert(bit.arshift(-4, 1, 999) == -2)
            "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn test_lua_surface_string_numeric_coercion_and_rounding() {
        let lua = Lua::new();
        let bit = create_module(&lua).unwrap();
        lua.globals().set("bit", bit).unwrap();

        // String numeric coercion
        lua.load(
            r#"
            assert(bit.band("15", "7") == 7)
            assert(bit.bor("1", "2") == 3)
            assert(bit.tobit("4294967295") == -1)
            "#,
        )
        .exec()
        .unwrap();

        // Lua surface IEEE-754 mantissa-shift round-to-nearest (ties to even)
        lua.load(
            r#"
            assert(bit.tobit(1.2) == 1)
            assert(bit.tobit(1.6) == 2)
            assert(bit.tobit(1.5) == 2)
            assert(bit.tobit(2.5) == 2)
            assert(bit.tobit(-1.6) == -2)
            "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn test_lua_surface_zero_args_and_type_errors() {
        let lua = Lua::new();
        let bit = create_module(&lua).unwrap();
        lua.globals().set("bit", bit).unwrap();

        // Zero-argument calls must fail (tested via Rust since sandbox excludes pcall)
        assert!(lua.load("bit.band()").exec().is_err());
        assert!(lua.load("bit.bor()").exec().is_err());
        assert!(lua.load("bit.bxor()").exec().is_err());
        assert!(lua.load("bit.tobit()").exec().is_err());
        assert!(lua.load("bit.bnot()").exec().is_err());
        assert!(lua.load("bit.lshift()").exec().is_err());
        assert!(lua.load("bit.lshift(1)").exec().is_err());
        assert!(lua.load("bit.rshift(1)").exec().is_err());
        assert!(lua.load("bit.arshift(1)").exec().is_err());

        // Invalid-type arguments must fail with conversion errors
        assert!(lua.load("bit.band({}, 1)").exec().is_err());
        assert!(lua.load("bit.band(nil, 1)").exec().is_err());
        assert!(lua.load("bit.tobit('not_a_number')").exec().is_err());
        assert!(lua.load("bit.lshift(1, true)").exec().is_err());
    }

    #[test]
    fn test_module_table_isolation() {
        let lua = Lua::new();
        let bit1 = create_module(&lua).unwrap();
        let bit2 = create_module(&lua).unwrap();

        bit1.set("custom_marker", "mod1").unwrap();
        assert_eq!(bit1.get::<String>("custom_marker").unwrap(), "mod1");
        assert_eq!(bit2.get::<Value>("custom_marker").unwrap(), Value::Nil);
    }
}
