//! The amp as a program of its own: what the player chose for the audio interface, kept
//! between launches, and the hook the window uses to list the machine's devices.
//!
//! nih-plug's standalone host opens its audio streams from its command line. So the program
//! (`main.rs`) turns the saved choices into that command line when it starts. The window's
//! Audio panel then changes the devices and their channels while the program runs (the fork's
//! host closes its streams and opens the new ones, the window stays); a new driver, buffer size
//! or sample rate still saves the choices and starts the program again. In a DAW nothing here
//! is used: no host is installed and the editor shows no Audio button.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};

/// The buffer sizes the panel offers, in samples
pub const PERIODS: [u32; 4] = [64, 128, 256, 512];
/// The sample rates the panel offers, in Hz
pub const RATES: [u32; 4] = [44_100, 48_000, 88_200, 96_000];
/// The name of the settings file in `config_dir`
pub const FILE: &str = "amp-standalone.json";

/// What the player chose. A device that is `None` is the machine's default
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// The audio driver, as nih-plug's `--backend` names it: `auto`, `alsa`, `jack`,
    /// `wasapi`, `core-audio`
    pub backend: String,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    /// Buffer size in samples; `None` leaves it to the host's own
    pub period_size: Option<u32>,
    /// Sample rate in Hz; `None` leaves it to the host's own
    pub sample_rate: Option<u32>,
    /// The first channel (counted from 1) of the input device the amp's pair is read from
    pub input_channel: u32,
    /// The first channel (counted from 1) of the output device the amp's pair is written to
    pub output_channel: u32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            backend: "auto".into(),
            input_device: None,
            output_device: None,
            period_size: None,
            sample_rate: None,
            input_channel: 1,
            output_channel: 1,
        }
    }
}

impl Prefs {
    /// Do the two open the same stream, so that only the devices and their channels differ?
    /// Those can be changed while the program plays; a new driver, buffer size or sample rate
    /// needs it to start again
    pub fn same_stream(&self, other: &Prefs) -> bool {
        self.backend == other.backend
            && self.period_size == other.period_size
            && self.sample_rate == other.sample_rate
    }

