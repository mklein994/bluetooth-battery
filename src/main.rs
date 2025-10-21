use std::time::Duration;

use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::{ObjectManager, Properties};

const USAGE_MESSAGE: &str = concat!(
    "Usage: ",
    env!("CARGO_BIN_NAME"),
    " [-3hlnsV] [--help | --i3 | --long | --narrow | --pango | --short | --usage | --version] [DEVICE]..."
);

const HELP_MESSAGE_FRAGMENT: &str = "Show the battery life of connected bluetooth devices.

Use -h or --help to show this help message.

Project home page: https://github.com/mklein994/bluetooth-battery

POSITIONAL ARGUMENTS:
  [DEVICE]...  The bluetooth device's address, e.g. AA:BB:CC:DD:EE:FF.

FORMAT OPTIONS:
  -3, --i3       Format for i3blocks using pango markup.
  --pango        An alias for --i3.
  -l, --long     Use a long format (icon, name, percentage).
  -s, --short    Use a short format (name, percentage).
  -n, --narrow   Use a narrow format (icon, percentage). This is the default.

OTHER OPTIONS:
  -h, --usage    Print a short usage message.
  --help         Print this full help message.
  -V, --version  Print the version.";

#[derive(Default)]
struct Opt {
    fmt: DeviceFormat,
    i3: bool,
    addresses: Vec<String>,
}

impl Opt {
    fn from_args(args: impl ExactSizeIterator<Item = String>) -> Self {
        let mut opt = Self::default();
        for arg in args {
            match arg.as_str() {
                "-s" | "--short" => {
                    opt.fmt = DeviceFormat::Short;
                }
                "-l" | "--long" => {
                    opt.fmt = DeviceFormat::Long;
                }
                "-n" | "--narrow" => {
                    opt.fmt = DeviceFormat::Narrow;
                }
                "-3" | "--i3" | "--pango" => {
                    opt.i3 = true;
                }
                "-h" | "--usage" => {
                    println!("{USAGE_MESSAGE}");
                    std::process::exit(0);
                }
                "--help" => {
                    println!("{USAGE_MESSAGE}\n\n{HELP_MESSAGE_FRAGMENT}");
                    std::process::exit(0);
                }
                "-V" | "--version" => {
                    println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                x if x.contains(|c: char| c.is_ascii_hexdigit() || c == ':') => {
                    opt.addresses.push(arg);
                }
                _ => {
                    eprintln!("{USAGE_MESSAGE}");
                    std::process::exit(1);
                }
            }
        }

        opt
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args(std::env::args().skip(1));

    let conn = Connection::new_system()?;
    let timeout = Duration::from_secs(5);

    let mut devices = if opt.addresses.is_empty() {
        let proxy = conn.with_proxy("org.bluez", "/", timeout);

        let objects = proxy.get_managed_objects()?;

        objects
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
            .collect()
    } else {
        let mut device_list = vec![];
        for address in opt.addresses {
            let path = format!(
                "/org/bluez/hci0/dev_{}",
                address.to_ascii_uppercase().replace(':', "_")
            );
            let proxy = conn.with_proxy("org.bluez", path, timeout);

            let connected: bool = proxy.get("org.bluez.Device1", "Connected")?;
            if !connected {
                continue;
            }

            let power: u8 = proxy.get("org.bluez.Battery1", "Percentage")?;
            let name: String = proxy.get("org.bluez.Device1", "Name")?;
            let icon: String = proxy.get("org.bluez.Device1", "Icon")?;

            device_list.push(Device {
                name,
                icon: Icon(icon),
                power: power.into(),
            });
        }

        device_list
    };

    devices.sort_unstable();

    for (i, device) in devices.iter().enumerate() {
        print!(
            "{}",
            match opt.fmt {
                DeviceFormat::Long => device.long(opt.i3),
                DeviceFormat::Short => device.short(),
                DeviceFormat::Narrow => device.narrow(opt.i3),
            }
        );

        if i < devices.len() - 1 {
            if let DeviceFormat::Short = opt.fmt {
                print!("  ");
            } else {
                print!(" ");
            }
        }

        if i == devices.len() - 1 {
            println!();
        }
    }

    Ok(())
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Device {
    name: String,
    icon: Icon,
    power: u64,
}

impl Device {
    fn long(&self, i3: bool) -> String {
        format!(
            "{}{} ({}%)",
            if i3 {
                self.icon.material_symbols()
            } else {
                self.icon.emoji()
            }
            .unwrap_or_default(),
            self.name,
            self.power
        )
    }

    fn short(&self) -> String {
        format!("{} {}%", self.name, self.power)
    }

    fn narrow(&self, i3: bool) -> String {
        format!(
            "{}{}%",
            if i3 {
                self.icon.material_symbols()
            } else {
                self.icon.emoji()
            }
            .unwrap_or_default(),
            self.power
        )
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
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
            "audio-headset" => Some("🎧 "),
            "phone" | "pda" => Some("📱 "),
            "input-keyboard" => Some("⌨️ "),
            "input-mouse" => Some("🖱️ "),
            "input-gaming" => Some("🎮 "),
            "input-tablet" => Some("🖍️  "),
            "multimedia-player" => Some("📻 "),
            "printer" | "scanner" => Some("🖨️  "),
            _ => None,
        }
    }

    fn material_symbols(&self) -> Option<&str> {
        // https://docs.gtk.org/Pango/pango_markup.html#the-span-attributes
        macro_rules! i3 {
            ($x:literal) => {
                concat!(
                    "<span font_desc='Material Symbols Outlined @opsz=20,FILL=1,GRAD=-25' rise='-3pt'>",
                    $x,
                    "</span> "
                )
            };
        }

        // https://specifications.freedesktop.org/icon-naming-spec/latest/#devices
        match self.0.as_str() {
            "audio-headset" => Some(i3!("headphones")),
            "phone" | "pda" => Some(i3!("smartphone")),
            "input-keyboard" => Some(i3!("keyboard")),
            "input-mouse" => Some(i3!("mouse")),
            "input-gaming" => Some(i3!("sports_esports")),
            "input-tablet" => Some(i3!("tablet_android")),
            "multimedia-player" => Some(i3!("media_bluetooth_on")),
            "printer" => Some(i3!("print")),
            "scanner" => Some(i3!("scanner")),
            _ => None,
        }
    }
}

#[derive(Default)]
enum DeviceFormat {
    Long,
    Short,
    #[default]
    Narrow,
}
