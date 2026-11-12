//! The amp as a program: `cargo run -p amp --features standalone`.
//!
//! Started with no arguments it opens the audio interface the player chose in the window's
//! Audio panel last time (`amp::standalone`), with the machine's default devices before
//! that. Started with arguments they go to nih-plug's host as they are (`--help` lists them).

use amp::standalone::{self, DriverDevices, Prefs, Reconfigure};
use cpal::traits::{DeviceTrait, HostTrait};
use nih_plug::prelude::*;
#[cfg(all(target_os = "windows", feature = "asio"))]
use std::{collections::HashMap, sync::OnceLock};

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
        #[cfg(all(target_os = "windows", feature = "asio"))]
        if driver == "asio" {
            return asio_snapshot().devices.clone();
        }
        live_devices(driver)
    }

    fn channels(&self, driver: &str, input: bool, device: Option<&str>) -> u32 {
        #[cfg(all(target_os = "windows", feature = "asio"))]
        if driver == "asio" {
            return asio_snapshot().channels(input, device);
        }
        live_channels(driver, input, device)
    }

    fn set_test_tone(&self, on: bool) {
        nih_standalone_set_test_tone(on);
    }

    fn reconfigure(&self, prefs: &Prefs) -> Reconfigure {
        // Only the devices and their channels can change while the amp plays
        if !prefs.same_stream(&standalone::launch()) {
            return Reconfigure::Unsupported;
        }
        let Some(path) = standalone::prefs_path() else {
            return Reconfigure::Unsupported;
        };
        let devices = resolved_running(prefs.clone());
        let Ok(answer) = nih_standalone_reconfigure(StandaloneDevices {
            input_device: devices.input_device.clone(),
            output_device: devices.output_device.clone(),
            input_channel: devices.input_channel,
            output_channel: devices.output_channel,
        }) else {
            return Reconfigure::Unsupported;
        };
        // What was chosen is kept for the next start, and noted as what plays, only once it
        // plays: a device that would not open must not be the one the amp starts on. What is
        // noted is the resolved choice, like the one the program started with
        let chosen = prefs.clone();
        let opening = standalone::after_ok(answer, move || {
            let _ = chosen.save(&path);
            standalone::set_launch(devices);
        });
        Reconfigure::Opening(opening)
    }

    fn restart(&self, prefs: &Prefs) -> std::io::Result<()> {
        let path = standalone::prefs_path()
            .ok_or_else(|| std::io::Error::other("no place for the settings"))?;
        prefs.save(&path)?;
        // An ASIO driver does not let go of its streams cleanly when the window closes: the
        // program hangs there. So with ASIO the program starts again from here, a moment
        // later (the driver is free once this process is gone), and this one exits at once
        #[cfg(all(target_os = "windows", feature = "asio"))]
        if standalone::launch().backend == "asio" {
            let program = std::env::current_exe()?;
            std::process::Command::new("cmd")
                .args(["/C", "ping", "-n", "3", "127.0.0.1", ">nul", "&", "start", "", "/B"])
                .arg(program)
                .spawn()?;
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(300));
                std::process::exit(0);
            });
            return Ok(());
        }
        // The program starts again from `main` once the window has closed and the audio
        // device is let go
        standalone::request_restart();
        Ok(())
    }
}

/// What a driver offers right now
fn live_devices(driver: &str) -> DriverDevices {
    // `auto` tries JACK, then the system's driver
    let Some(host) = Machine::host(driver) else {
        return DriverDevices::default();
    };
    DriverDevices {
        inputs: host.input_devices().map(names).unwrap_or_default(),
        outputs: host.output_devices().map(names).unwrap_or_default(),
    }
}