    /// The saved choices, or the defaults for a missing or unreadable file. Values the
    /// panel does not offer are put back to the nearest it does
    pub fn load(path: &Path) -> Self {
        let mut prefs = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| Self::from_json(&text))
            .unwrap_or_default();
        prefs.period_size = prefs.period_size.map(|size| nearest(&PERIODS, size));
        prefs.sample_rate = prefs.sample_rate.map(|rate| nearest(&RATES, rate));
        prefs
    }

    /// Writes the choices, making the folder if it is not there yet
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_json())
    }

    fn to_json(&self) -> String {
        let text = |value: &Option<String>| value.as_deref().map_or("null".into(), quoted);
        format!(
            "{{\n  \"backend\": {},\n  \"input_device\": {},\n  \"output_device\": {},\n  \"period_size\": {},\n  \"sample_rate\": {},\n  \"input_channel\": {},\n  \"output_channel\": {}\n}}\n",
            quoted(&self.backend),
            text(&self.input_device),
            text(&self.output_device),
            number(self.period_size),
            number(self.sample_rate),
            self.input_channel,
            self.output_channel
        )
    }

    /// Reads the one flat object `to_json` writes; a field it lacks keeps its default and
    /// anything that is not such an object is `None`. (No JSON library: the plugin in a DAW
    /// must not gain dependencies for the standalone program's one small file.)
    fn from_json(text: &str) -> Option<Self> {
        let mut prefs = Self::default();
        let mut chars = text.trim().strip_prefix('{')?.strip_suffix('}')?.chars().peekable();
        loop {
            skip(&mut chars, |c| c.is_whitespace() || c == ',');
            if chars.peek().is_none() {
                return Some(prefs);
            }
            let key = string(&mut chars)?;
            skip(&mut chars, char::is_whitespace);
            (chars.next()? == ':').then_some(())?;
            skip(&mut chars, char::is_whitespace);
            let value = if chars.peek() == Some(&'"') {
                Some(string(&mut chars)?)
            } else {
                let mut word = String::new();
                while let Some(c) = chars.next_if(|c| !matches!(c, ',' | '}') && !c.is_whitespace())
                {
                    word.push(c);
                }
                (word != "null").then_some(word)
            };
            match (key.as_str(), value) {
                ("backend", Some(value)) => prefs.backend = value,
                ("input_device", value) => prefs.input_device = value,
                ("output_device", value) => prefs.output_device = value,
                ("period_size", value) => prefs.period_size = number_of(value)?,
                ("sample_rate", value) => prefs.sample_rate = number_of(value)?,
                // A channel counts from 1; anything else is the first
                ("input_channel", value) => prefs.input_channel = channel_of(value)?,
                ("output_channel", value) => prefs.output_channel = channel_of(value)?,
                _ => {}
            }
        }
    }

    /// What a command line of the host's asks for (`--backend`, `-b`, `--input-device`,
    /// `--output-device`, `--period-size`/`-p`, `--sample-rate`/`-r`, as `--flag value` or
    /// `--flag=value`), on top of the defaults
    pub fn from_args(args: &[String]) -> Self {
        let mut prefs = Self::default();
        let mut args = args.iter().skip(1);
        while let Some(arg) = args.next() {
            let (flag, inline) = match arg.split_once('=') {
                Some((flag, value)) => (flag, Some(value.to_owned())),
                None => (arg.as_str(), None),
            };
            let known = matches!(
                flag,
                "--backend"
                    | "-b"
                    | "--input-device"
                    | "--output-device"
                    | "--period-size"
                    | "-p"
                    | "--sample-rate"
                    | "-r"
                    | "--input-channel"
                    | "--output-channel"
            );
            if !known {
                continue;
            }
            let Some(value) = inline.or_else(|| args.next().cloned()) else {
                break;
            };
            match flag {
                "--backend" | "-b" => prefs.backend = value,
                "--input-device" => prefs.input_device = Some(value),
                "--output-device" => prefs.output_device = Some(value),
                "--period-size" | "-p" => prefs.period_size = value.parse().ok(),
                "--input-channel" => prefs.input_channel = value.parse().unwrap_or(1).max(1),
                "--output-channel" => prefs.output_channel = value.parse().unwrap_or(1).max(1),
                _ => prefs.sample_rate = value.parse().ok(),
            }
        }
        prefs
    }

    /// The command line of nih-plug's standalone host for these choices, program name first
    pub fn args(&self, program: &str) -> Vec<String> {
        let mut args = vec![program.to_owned(), "--backend".into(), self.backend.clone()];
        for (flag, value) in
            [("--period-size", self.period_size), ("--sample-rate", self.sample_rate)]
        {
            if let Some(value) = value {
                args.push(flag.into());
                args.push(value.to_string());
            }
        }
        for (flag, channel) in
            [("--input-channel", self.input_channel), ("--output-channel", self.output_channel)]
        {
            if channel != 1 {
                args.push(flag.into());
                args.push(channel.to_string());
            }
        }
        for (flag, device) in
            [("--input-device", &self.input_device), ("--output-device", &self.output_device)]
        {
            if let Some(device) = device {
                // One argument, so a name that starts with a dash is not read as a flag
                args.push(format!("{flag}={device}"));
            }
        }
        args
    }
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// A number or `null` read back: `None` for something else
fn number_of(value: Option<String>) -> Option<Option<u32>> {
    match value {
        None => Some(None),
        Some(text) => text.parse().ok().map(Some),
    }
}

/// A channel number read back (`Some` of the first channel for 0 or `null`): `None` for
/// something that is no number
fn channel_of(value: Option<String>) -> Option<u32> {
    number_of(value).map(|number| number.unwrap_or(1).max(1))
}

fn number(value: Option<u32>) -> String {
    value.map_or("null".into(), |value| value.to_string())
}

