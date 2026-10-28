//! The three enhanced HUD-number setting keys and their existing file format.
use super::ClientOptions;
pub(super) fn write(c: &ClientOptions, text: &mut String) {
    if !c.target_hp_percent {
        text.push_str("target_hp_percent=off\n");
    }
    if !c.xp_details {
        text.push_str("xp_details=off\n");
    }
    if !c.hud_numbers {
        text.push_str("hud_numbers=off\n");
    }
}
pub(super) fn parse(c: &mut ClientOptions, key: &str, value: &str) {
    let on = match value {
        "on" => true,
        "off" => false,
        _ => return,
    };
    match key {
        "target_hp_percent" => c.target_hp_percent = on,
        "xp_details" => c.xp_details = on,
        "hud_numbers" => c.hud_numbers = on,
        _ => {}
    }
}
pub(super) fn description(c: &ClientOptions) -> String {
    let on_off = |on| if on { "on" } else { "off" };
    format!(
        ", target hp % {}, xp details {}, hud numbers {}",
        on_off(c.target_hp_percent),
        on_off(c.xp_details),
        on_off(c.hud_numbers)
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_number_settings_write_nothing_and_off_keeps_legacy_keys() {
        let defaults = ClientOptions::default();
        let mut text = String::new();
        write(&defaults, &mut text);
        assert_eq!(text, "");
        let c = ClientOptions {
            target_hp_percent: false,
            xp_details: false,
            hud_numbers: false,
            ..defaults
        };
        write(&c, &mut text);
        assert_eq!(
            text,
            "target_hp_percent=off\nxp_details=off\nhud_numbers=off\n"
        );
        let mut restored = defaults;
        for line in text.lines() {
            let (key, value) = line.split_once('=').unwrap();
            parse(&mut restored, key, value);
        }
        assert_eq!(restored, c);
        assert_eq!(
            description(&c),
            ", target hp % off, xp details off, hud numbers off"
        );
        parse(&mut restored, "hud_numbers", "on");
        assert!(restored.hud_numbers);
    }
    #[test]
    fn unknown_keys_and_invalid_number_values_leave_defaults_alone() {
        let defaults = ClientOptions::default();
        let mut c = defaults;
        for (key, value) in [
            ("minimap", "off"),
            ("hud_numbers", "false"),
            ("target_hp_percent", "OFF"),
            ("xp_details", "0"),
        ] {
            parse(&mut c, key, value);
        }
        assert_eq!(c, defaults);
    }
}
