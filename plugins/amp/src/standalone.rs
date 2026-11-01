//! The amp as a program of its own: what the player chose for the audio interface, kept
//! between launches, and the hook the window uses to list the machine's devices.
//!
//! nih-plug's standalone host opens its audio streams once, from its command line. So the
//! program (`main.rs`) turns the saved choices into that command line when it starts, and the
//! window's Audio panel saves new choices and starts the program again to open them. In a
//! DAW nothing here is used: no host is installed and the editor shows no Audio button.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The buffer sizes the panel offers, in samples
pub const PERIODS: [u32; 4] = [64, 128, 256, 512];
/// The sample rates the panel offers, in Hz
pub const RATES: [u32; 4] = [44_100, 48_000, 88_200, 96_000];
/// The name of the settings file in `config_dir`
pub const FILE: &str = "amp-standalone.json";

/// What the player chose. A device that is `None` is the machine's default
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// The audio driver, as nih-plug's `--backend` names it: `auto`, `alsa`, `jack`,
    /// `wasapi`, `core-audio`
    pub backend: String,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    /// Buffer size in samples
    pub period_size: u32,
    pub sample_rate: u32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            backend: "auto".into(),
            input_device: None,
            output_device: None,
            period_size: 256,
            sample_rate: 48_000,
        }
    }
}

impl Prefs {
    /// The saved choices, or the defaults for a missing or unreadable file. Values the
    /// panel does not offer are put back to the nearest it does
    pub fn load(path: &Path) -> Self {
        let mut prefs: Self = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        prefs.period_size = nearest(&PERIODS, prefs.period_size);
        prefs.sample_rate = nearest(&RATES, prefs.sample_rate);
        prefs
    }

    /// Writes the choices, making the folder if it is not there yet
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    /// The command line of nih-plug's standalone host for these choices, program name first
    pub fn args(&self, program: &str) -> Vec<String> {
        let mut args = vec![
            program.to_owned(),
            "--backend".into(),
            self.backend.clone(),
            "--period-size".into(),
            self.period_size.to_string(),
            "--sample-rate".into(),
            self.sample_rate.to_string(),
        ];
        for (flag, device) in
            [("--input-device", &self.input_device), ("--output-device", &self.output_device)]
        {
            if let Some(device) = device {
                args.push(flag.into());
                args.push(device.clone());
            }
        }
        args
    }
}

fn nearest(list: &[u32], value: u32) -> u32 {
    list.iter().copied().min_by_key(|&entry| entry.abs_diff(value)).unwrap_or(value)
}

/// The folder of the settings file: `HOJT_CONFIG_DIR` if set, else the system's own place
/// for a program's settings (`%APPDATA%` on Windows, `~/Library/Application Support` on
/// macOS, `$XDG_CONFIG_HOME` or `~/.config` elsewhere), in a folder `Hojt Audio`
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("HOJT_CONFIG_DIR").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let env = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        env("APPDATA")?
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library").join("Application Support")
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|home| home.join(".config")))?
    };
    Some(base.join("Hojt Audio"))
}

/// The settings file
pub fn prefs_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(FILE))
}

/// What the machine offers for one driver
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DriverDevices {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

/// The program's side of the Audio panel: it knows the machine's drivers and devices and
/// how to start the program again with new choices
pub trait Host: Send + Sync {
    /// The drivers, as the `--backend` names them
    fn drivers(&self) -> Vec<String>;
    /// The devices of one driver
    fn devices(&self, driver: &str) -> DriverDevices;
    /// Saves `prefs` and starts the program again with them. The window is closed after
    fn restart(&self, prefs: &Prefs) -> std::io::Result<()>;
}

static HOST: OnceLock<Box<dyn Host>> = OnceLock::new();

/// Installs the host. Only the standalone program does; the plugin in a DAW has none
pub fn install(host: Box<dyn Host>) {
    let _ = HOST.set(host);
}

/// The host, if this is the standalone program
pub fn host() -> Option<&'static dyn Host> {
    HOST.get().map(|host| &**host)
}

/// What the Audio panel works on: the lists it steps through and the choice so far
#[derive(Clone, Debug)]
pub struct AudioChoice {
    drivers: Vec<String>,
    devices: DriverDevices,
    pub prefs: Prefs,
    /// What `restart` said when it failed
    pub error: Option<String>,
}

/// Shown for "the machine's default device" and when a driver has no device
pub const DEFAULT_DEVICE: &str = "Default";
pub const NO_DEVICE: &str = "None";

impl AudioChoice {
    /// Opens the panel on `prefs`, listing `host`'s drivers and the devices of the driver
    /// that is chosen
    pub fn open(host: &dyn Host, prefs: Prefs) -> Self {
        let mut drivers = host.drivers();
        if !drivers.contains(&prefs.backend) {
            drivers.insert(0, prefs.backend.clone());
        }
        let devices = host.devices(&prefs.backend);
        Self { drivers, devices, prefs, error: None }
    }

