use std::time::Duration;

use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::ObjectManager;

const HELP_MESSAGE: &str = concat!(
    "Usage: ",
    env!("CARGO_BIN_NAME"),
    " [-hflnsV] [ --help | --long | --narrow | --short | --version ]"
);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fmt = std::env::args()
        .nth(1)
        .map(|x| match x.as_str() {
            "--short" | "-s" => DeviceFormat::Short,
            "--long" | "--full" | "-l" | "-f" => DeviceFormat::Long,
            "--narrow" | "-n" => DeviceFormat::Narrow,
            "--help" | "-h" => {
                println!("{HELP_MESSAGE}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            _ => {
                eprintln!("{HELP_MESSAGE}");
                std::process::exit(1);
            }
        })
        .unwrap_or_default();

    let conn = Connection::new_system()?;
    let proxy = conn.with_proxy("org.bluez", "/", Duration::from_secs(5));

    let objects = proxy.get_managed_objects()?;

    let devices = objects
        .into_values()
        .filter_map(|v| {
            let device = v.get("org.bluez.Device1")?;
            let connected = device
                .get("Connected")
                .and_then(|x| x.0.as_u64())
                .is_some_and(|x| x != 0);
            let name = device.get("Name").and_then(|x| x.0.as_str())?.to_string();
            let icon = device
                .get("Icon")
                .and_then(|x| x.0.as_str())?
                .parse()
                .ok()?;
            let power = v
                .get("org.bluez.Battery1")
                .and_then(|x| x.get("Percentage"))
                .and_then(|x| x.0.as_u64())?;

            connected.then_some(Device { name, icon, power })
        })
        .map(|x| match fmt {
            DeviceFormat::Long => x.long(),
            DeviceFormat::Short => x.short(),
            DeviceFormat::Narrow => x.narrow(),
        });

    for device in devices {
        println!("{device}");
    }

    Ok(())
}

#[derive(Debug)]
struct Device {
    name: String,
    icon: Icon,
    power: u64,
}

impl Device {
    fn long(&self) -> String {
        format!(
            "{:<2}{} ({}%)",
            self.icon.emoji().unwrap_or_default(),
            self.name,
            self.power
        )
    }

    fn short(&self) -> String {
        format!("{} {}%", self.name, self.power)
    }

    fn narrow(&self) -> String {
        format!(
            "{:<2}{}%",
            self.icon.emoji().unwrap_or_default(),
            self.power
        )
    }
}

#[derive(Debug)]
struct Icon(String);

impl std::str::FromStr for Icon {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl Icon {
    // https://specifications.freedesktop.org/icon-naming-spec/latest/#devices
    fn emoji(&self) -> Option<&str> {
        match self.0.as_str() {
            "audio-headset" => Some("🎧"),
            "phone" | "pda" => Some("📱"),
            "input-keyboard" => Some("⌨️ "),
            "input-mouse" => Some("🖱️ "),
            "input-gaming" => Some("🎮"),
            "input-tablet" => Some("🖍️ "),
            "multimedia-player" => Some("📻"),
            "printer" | "scanner" => Some("🖨️ "),
            _ => None,
        }
    }
}

#[derive(Default)]
enum DeviceFormat {
    #[default]
    Long,
    Short,
    Narrow,
}