fn quoted(text: &str) -> String {
    let mut out = String::from('"');
    for c in text.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn skip(chars: &mut Chars, skipped: impl Fn(char) -> bool) {
    while chars.next_if(|&c| skipped(c)).is_some() {}
}

/// A quoted string at the front of `chars`
fn string(chars: &mut Chars) -> Option<String> {
    (chars.next()? == '"').then_some(())?;
    let mut out = String::new();
    loop {
        match chars.next()? {
            '"' => return Some(out),
            '\\' => out.push(match chars.next()? {
                'n' => '\n',
                c => c,
            }),
            c => out.push(c),
        }
    }
}

fn nearest(list: &[u32], value: u32) -> u32 {
    list.iter().copied().min_by_key(|&entry| entry.abs_diff(value)).unwrap_or(value)
}

/// What the program plays on now, which the Audio panel opens on: what it was started with,
/// until the devices are changed while it runs
static LAUNCH: Mutex<Option<Prefs>> = Mutex::new(None);

/// Tells the panel what the program plays on: at the start, and after each change of devices
pub fn set_launch(prefs: Prefs) {
    *LAUNCH.lock().unwrap_or_else(|e| e.into_inner()) = Some(prefs);
}

/// What the program plays on: the settings saved, or the arguments given, or the devices
/// chosen since
pub fn launch() -> Prefs {
    LAUNCH.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default()
}

/// The saved choices with the defaults of this machine put in: an input is only connected
/// when named (nih-plug does not connect one by itself, to keep a microphone from feeding
/// back), a device that is no longer plugged in is left to the default, and a driver the
/// machine lacks is `auto`. `drivers`, `inputs` and `outputs` are what is plugged in, `default_input` the machine's own pick. JACK connects
/// by port, so it is left as it is
pub fn resolve(
    mut prefs: Prefs,
    drivers: &[String],
    inputs: &[String],
    outputs: &[String],
    default_input: Option<String>,
) -> Prefs {
    // A driver this machine does not have (a file from another system) would make the host
    // quit on its command line, so it is `auto` instead
    if !drivers.contains(&prefs.backend) {
        prefs.backend = "auto".into();
    }
    if prefs.backend != "jack" {
        // Channels belong to a device: a device that is gone takes its channels with it
        if prefs.input_device.as_ref().is_none_or(|name| !inputs.contains(name)) {
            prefs.input_channel = 1;
        }
        if prefs.output_device.as_ref().is_none_or(|name| !outputs.contains(name)) {
            prefs.output_channel = 1;
        }
        prefs.input_device =
            prefs.input_device.filter(|name| inputs.contains(name)).or(default_input);
        prefs.output_device = prefs.output_device.filter(|name| outputs.contains(name));
    }
    prefs
}

static RESTART: AtomicBool = AtomicBool::new(false);

/// Asks for the program to be started again once it has closed, with the settings saved
pub fn request_restart() {
    RESTART.store(true, Ordering::SeqCst);
}

/// Was a restart asked for
pub fn restart_requested() -> bool {
    RESTART.load(Ordering::SeqCst)
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

/// TEMPORARY, to find a hang on a change of devices: where the trace is written, besides stderr
pub fn trace_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("reconfigure-trace.log"))
}

/// TEMPORARY, to find a hang on a change of devices: a timestamped line on stderr and appended
/// to `trace_path()`, the same file the fork's host writes to. Always on for now; to be removed
pub fn trace(args: std::fmt::Arguments) {
    use std::io::Write;
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let thread = std::thread::current();
    let line = format!(
        "{}.{:03} [{}] amp: {}\n",
        now.as_secs(),
        now.subsec_millis(),
        thread.name().unwrap_or("unnamed"),
        args
    );
    eprint!("{line}");
    if let Some(path) = trace_path() {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(line.as_bytes());
        }
    }
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

/// The answer of a change of devices: `Ok` once they play, `Err` with why they would not open
pub type Answer = Receiver<Result<(), String>>;

/// What the program says to a request to change the devices while it plays
#[derive(Debug)]
pub enum Reconfigure {
    /// It cannot (another driver, buffer size or sample rate; or a driver that cannot): the
    /// program has to start again
    Unsupported,
    /// It is opening them: the answer comes on the receiver, `Ok` when the new devices play
    /// and `Err` with what went wrong when they would not open (the old ones then play again)
    Opening(Answer),
}

/// Passes an answer on, running `on_ok` first when it is `Ok`: what was chosen is kept (saved,
/// and noted as what plays) only once the new devices play, never when they would not open
pub fn after_ok(answer: Answer, on_ok: impl FnOnce() + Send + 'static) -> Answer {
    let (sender, passed) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        trace(format_args!("after_ok: waiting for the host's answer"));
        let result = answer.recv().unwrap_or_else(|_| Err("the audio stopped".into()));
        trace(format_args!("after_ok: the host answered {result:?}"));
        if result.is_ok() {
            on_ok();
            trace(format_args!("after_ok: saved and noted"));
        }
        let _ = sender.send(result);
        trace(format_args!("after_ok: passed on to the panel"));
    });
    passed
}

/// The program's side of the Audio panel: it knows the machine's drivers and devices and
/// how to start the program again with new choices
pub trait Host: Send + Sync {
    /// The drivers, as the `--backend` names them
    fn drivers(&self) -> Vec<String>;
    /// The devices of one driver
    fn devices(&self, driver: &str) -> DriverDevices;
    /// Changes the devices and channels to `prefs` while the program plays, and saves `prefs`
    /// for the next start once they open. `Unsupported` (the default) when that is not
    /// possible, and the panel then uses `restart`
    fn reconfigure(&self, _prefs: &Prefs) -> Reconfigure {
        Reconfigure::Unsupported
    }
    /// Saves `prefs` and asks for the program to start again with them once the window has
    /// closed and let go of the audio device. The settings file then wins over any
    /// arguments the program was started with
    fn restart(&self, prefs: &Prefs) -> std::io::Result<()>;
    /// How many channels a device has (`None`: the driver's default input or output). The
    /// panel offers the amp's stereo pairs of them. Two where the driver cannot say
    fn channels(&self, _driver: &str, _input: bool, _device: Option<&str>) -> u32 {
        2
    }
    /// Turns the test tone on the chosen outputs on or off, at once and without a restart
    fn set_test_tone(&self, _on: bool) {}
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
    /// The channels of the chosen input and output device
    input_channels: u32,
    output_channels: u32,
    /// Is the test tone sounding. Not kept between launches
    pub test_tone: bool,
    /// What `restart` or the new devices said when they failed
    pub error: Option<String>,
    /// The answer of a change of devices that is being opened. Shared, so the choice stays
    /// `Clone`
    opening: Option<Arc<Mutex<Answer>>>,
    /// The devices of the last Apply are open and playing
    pub applied: bool,
    /// Looks at the answer so far, for the trace
    polls: u32,
}

