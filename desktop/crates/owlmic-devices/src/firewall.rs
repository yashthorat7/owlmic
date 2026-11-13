//! The firewall rules Owlmic needs (SYSTEM_DESIGN section 24.2, step 6), named exactly as the
//! installer names them, so a repair replaces rather than duplicates them.

pub const PROGRAM_RULE: &str = "Owlmic";

/// Name, protocol and port of each inbound port rule.
pub const PORT_RULES: [(&str, &str, u16); 3] = [
    ("Owlmic TCP", "TCP", 7653),
    ("Owlmic UDP Beacon", "UDP", 7654),
    ("Owlmic UDP Media", "UDP", 7655),
];

/// The netsh commands that remove earlier rules and add the current ones.
pub fn commands(exe: &str) -> Vec<String> {
    let mut out: Vec<String> = PORT_RULES
        .iter()
        .map(|(name, _, _)| name)
        .chain([&PROGRAM_RULE])
        .map(|name| format!("netsh advfirewall firewall delete rule name=\"{name}\""))
        .collect();
    out.extend(PORT_RULES.iter().map(|(name, protocol, port)| {
        format!("netsh advfirewall firewall add rule name=\"{name}\" dir=in action=allow protocol={protocol} localport={port} profile=any")
    }));
    out.push(format!(
        "netsh advfirewall firewall add rule name=\"{PROGRAM_RULE}\" dir=in action=allow program=\"{exe}\" enable=yes profile=any"
    ));
    out
}

/// The public bit of a rule's profiles (NET_FW_PROFILE2_PUBLIC).
const PROFILE_PUBLIC: i32 = 4;

/// Whether a rule lets phones in: enabled, inbound, allowing, and covering public networks
/// (Windows often treats home Wi-Fi as public).
pub fn rule_allows(enabled: bool, inbound: bool, allow: bool, profiles: i32) -> bool {
    enabled && inbound && allow && profiles & PROFILE_PUBLIC != 0
}

/// Reads the rules through the firewall's own API, which answers the same on every Windows
/// language (netsh's output is translated). Needs COM on the calling thread.
#[cfg(windows)]
pub fn rules_present() -> bool {
    use windows::Win32::NetworkManagement::WindowsFirewall::{
        INetFwPolicy2, NET_FW_ACTION_ALLOW, NET_FW_RULE_DIR_IN, NetFwPolicy2,
    };
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    use windows::core::BSTR;

    let Ok(rules) = (unsafe {
        CoCreateInstance::<_, INetFwPolicy2>(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)
            .and_then(|p| p.Rules())
    }) else {
        return false;
    };
    PORT_RULES.iter().all(|(name, _, _)| unsafe {
        rules.Item(&BSTR::from(*name)).is_ok_and(|r| {
            rule_allows(
                r.Enabled().is_ok_and(|e| e.as_bool()),
                r.Direction().is_ok_and(|d| d == NET_FW_RULE_DIR_IN),
                r.Action().is_ok_and(|a| a == NET_FW_ACTION_ALLOW),
                r.Profiles().unwrap_or(0),
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_must_be_enabled_inbound_allowing_and_cover_public_networks() {
        let all = 0x7FFF_FFFF;
        assert!(rule_allows(true, true, true, all));
        assert!(rule_allows(true, true, true, PROFILE_PUBLIC));
        assert!(
            !rule_allows(true, true, true, 1 | 2),
            "domain and private only"
        );
        assert!(!rule_allows(false, true, true, all));
        assert!(!rule_allows(true, false, true, all));
        assert!(!rule_allows(true, true, false, all));
    }

    #[test]
    fn commands_replace_every_rule_by_its_exact_name() {
        let c = commands("C:\\Program Files\\Owlmic\\owlmic.exe");
        assert_eq!(c.len(), 8);
        assert!(c[0].ends_with("name=\"Owlmic TCP\""));
        assert!(c[3].ends_with("name=\"Owlmic\""));
        assert!(c.iter().any(|l| l.contains("name=\"Owlmic UDP Media\"")
            && l.contains("protocol=UDP localport=7655")));
        assert!(c[7].contains("program=\"C:\\Program Files\\Owlmic\\owlmic.exe\""));
    }
}