    /// The five rows of the panel: caption and what shows
    pub fn rows(&self) -> [(&'static str, String); 5] {
        [
            ("Driver", self.prefs.backend.to_uppercase()),
            ("Input", shown(self.prefs.input_device.as_deref(), &self.devices.inputs)),
            ("Output", shown(self.prefs.output_device.as_deref(), &self.devices.outputs)),
            ("Buffer", format!("{} samples", self.prefs.period_size)),
            ("Sample rate", format!("{} Hz", self.prefs.sample_rate)),
        ]
    }

    /// Moves row `row` (see `rows`) to the next entry of its list, or the previous one
    pub fn step(&mut self, host: &dyn Host, row: usize, forward: bool) {
        self.error = None;
        match row {
            0 => {
                let at = position(&self.drivers, &self.prefs.backend);
                self.prefs.backend = self.drivers[around(at, self.drivers.len(), forward)].clone();
                self.devices = host.devices(&self.prefs.backend);
                // A device of another driver means nothing to this one
                self.prefs.input_device = None;
                self.prefs.output_device = None;
            }
            1 => step_device(&mut self.prefs.input_device, &self.devices.inputs, forward),
            2 => step_device(&mut self.prefs.output_device, &self.devices.outputs, forward),
            3 => self.prefs.period_size = step_list(&PERIODS, self.prefs.period_size, forward),
            _ => self.prefs.sample_rate = step_list(&RATES, self.prefs.sample_rate, forward),
        }
    }
}

fn shown(chosen: Option<&str>, listed: &[String]) -> String {
    match chosen {
        Some(name) => name.to_owned(),
        None if listed.is_empty() => NO_DEVICE.into(),
        None => DEFAULT_DEVICE.into(),
    }
}

fn position(list: &[String], name: &str) -> usize {
    list.iter().position(|entry| entry == name).unwrap_or(0)
}

fn around(at: usize, len: usize, forward: bool) -> usize {
    if forward {
        (at + 1) % len
    } else {
        (at + len - 1) % len
    }
}

/// The entries are the default (`None`) and then the listed names
fn step_device(chosen: &mut Option<String>, listed: &[String], forward: bool) {
    let at = chosen.as_deref().map_or(0, |name| 1 + position(listed, name));
    *chosen = match around(at, listed.len() + 1, forward) {
        0 => None,
        entry => Some(listed[entry - 1].clone()),
    };
}

fn step_list(list: &[u32], value: u32, forward: bool) -> u32 {
    let at = list.iter().position(|&entry| entry == value).unwrap_or(0);
    list[around(at, list.len(), forward)]
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;
    impl Host for Fake {
        fn drivers(&self) -> Vec<String> {
            vec!["auto".into(), "wasapi".into()]
        }
        fn devices(&self, driver: &str) -> DriverDevices {
            match driver {
                "wasapi" => DriverDevices {
                    inputs: vec!["Scarlett".into(), "Mic".into()],
                    outputs: vec!["Speakers".into()],
                },
                _ => DriverDevices::default(),
            }
        }
        fn restart(&self, _: &Prefs) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hojt-standalone-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn test_the_choices_come_back_after_a_restart() {
        let path = dir("roundtrip").join(FILE);
        let prefs = Prefs {
            backend: "wasapi".into(),
            input_device: Some("Scarlett".into()),
            output_device: Some("Speakers".into()),
            period_size: 128,
            sample_rate: 96_000,
        };
        prefs.save(&path).unwrap();
        assert_eq!(Prefs::load(&path), prefs);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_a_missing_or_broken_file_gives_the_defaults() {
        let path = dir("broken").join(FILE);
        assert_eq!(Prefs::load(&path), Prefs::default());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not json {").unwrap();
        assert_eq!(Prefs::load(&path), Prefs::default());
        // A field the file lacks is the default; a size the panel lacks is put to the nearest
        std::fs::write(&path, r#"{"period_size": 100, "input_device": "A"}"#).unwrap();
        let loaded = Prefs::load(&path);
        assert_eq!((loaded.period_size, loaded.backend.as_str()), (128, "auto"));
        assert_eq!(loaded.input_device.as_deref(), Some("A"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_the_choices_become_the_hosts_command_line() {
        let prefs = Prefs {
            backend: "wasapi".into(),
            input_device: Some("Scarlett 2i2".into()),
            output_device: None,
            period_size: 64,
            sample_rate: 44_100,
        };
        assert_eq!(
            prefs.args("amp"),
            [
                "amp",
                "--backend",
                "wasapi",
                "--period-size",
                "64",
                "--sample-rate",
                "44100",
                "--input-device",
                "Scarlett 2i2"
            ]
        );
    }

    #[test]
    fn test_the_panel_steps_through_each_list_and_round() {
        let host = Fake;
        let mut choice = AudioChoice::open(&host, Prefs::default());
        assert_eq!(choice.rows()[1].1, NO_DEVICE);
        choice.step(&host, 0, true);
        assert_eq!(choice.prefs.backend, "wasapi");
        assert_eq!(choice.rows()[1].1, DEFAULT_DEVICE);
        choice.step(&host, 1, true);
        choice.step(&host, 1, true);
        assert_eq!(choice.prefs.input_device.as_deref(), Some("Mic"));
        choice.step(&host, 1, true);
        assert_eq!(choice.prefs.input_device, None);
        choice.step(&host, 1, false);
        assert_eq!(choice.prefs.input_device.as_deref(), Some("Mic"));
        // Another driver forgets the devices of this one
        choice.step(&host, 0, true);
        assert_eq!(choice.prefs.input_device, None);
        choice.step(&host, 3, false);
        assert_eq!(choice.prefs.period_size, 128);
        choice.step(&host, 3, false);
        choice.step(&host, 3, false);
        assert_eq!(choice.prefs.period_size, 512);
        choice.step(&host, 4, true);
        assert_eq!(choice.prefs.sample_rate, 88_200);
    }
}
