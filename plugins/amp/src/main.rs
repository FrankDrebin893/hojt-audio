//! The amp as a program: `cargo run -p amp --features standalone`.
//!
//! Started with no arguments it opens the audio interface the player chose in the window's
//! Audio panel last time (`amp::standalone`), with the machine's default devices before
//! that. Started with arguments they go to nih-plug's host as they are (`--help` lists them).

use amp::standalone::{self, DriverDevices, Prefs};
use cpal::traits::{DeviceTrait, HostTrait};
use nih_plug::prelude::*;

/// The driver cpal talks to on this system, and the name `--backend` knows it by
#[cfg(target_os = "windows")]
const SYSTEM: (cpal::HostId, &str) = (cpal::HostId::Wasapi, "wasapi");
#[cfg(target_os = "macos")]
const SYSTEM: (cpal::HostId, &str) = (cpal::HostId::CoreAudio, "core-audio");
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const SYSTEM: (cpal::HostId, &str) = (cpal::HostId::Alsa, "alsa");

struct Machine;

impl Machine {
    fn host() -> Option<cpal::Host> {
        cpal::host_from_id(SYSTEM.0).ok()
    }
}

fn names<D: DeviceTrait>(devices: impl Iterator<Item = D>) -> Vec<String> {
    devices.filter_map(|device| device.name().ok()).collect()
}

impl standalone::Host for Machine {
    fn drivers(&self) -> Vec<String> {
        ["auto", SYSTEM.1, "jack"].map(String::from).to_vec()
    }

    fn devices(&self, driver: &str) -> DriverDevices {
        // JACK connects by port, not by device; `auto` tries JACK, then the system's driver
        if driver == "jack" {
            return DriverDevices::default();
        }
        let Some(host) = Self::host() else {
            return DriverDevices::default();
        };
        DriverDevices {
            inputs: host.input_devices().map(names).unwrap_or_default(),
            outputs: host.output_devices().map(names).unwrap_or_default(),
        }
    }

    fn restart(&self, prefs: &Prefs) -> std::io::Result<()> {
        let path = standalone::prefs_path()
            .ok_or_else(|| std::io::Error::other("no place for the settings"))?;
        prefs.save(&path)?;
        // The same program, no arguments: it reads what was just saved
        std::process::Command::new(std::env::current_exe()?).spawn()?;
        Ok(())
    }
}

/// The saved choices with the defaults of this machine put in: an input is only connected
/// when named (nih-plug does not connect one by itself, to keep a microphone from feeding
/// back), and a device that is no longer plugged in is left to the default
fn resolved(mut prefs: Prefs) -> Prefs {
    if prefs.backend == "jack" {
        return prefs;
    }
    let Some(host) = Machine::host() else {
        return prefs;
    };
    let plugged = |wanted: &Option<String>, list: Vec<String>| {
        wanted.clone().filter(|name| list.contains(name))
    };
    prefs.input_device =
        plugged(&prefs.input_device, host.input_devices().map(names).unwrap_or_default())
            .or_else(|| host.default_input_device().and_then(|device| device.name().ok()));
    prefs.output_device =
        plugged(&prefs.output_device, host.output_devices().map(names).unwrap_or_default());
    prefs
}

fn main() {
    standalone::install(Box::new(Machine));
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        nih_export_standalone_with_args::<amp::GuitarAmpPlugin, _>(args);
        return;
    }
    let prefs = standalone::prefs_path().map(|path| Prefs::load(&path)).unwrap_or_default();
    nih_export_standalone_with_args::<amp::GuitarAmpPlugin, _>(resolved(prefs).args(&args[0]));
}
