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
    /// The cpal host of a driver, as `--backend` names it. JACK connects by port, not by
    /// device, so it has none; `auto` is the system's
    fn host(driver: &str) -> Option<cpal::Host> {
        match driver {
            "jack" => None,
            #[cfg(all(target_os = "windows", feature = "asio"))]
            "asio" => cpal::host_from_id(cpal::HostId::Asio).ok(),
            _ => cpal::host_from_id(SYSTEM.0).ok(),
        }
    }
}

fn names<D: DeviceTrait>(devices: impl Iterator<Item = D>) -> Vec<String> {
    devices.filter_map(|device| device.name().ok()).collect()
}

impl standalone::Host for Machine {
    fn drivers(&self) -> Vec<String> {
        let mut drivers = vec!["auto", SYSTEM.1, "jack"];
        // ASIO is what an audio interface's own driver offers at the lowest latency
        if cfg!(all(target_os = "windows", feature = "asio")) {
            drivers.insert(2, "asio");
        }
        drivers.into_iter().map(String::from).collect()
    }

    fn devices(&self, driver: &str) -> DriverDevices {
        // `auto` tries JACK, then the system's driver
        let Some(host) = Self::host(driver) else {
            return DriverDevices::default();
        };
        DriverDevices {
            inputs: host.input_devices().map(names).unwrap_or_default(),
            outputs: host.output_devices().map(names).unwrap_or_default(),
        }
    }

    fn channels(&self, driver: &str, input: bool, device: Option<&str>) -> u32 {
        let Some(host) = Self::host(driver) else {
            return 2;
        };
        let find = |mut devices: Vec<cpal::Device>, default: Option<cpal::Device>| {
            let named = device
                .and_then(|name| devices.iter().position(|d| d.name().is_ok_and(|n| n == name)));
            named.map(|at| devices.swap_remove(at)).or(default)
        };
        let most = |configs: Vec<cpal::SupportedStreamConfigRange>| {
            configs.iter().map(|config| u32::from(config.channels())).max()
        };
        if input {
            let devices = host.input_devices().map(|d| d.collect()).unwrap_or_default();
            find(devices, host.default_input_device())
                .and_then(|d| d.supported_input_configs().ok().and_then(|c| most(c.collect())))
        } else {
            let devices = host.output_devices().map(|d| d.collect()).unwrap_or_default();
            find(devices, host.default_output_device())
                .and_then(|d| d.supported_output_configs().ok().and_then(|c| most(c.collect())))
        }
        .unwrap_or(2)
    }

    fn set_test_tone(&self, on: bool) {
        nih_standalone_set_test_tone(on);
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
    let Some(host) = Machine::host(&prefs.backend) else {
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

#[cfg(target_os = "windows")]
fn enable_dpi_awareness() {
    const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;
    #[link(name = "user32")]
    extern "system" {
        fn SetProcessDpiAwarenessContext(value: isize) -> i32;
    }
    // SAFETY: tells Windows this standalone process handles monitor DPI without bitmap scaling.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    enable_dpi_awareness();
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