/// What Apply came to
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// The program starts again: the window is done
    Restarting,
    /// The new devices are being opened: the window stays, and asks `poll` for the answer
    Opening,
    /// Nothing was done; `error` says why
    Failed,
}

/// The amp's audio is a stereo pair, so the device's channels are offered two at a time
const PAIR: u32 = 2;

/// Shown for "the machine's default device" and when a driver has no device
pub const DEFAULT_DEVICE: &str = "Default";
pub const NO_DEVICE: &str = "None";

impl AudioChoice {
    /// Opens the panel on `prefs`, listing `host`'s drivers and the devices of the driver
    /// that is chosen
    pub fn open(host: &dyn Host, mut prefs: Prefs) -> Self {
        let mut drivers = host.drivers();
        if !drivers.contains(&prefs.backend) {
            drivers.insert(0, prefs.backend.clone());
        }
        let devices = host.devices(&prefs.backend);
        // A device that is not plugged in plays on the default, and the panel says so
        if prefs.backend != "jack" {
            prefs.input_device = prefs.input_device.filter(|name| devices.inputs.contains(name));
            prefs.output_device = prefs.output_device.filter(|name| devices.outputs.contains(name));
        }
        let mut choice = Self {
            drivers,
            devices,
            prefs,
            input_channels: PAIR,
            output_channels: PAIR,
            test_tone: false,
            error: None,
            opening: None,
            applied: false,
            polls: 0,
        };
        choice.count_channels(host);
        // A channel the device does not have (a file from another interface) is the first
        choice.prefs.input_channel = fit_pair(choice.prefs.input_channel, choice.input_channels);
        choice.prefs.output_channel = fit_pair(choice.prefs.output_channel, choice.output_channels);
        choice
    }

    fn count_channels(&mut self, host: &dyn Host) {
        let (backend, prefs) = (self.prefs.backend.as_str(), &self.prefs);
        self.input_channels = host.channels(backend, true, prefs.input_device.as_deref());
        self.output_channels = host.channels(backend, false, prefs.output_device.as_deref());
    }