/// How many channels a device has (the default device when none is named), 2 when unknown
fn live_channels(driver: &str, input: bool, device: Option<&str>) -> u32 {
    let Some(host) = Machine::host(driver) else {
        return 2;
    };
    let find = |mut devices: Vec<cpal::Device>, default: Option<cpal::Device>| {
        let named =
            device.and_then(|name| devices.iter().position(|d| d.name().is_ok_and(|n| n == name)));
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

/// What ASIO offered before the program opened its streams. An ASIO driver can be loaded
/// once, so asking it again for its devices while the streams run (as the Audio panel does)
/// would hang the window; the panel reads this instead
#[cfg(all(target_os = "windows", feature = "asio"))]
struct AsioSnapshot {
    devices: DriverDevices,
    inputs: HashMap<String, u32>,
    outputs: HashMap<String, u32>,
}

#[cfg(all(target_os = "windows", feature = "asio"))]
impl AsioSnapshot {
    fn take() -> Self {
        let devices = live_devices("asio");
        let count = |input: bool, names: &[String]| {
            names
                .iter()
                .map(|name| (name.clone(), live_channels("asio", input, Some(name))))
                .collect()
        };
        Self {
            inputs: count(true, &devices.inputs),
            outputs: count(false, &devices.outputs),
            devices,
        }
    }

    fn channels(&self, input: bool, device: Option<&str>) -> u32 {
        let (counts, names) = if input {
            (&self.inputs, &self.devices.inputs)
        } else {
            (&self.outputs, &self.devices.outputs)
        };
        device
            .or(names.first().map(String::as_str))
            .and_then(|d| counts.get(d))
            .copied()
            .unwrap_or(2)
    }
}

#[cfg(all(target_os = "windows", feature = "asio"))]
static ASIO: OnceLock<AsioSnapshot> = OnceLock::new();

#[cfg(all(target_os = "windows", feature = "asio"))]
fn asio_snapshot() -> &'static AsioSnapshot {
    ASIO.get_or_init(AsioSnapshot::take)
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

/// `resolved` for the amp that is playing. An ASIO driver cannot be asked again while it plays
/// (the Audio panel reads its snapshot), so ASIO's devices are the snapshot's
fn resolved_running(prefs: Prefs) -> Prefs {
    #[cfg(all(target_os = "windows", feature = "asio"))]
    if prefs.backend == "asio" {
        let snapshot = asio_snapshot();
        let drivers = standalone::Host::drivers(&Machine);
        return standalone::resolve(
            prefs,
            &drivers,
            &snapshot.devices.inputs,
            &snapshot.devices.outputs,
            snapshot.devices.inputs.first().cloned(),
        );
    }
    resolved(prefs)
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

/// The window the plugin's editor opens is made by the framework without an icon, so the
/// taskbar and the title bar show Windows' default one. Once the window is there it is given
/// the badge the exe carries (`assets/amp.rc`, resource 1)
#[cfg(target_os = "windows")]
fn give_window_the_icon() {
    use std::ffi::c_void;
    type Hwnd = *mut c_void;
    #[link(name = "user32")]
    extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
        fn LoadImageW(
            module: *mut c_void,
            name: *const u16,
            kind: u32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> *mut c_void;
        fn SendMessageW(window: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
        fn GetSystemMetrics(index: i32) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }
    const IMAGE_ICON: u32 = 1;
    const WM_SETICON: u32 = 0x0080;
    const ICON_SMALL: usize = 0;
    const ICON_BIG: usize = 1;
    const SM_CXICON: i32 = 11;
    const SM_CXSMICON: i32 = 49;
    std::thread::spawn(|| {
        let title: Vec<u16> = "Hojt Guitar Amp ".encode_utf16().collect();
        for _ in 0..200 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            // SAFETY: plain Win32 calls with NUL-terminated strings; the icon resource 1 is
            // the one the exe is built with, and the handles are only passed to the window
            unsafe {
                let window = FindWindowW(std::ptr::null(), title.as_ptr());
                if window.is_null() {
                    continue;
                }
                let module = GetModuleHandleW(std::ptr::null());
                let resource = 1usize as *const u16;
                for (kind, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
                    let size = GetSystemMetrics(metric);
                    let icon = LoadImageW(module, resource, IMAGE_ICON, size, size, 0);
                    if !icon.is_null() {
                        SendMessageW(window, WM_SETICON, kind, icon as isize);
                    }
                }
                return;
            }
        }
    });
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        enable_dpi_awareness();
        give_window_the_icon();
    }
    standalone::install(Box::new(Machine));
    // Before any stream is open: the Audio panel cannot ask the driver once it is running
    #[cfg(all(target_os = "windows", feature = "asio"))]
    asio_snapshot();
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
