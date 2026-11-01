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
        // The program starts again from `main` once the window has closed and the audio
        // device is let go
        standalone::request_restart();
        Ok(())
    }
}

/// The saved choices with this machine's devices put in (see `standalone::resolve`)
fn resolved(prefs: Prefs) -> Prefs {
    let drivers = standalone::Host::drivers(&Machine);
    let Some(host) = Machine::host() else {
        return standalone::resolve(prefs, &drivers, &[], &[], None);
    };
    standalone::resolve(
        prefs,
        &drivers,
        &host.input_devices().map(names).unwrap_or_default(),
        &host.output_devices().map(names).unwrap_or_default(),
        host.default_input_device().and_then(|device| device.name().ok()),
    )
}

fn start(args: Vec<String>) -> bool {
    nih_export_standalone_with_args::<amp::GuitarAmpPlugin, _>(args)
}

fn main() {
    standalone::install(Box::new(Machine));
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        standalone::set_launch(Prefs::from_args(&args));
        start(args);
    } else {
        let path = standalone::prefs_path();
        let saved = path.as_deref().map(Prefs::load).unwrap_or_default();
        let launch = resolved(saved);
        standalone::set_launch(launch.clone());
        // A saved choice that does not open must not keep the amp from starting: set the
        // file aside and start again on the defaults
        if !start(launch.args(&args[0])) && launch != Prefs::default() {
            if let Some(path) = path {
                let _ = std::fs::rename(&path, path.with_extension("bad"));
            }
            start(Prefs::default().args(&args[0]));
        }
    }
    // The window is closed and the streams are let go: only now can the program open them again
    if standalone::restart_requested() {
        if let Ok(program) = std::env::current_exe() {
            let _ = std::process::Command::new(program).spawn();
        }
    }
}