    /// The eight rows of the panel: caption and what shows
    pub fn rows(&self) -> [(&'static str, String); 8] {
        [
            ("Driver", self.prefs.backend.to_uppercase()),
            ("Input", shown(self.prefs.input_device.as_deref(), &self.devices.inputs)),
            ("Input channels", pair_name(self.prefs.input_channel, self.input_channels)),
            ("Output", shown(self.prefs.output_device.as_deref(), &self.devices.outputs)),
            ("Output channels", pair_name(self.prefs.output_channel, self.output_channels)),
            (
                "Buffer",
                self.prefs.period_size.map_or(DEFAULT_DEVICE.into(), |n| format!("{n} samples")),
            ),
            (
                "Sample rate",
                self.prefs.sample_rate.map_or(DEFAULT_DEVICE.into(), |n| format!("{n} Hz")),
            ),
            ("Test tone", if self.test_tone { "On, 440 Hz" } else { "Off" }.into()),
        ]
    }

    /// Moves row `row` (see `rows`) to the next entry of its list, or the previous one. The
    /// test tone row switches the tone at once; the others wait for Apply
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
                self.prefs.input_channel = 1;
                self.prefs.output_channel = 1;
            }
            1 => {
                step_device(&mut self.prefs.input_device, &self.devices.inputs, forward);
                self.prefs.input_channel = 1;
            }
            2 => {
                self.prefs.input_channel =
                    step_pair(self.prefs.input_channel, self.input_channels, forward)
            }
            3 => {
                step_device(&mut self.prefs.output_device, &self.devices.outputs, forward);
                self.prefs.output_channel = 1;
            }
            4 => {
                self.prefs.output_channel =
                    step_pair(self.prefs.output_channel, self.output_channels, forward)
            }
            5 => self.prefs.period_size = step_list(&PERIODS, self.prefs.period_size, forward),
            6 => self.prefs.sample_rate = step_list(&RATES, self.prefs.sample_rate, forward),
            _ => {
                self.test_tone = !self.test_tone;
                host.set_test_tone(self.test_tone);
            }
        }
        if row <= 4 {
            self.count_channels(host);
        }
    }

    /// Apply: changes the devices while the program plays if it can, else starts it again
    pub fn apply(&mut self, host: &dyn Host) -> Applied {
        self.error = None;
        self.applied = false;
        if self.opening.is_some() {
            // The last one is still being opened
            return Applied::Opening;
        }
        trace(format_args!("panel: Apply, {:?}", self.prefs));
        let reconfigure = host.reconfigure(&self.prefs);
        trace(format_args!(
            "panel: the host says {}",
            if matches!(reconfigure, Reconfigure::Opening(_)) { "opening" } else { "unsupported" }
        ));
        match reconfigure {
            Reconfigure::Opening(answer) => {
                self.polls = 0;
                self.opening = Some(Arc::new(Mutex::new(answer)));
                Applied::Opening
            }
            Reconfigure::Unsupported => match host.restart(&self.prefs) {
                Ok(()) => Applied::Restarting,
                Err(error) => {
                    self.error = Some(format!("Could not restart: {error}"));
                    Applied::Failed
                }
            },
        }
    }

    /// Is a change of devices being opened
    pub fn is_opening(&self) -> bool {
        self.opening.is_some()
    }

    /// Looks for the answer of the devices being opened. Returns whether they still are
    pub fn poll(&mut self) -> bool {
        let Some(answer) = self.opening.clone() else {
            return false;
        };
        let received = answer.lock().unwrap_or_else(|e| e.into_inner()).try_recv();
        self.polls += 1;
        if self.polls == 1 || self.polls.is_multiple_of(40) {
            trace(format_args!("panel: poll number {}: {received:?}", self.polls));
        }
        match received {
            Err(TryRecvError::Empty) => return true,
            Ok(Ok(())) => self.applied = true,
            Ok(Err(error)) => {
                self.error = Some(format!("Could not open them ({error}), the old ones play"))
            }
            Err(TryRecvError::Disconnected) => {
                self.error = Some("The audio stopped while changing".into())
            }
        }
        trace(format_args!("panel: the answer reached the panel after {} polls", self.polls));
        self.opening = None;
        false
    }

    /// The panel is closing: the tone must not be left sounding
    pub fn close(&mut self, host: &dyn Host) {
        if self.test_tone {
            self.test_tone = false;
            host.set_test_tone(false);
        }
    }
}

/// The first channels of the pairs a device with `channels` channels offers: 1, 3, 5...
/// (at least the first, whatever the device says)
fn pairs(channels: u32) -> Vec<u32> {
    (1..channels.max(PAIR)).step_by(PAIR as usize).collect()
}

/// `first` if the device has the pair that starts there, else the first
fn fit_pair(first: u32, channels: u32) -> u32 {
    if pairs(channels).contains(&first) {
        first
    } else {
        1
    }
}

fn pair_name(first: u32, channels: u32) -> String {
    format!("{}-{}", fit_pair(first, channels), fit_pair(first, channels) + 1)
}

