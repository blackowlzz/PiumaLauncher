use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PowerInfo {
    pub active: String,
}

#[derive(Debug)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct Hold {
    #[cfg(target_os = "linux")]
    bus: gio::DBusConnection,
    service: usize,
    cookie: u32,
}

#[cfg(target_os = "linux")]
mod imp {
    use super::{Hold, PowerInfo};
    use gio::glib::{self, ToVariant};

    const SERVICES: [(&str, &str); 2] = [
        ("org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles"),
        ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"),
    ];

    fn bus() -> Option<gio::DBusConnection> {
        gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE).ok()
    }

    fn call(bus: &gio::DBusConnection, service: usize, interface: &str, method: &str, args: &glib::Variant) -> Option<glib::Variant> {
        let (name, path) = SERVICES[service];
        bus.call_sync(Some(name), path, interface, method, Some(args), None, gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE).ok()
    }

    pub fn info() -> Option<PowerInfo> {
        let bus = bus()?;
        (0..SERVICES.len()).find_map(|i| {
            let reply = call(&bus, i, "org.freedesktop.DBus.Properties", "Get", &(SERVICES[i].0, "ActiveProfile").to_variant())?;
            let active = reply.child_value(0).as_variant()?.get::<String>()?;
            Some(PowerInfo { active })
        })
    }

    pub fn hold(reason: &str) -> Option<Hold> {
        let bus = bus()?;
        (0..SERVICES.len()).find_map(|i| {
            let reply = call(&bus, i, SERVICES[i].0, "HoldProfile", &("performance", reason, "dev.piuma.launcher").to_variant())?;
            let (cookie,) = reply.get::<(u32,)>()?;
            Some(Hold { bus: bus.clone(), service: i, cookie })
        })
    }

    pub fn release(hold: Hold) {
        let _ = call(&hold.bus, hold.service, SERVICES[hold.service].0, "ReleaseProfile", &(hold.cookie,).to_variant());
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::{Hold, PowerInfo};

    pub fn info() -> Option<PowerInfo> {
        None
    }

    pub fn hold(_reason: &str) -> Option<Hold> {
        None
    }

    pub fn release(_hold: Hold) {}
}

pub fn info() -> Option<PowerInfo> {
    imp::info()
}

pub async fn hold_performance() -> Option<Hold> {
    tokio::task::spawn_blocking(|| imp::hold("Playing Minecraft")).await.ok().flatten()
}

pub async fn release(hold: Hold) {
    let _ = tokio::task::spawn_blocking(move || imp::release(hold)).await;
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn hold_switches_to_performance_and_back() {
        let before = info().expect("power-profiles-daemon not running").active;
        let hold = hold_performance().await.expect("hold refused");
        assert_eq!(info().unwrap().active, "performance");
        release(hold).await;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert_eq!(info().unwrap().active, before);
    }
}