fn step_pair(first: u32, channels: u32, forward: bool) -> u32 {
    let list = pairs(channels);
    let at = list.iter().position(|&entry| entry == first).unwrap_or(0);
    list[around(at, list.len(), forward)]
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

/// The entries are the host's own (`None`) and then the list
fn step_list(list: &[u32], value: Option<u32>, forward: bool) -> Option<u32> {
    let at = value.map_or(0, |value| 1 + list.iter().position(|&e| e == value).unwrap_or(0));
    match around(at, list.len() + 1, forward) {
        0 => None,
        entry => Some(list[entry - 1]),
    }
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
        fn channels(&self, _: &str, input: bool, device: Option<&str>) -> u32 {
            // The Scarlett has four inputs and six outputs, everything else a pair
            match (input, device) {
                (true, Some("Scarlett")) => 4,
                (false, Some("Scarlett")) => 6,
                _ => 2,
            }
        }
        fn set_test_tone(&self, on: bool) {
            TONE.store(on, std::sync::atomic::Ordering::SeqCst);
        }
    }

    static TONE: AtomicBool = AtomicBool::new(false);

    fn dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hojt-standalone-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn test_a_choice_never_made_passes_no_flag_and_a_launch_is_read_back() {
        let args = Prefs::default().args("amp");
        assert_eq!(args, ["amp", "--backend", "auto"]);
        let launch: Vec<String> =
            ["amp", "-b", "alsa", "--input-device=Mic", "-p", "128"].map(String::from).to_vec();
        let prefs = Prefs::from_args(&launch);
        assert_eq!(
            (prefs.backend.as_str(), prefs.input_device.as_deref(), prefs.period_size),
            ("alsa", Some("Mic"), Some(128))
        );
        assert_eq!(prefs.sample_rate, None);
        let channels: Vec<String> =
            ["amp", "--input-channel", "3", "--output-channel=0"].map(String::from).to_vec();
        let prefs = Prefs::from_args(&channels);
        assert_eq!((prefs.input_channel, prefs.output_channel), (3, 1));
    }

    #[test]
    fn test_the_panel_shows_the_default_for_a_device_that_is_unplugged() {
        let prefs = Prefs {
            backend: "wasapi".into(),
            input_device: Some("Unplugged".into()),
            output_device: Some("Speakers".into()),
            ..Prefs::default()
        };
        let choice = AudioChoice::open(&Fake, prefs);
        assert_eq!(choice.prefs.input_device, None);
        assert_eq!(choice.prefs.output_device.as_deref(), Some("Speakers"));
    }

    #[test]
    fn test_awkward_names_survive_the_file() {
        let path = dir("names").join(FILE);
        let prefs = Prefs { input_device: Some("Mic \"A\" \\ B, {1}".into()), ..Prefs::default() };
        prefs.save(&path).unwrap();
        assert_eq!(Prefs::load(&path), prefs);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_a_device_that_is_gone_falls_back_and_the_input_defaults() {
        let list = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        let (inputs, outputs) = (list(&["Scarlett", "Mic"]), list(&["Speakers"]));
        let saved = Prefs {
            input_device: Some("Unplugged".into()),
            output_device: Some("Gone".into()),
            ..Prefs::default()
        };
        let drivers = list(&["auto", "wasapi", "jack"]);
        let resolved = resolve(saved, &drivers, &inputs, &outputs, Some("Mic".into()));
        assert_eq!(resolved.input_device.as_deref(), Some("Mic"));
        assert_eq!(resolved.output_device, None);
        // A device that is there is kept, and with none chosen the default input is used
        let kept = Prefs {
            input_device: Some("Scarlett".into()),
            output_device: Some("Speakers".into()),
            ..Prefs::default()
        };
        let resolved = resolve(kept.clone(), &drivers, &inputs, &outputs, Some("Mic".into()));
        assert_eq!(resolved, kept);
        let none = resolve(Prefs::default(), &drivers, &inputs, &outputs, Some("Mic".into()));
        assert_eq!(none.input_device.as_deref(), Some("Mic"));
        // JACK is left alone
        let jack = Prefs { backend: "jack".into(), ..Prefs::default() };
        assert_eq!(resolve(jack.clone(), &drivers, &inputs, &outputs, Some("Mic".into())), jack);
        // A driver the machine lacks, or a made-up one, is `auto`
        for backend in ["alsa", "foo"] {
            let odd = Prefs { backend: backend.into(), ..Prefs::default() };
            let fixed = resolve(odd, &drivers, &inputs, &outputs, None);
            assert_eq!(fixed.backend, "auto");
        }
    }

    #[test]
    fn test_the_choices_come_back_after_a_restart() {
        let path = dir("roundtrip").join(FILE);
        let prefs = Prefs {
            backend: "wasapi".into(),
            input_device: Some("Scarlett".into()),
            output_device: Some("Speakers".into()),
            period_size: Some(128),
            sample_rate: Some(96_000),
            input_channel: 3,
            output_channel: 5,
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
        assert_eq!((loaded.period_size, loaded.backend.as_str()), (Some(128), "auto"));
        assert_eq!(loaded.input_device.as_deref(), Some("A"));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_the_choices_become_the_hosts_command_line() {
        let prefs = Prefs {
            backend: "wasapi".into(),
            input_device: Some("Scarlett 2i2".into()),
            output_device: None,
            period_size: Some(64),
            sample_rate: Some(44_100),
            input_channel: 3,
            output_channel: 1,
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
                "--input-channel",
                "3",
                "--input-device=Scarlett 2i2"
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
        // Nothing chosen is the host's own; stepping back from it goes round to the last
        assert_eq!(choice.prefs.period_size, None);
        choice.step(&host, 5, false);
        assert_eq!(choice.prefs.period_size, Some(512));
        choice.step(&host, 5, true);
        choice.step(&host, 5, true);
        assert_eq!(choice.prefs.period_size, Some(64));
        choice.step(&host, 6, true);
        choice.step(&host, 6, true);
        assert_eq!(choice.prefs.sample_rate, Some(48_000));
    }

    #[test]
    fn test_channels_come_in_pairs_of_the_device_chosen() {
        assert_eq!(pairs(0), [1]);
        assert_eq!(pairs(2), [1]);
        assert_eq!(pairs(3), [1]);
        assert_eq!(pairs(4), [1, 3]);
        assert_eq!(pairs(6), [1, 3, 5]);
        assert_eq!((fit_pair(5, 4), fit_pair(3, 4), fit_pair(2, 4)), (1, 3, 1));
        assert_eq!(pair_name(3, 6), "3-4");

        let host = Fake;
        let prefs = Prefs { backend: "wasapi".into(), ..Prefs::default() };
        let mut choice = AudioChoice::open(&host, prefs);
        // The default input is a pair: nothing to step to
        choice.step(&host, 2, true);
        assert_eq!(choice.rows()[2].1, "1-2");
        // The Scarlett has two pairs in and three out, and each list goes round
        choice.step(&host, 1, true);
        assert_eq!(choice.prefs.input_device.as_deref(), Some("Scarlett"));
        choice.step(&host, 2, true);
        assert_eq!(choice.rows()[2].1, "3-4");
        choice.step(&host, 2, true);
        assert_eq!(choice.rows()[2].1, "1-2");
        choice.step(&host, 2, false);
        assert_eq!(choice.prefs.input_channel, 3);
        choice.step(&host, 3, true);
        assert_eq!(choice.prefs.output_device.as_deref(), Some("Speakers"));
        choice.prefs.output_device = Some("Scarlett".into());
        choice.count_channels(&host);
        for _ in 0..2 {
            choice.step(&host, 4, true);
        }
        assert_eq!(choice.rows()[4].1, "5-6");
        // A different device is a different set of channels: back to the first pair
        choice.step(&host, 1, true);
        assert_eq!((choice.prefs.input_channel, choice.prefs.output_channel), (1, 5));
        choice.step(&host, 3, true);
        assert_eq!(choice.prefs.output_channel, 1);
        // A saved channel the device lacks opens as the first pair
        let saved = Prefs {
            backend: "wasapi".into(),
            input_device: Some("Mic".into()),
            input_channel: 3,
            ..Prefs::default()
        };
        let choice = AudioChoice::open(&host, saved);
        assert_eq!(choice.prefs.input_channel, 1);
    }

    #[test]
    fn test_the_test_tone_is_on_until_the_panel_closes() {
        let host = Fake;
        let mut choice = AudioChoice::open(&host, Prefs::default());
        assert_eq!(choice.rows()[7], ("Test tone", "Off".to_string()));
        choice.step(&host, 7, true);
        assert!(choice.test_tone && TONE.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(choice.rows()[7].1, "On, 440 Hz");
        choice.close(&host);
        assert!(!choice.test_tone && !TONE.load(std::sync::atomic::Ordering::SeqCst));
        // Closing with the tone off asks for nothing
        choice.close(&host);
    }

    #[test]
    fn test_an_old_file_without_channels_opens_on_the_first_pair() {
        let path = dir("old").join(FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"backend": "wasapi", "input_device": "Scarlett"}"#).unwrap();
        let loaded = Prefs::load(&path);
        assert_eq!((loaded.input_channel, loaded.output_channel), (1, 1));
        std::fs::write(&path, r#"{"input_channel": 0, "output_channel": 3}"#).unwrap();
        let loaded = Prefs::load(&path);
        assert_eq!((loaded.input_channel, loaded.output_channel), (1, 3));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_a_device_that_is_gone_takes_its_channels_with_it() {
        let list = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        let drivers = list(&["auto", "wasapi"]);
        let saved = Prefs {
            input_device: Some("Gone".into()),
            input_channel: 3,
            output_device: Some("Scarlett".into()),
            output_channel: 5,
            ..Prefs::default()
        };
        let resolved = resolve(saved, &drivers, &list(&["Mic"]), &list(&["Scarlett"]), None);
        assert_eq!((resolved.input_channel, resolved.output_channel), (1, 5));
    }

    /// A host that answers a change of devices as it is told to, and counts the restarts
    struct Live {
        answer: Mutex<Option<Result<(), String>>>,
        live: bool,
        restarts: std::sync::atomic::AtomicU32,
    }
    impl Live {
        fn new(live: bool, answer: Result<(), String>) -> Self {
            Self { answer: Mutex::new(Some(answer)), live, restarts: Default::default() }
        }
    }
    impl Host for Live {
        fn drivers(&self) -> Vec<String> {
            vec!["wasapi".into()]
        }
        fn devices(&self, _: &str) -> DriverDevices {
            DriverDevices { inputs: vec!["Mic".into()], outputs: vec!["Speakers".into()] }
        }
        fn reconfigure(&self, _: &Prefs) -> Reconfigure {
            if !self.live {
                return Reconfigure::Unsupported;
            }
            let (sender, receiver) = std::sync::mpsc::channel();
            if let Some(answer) = self.answer.lock().unwrap().take() {
                sender.send(answer).unwrap();
            }
            // The sender goes when this returns: an answer already waits in the channel
            Reconfigure::Opening(receiver)
        }
        fn restart(&self, _: &Prefs) -> std::io::Result<()> {
            self.restarts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    fn wasapi() -> Prefs {
        Prefs { backend: "wasapi".into(), ..Prefs::default() }
    }

    #[test]
    fn test_only_the_devices_and_channels_can_change_while_playing() {
        let base = wasapi();
        let other_device =
            Prefs { output_device: Some("Speakers".into()), output_channel: 3, ..base.clone() };
        assert!(base.same_stream(&other_device));
        for different in [
            Prefs { backend: "asio".into(), ..base.clone() },
            Prefs { period_size: Some(64), ..base.clone() },
            Prefs { sample_rate: Some(96_000), ..base.clone() },
        ] {
            assert!(!base.same_stream(&different));
        }
    }

    #[test]
    fn test_apply_opens_the_new_devices_without_a_restart() {
        let host = Live::new(true, Ok(()));
        let mut choice = AudioChoice::open(&host, wasapi());
        assert_eq!(choice.apply(&host), Applied::Opening);
        assert!(choice.is_opening(), "the panel shows Opening ... until the answer");
        assert!(!choice.poll(), "the answer has come");
        assert!(choice.applied && choice.error.is_none() && !choice.is_opening());
        assert_eq!(host.restarts.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn test_a_failing_reconfigure_keeps_the_old_settings_and_shows_the_error() {
        let host = Live::new(true, Err("no such device".into()));
        let before = wasapi();
        let mut choice = AudioChoice::open(&host, before.clone());
        assert_eq!(choice.apply(&host), Applied::Opening);
        assert!(!choice.poll());
        assert!(!choice.applied);
        let error = choice.error.clone().expect("the error is shown");
        assert!(error.contains("no such device") && error.contains("old ones play"), "{error}");
        // Nothing was restarted
        assert_eq!(host.restarts.load(std::sync::atomic::Ordering::SeqCst), 0);
        // The panel is not stuck: Apply can be tried again
        assert!(!choice.is_opening());
    }

    #[test]
    fn test_what_cannot_change_while_playing_starts_the_amp_again() {
        let host = Live::new(false, Ok(()));
        let mut choice = AudioChoice::open(&host, wasapi());
        assert_eq!(choice.apply(&host), Applied::Restarting);
        assert_eq!(host.restarts.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn test_the_choice_is_kept_only_after_the_devices_open() {
        let kept = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let keep = |kept: &Arc<std::sync::atomic::AtomicU32>| {
            let kept = kept.clone();
            move || {
                kept.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        };
        let (sender, answer) = std::sync::mpsc::channel();
        sender.send(Err("no such device".to_string())).unwrap();
        let passed = after_ok(answer, keep(&kept));
        assert_eq!(passed.recv().unwrap(), Err("no such device".to_string()));
        assert_eq!(kept.load(std::sync::atomic::Ordering::SeqCst), 0, "nothing kept on an error");
        // The program ending without an answer is an error too
        let (sender, answer) = std::sync::mpsc::channel::<Result<(), String>>();
        drop(sender);
        assert!(after_ok(answer, keep(&kept)).recv().unwrap().is_err());
        assert_eq!(kept.load(std::sync::atomic::Ordering::SeqCst), 0);
        let (sender, answer) = std::sync::mpsc::channel();
        sender.send(Ok(())).unwrap();
        // `on_ok` has run by the time the panel hears Ok
        assert_eq!(after_ok(answer, keep(&kept)).recv().unwrap(), Ok(()));
        assert_eq!(kept.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn test_the_resolved_launch_is_the_same_stream_as_the_choice_opened_on_it() {
        // `main` puts the resolved settings in `set_launch`, the panel opens on them and Apply
        // compares what it chose with them: resolving changes devices, never the stream
        let drivers = ["auto".to_string(), "wasapi".to_string()];
        let saved = Prefs {
            backend: "wasapi".into(),
            period_size: Some(128),
            sample_rate: Some(96_000),
            input_device: Some("Gone".into()),
            ..Prefs::default()
        };
        let launch = resolve(
            saved.clone(),
            &drivers,
            &["Mic".into()],
            &["Speakers".into()],
            Some("Mic".into()),
        );
        assert!(launch.same_stream(&saved));
        let host = Live::new(true, Ok(()));
        let choice = AudioChoice::open(&host, launch.clone());
        assert!(
            choice.prefs.same_stream(&launch),
            "the panel changes no stream setting on opening"
        );
        // Only another driver on this machine is changed by resolving, and then it is the
        // resolved one that plays and that the panel starts from
        let other = Prefs { backend: "alsa".into(), ..saved };
        let launch = resolve(other, &drivers, &[], &[], None);
        assert_eq!(launch.backend, "auto");
        assert!(AudioChoice::open(&host, launch.clone()).prefs.same_stream(&launch));
    }
}
