//! MIDI learn and the bindings (#479, milestone 2): which controller moves which knob.
//!
//! A footcontroller's expression pedal sends control changes whose numbers are set on the unit,
//! so nothing here assumes a number: the player pairs them. With learn on, a click on a knob (or
//! on a pedal's footswitch) picks it, and the next control change that comes in is bound to it.
//! The bindings are a controller number and the parameter's id, which is what is saved
//! (`standalone::Prefs::midi_bindings`: the controller belongs to the player's rig, not to a
//! project). A bound knob follows its controller, as one gesture for the host each time; a bound
//! switch toggles on a press (a value of 64 and up) and ignores the release.
//!
//! Everything here runs in the editor: the audio thread only keeps the controllers' values
//! (`midi::MidiInbox`).

use std::sync::Arc;

use nih_plug::prelude::{ParamPtr, ParamSetter, Params};
use suite_common::ui::learn;

use crate::presets::{set_together, Edit};
use crate::GuitarAmpParams;

/// A controller's value from which a footswitch counts as pressed
const PRESSED: f32 = 0.5;

/// Which controller moves which parameter: a controller number (0 to 127) and a parameter id
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bindings(Vec<(u8, String)>);

impl Bindings {
    /// Binds controller `cc` to the parameter `id`, and takes both off whatever they were
    /// bound to: a controller moves one thing and a thing has one controller
    pub fn bind(&mut self, cc: u8, id: &str) {
        self.0.retain(|(other, bound)| *other != cc && bound != id);
        self.0.push((cc, id.to_owned()));
    }

    /// Forgets the controller of the parameter `id`; whether it had one
    pub fn forget(&mut self, id: &str) -> bool {
        let before = self.0.len();
        self.0.retain(|(_, bound)| bound != id);
        self.0.len() != before
    }

    /// The parameter controller `cc` moves
    pub fn param_of(&self, cc: u8) -> Option<&str> {
        self.0.iter().find(|(other, _)| *other == cc).map(|(_, id)| id.as_str())
    }

    /// The controller that moves the parameter `id`
    pub fn cc_of(&self, id: &str) -> Option<u8> {
        self.0.iter().find(|(_, bound)| bound == id).map(|(cc, _)| *cc)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// What the settings file holds: `7:in_gain;11:out_level`
    pub fn encode(&self) -> String {
        let pairs: Vec<String> = self.0.iter().map(|(cc, id)| format!("{cc}:{id}")).collect();
        pairs.join(";")
    }

    /// The bindings of `encode`'s text. A pair that is not a controller number from 0 to 127, a
    /// colon and an id is left out
    pub fn decode(text: &str) -> Self {
        let mut bindings = Self::default();
        for pair in text.split(';') {
            let Some((cc, id)) = pair.split_once(':') else {
                continue;
            };
            let id = id.trim();
            if let (Ok(cc), false) = (cc.trim().parse::<u8>(), id.is_empty()) {
                if cc < 128 {
                    bindings.bind(cc, id);
                }
            }
        }
        bindings
    }
}

/// Where learn stands
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Learn {
    #[default]
    Off,
    /// On: a click on a knob or a switch picks it
    Waiting,
    /// A control is picked, by its parameter's id, and the next control change binds to it
    Armed(String),
}

/// What a control change did
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing is bound to it
    Ignored,
    /// It moved or toggled the parameter bound to it
    Moved,
    /// It was bound to the parameter learn had picked
    Learned,
}

/// The bindings and learn, as the editor holds them
#[derive(Clone, Debug, Default)]
pub struct MidiMap {
    pub bindings: Bindings,
    pub learn: Learn,
    /// The places of the parameters that have a controller, for the controls to ring
    bound: Arc<Vec<usize>>,
}

/// The parameter with this id, as a pointer
fn param_of(params: &GuitarAmpParams, id: &str) -> Option<ParamPtr> {
    params.param_map().into_iter().find(|(each, _, _)| each == id).map(|(_, ptr, _)| ptr)
}

/// The id of the parameter at `address` (`learn::address`)
fn id_at(params: &GuitarAmpParams, address: usize) -> Option<String> {
    params
        .param_map()
        .into_iter()
        .find(|(_, ptr, _)| learn::address(*ptr) == address)
        .map(|(id, _, _)| id)
}

impl MidiMap {
    pub fn new(bindings: Bindings, params: &GuitarAmpParams) -> Self {
        let mut map = Self { bindings, ..Self::default() };
        map.refresh(params);
        map
    }

    /// The places of the parameters that have a controller
    pub fn bound(&self) -> Arc<Vec<usize>> {
        self.bound.clone()
    }

    pub fn is_learning(&self) -> bool {
        self.learn != Learn::Off
    }

    /// The place of the control that waits for a controller, if one does
    pub fn armed(&self, params: &GuitarAmpParams) -> Option<usize> {
        let Learn::Armed(id) = &self.learn else {
            return None;
        };
        param_of(params, id).map(learn::address)
    }

    /// The Learn button: starts learn, or ends it
    pub fn toggle_learn(&mut self) {
        self.learn = if self.is_learning() { Learn::Off } else { Learn::Waiting };
    }

    /// The control at `address` was clicked: while learn is on it waits for a controller. Returns
    /// whether it does
    pub fn pick(&mut self, params: &GuitarAmpParams, address: usize) -> bool {
        if !self.is_learning() {
            return false;
        }
        match id_at(params, address) {
            Some(id) => {
                self.learn = Learn::Armed(id);
                true
            }
            None => false,
        }
    }

    /// The control at `address` was right-clicked: forgets its controller. Returns whether it had one
    pub fn forget(&mut self, params: &GuitarAmpParams, address: usize) -> bool {
        let forgotten = id_at(params, address).is_some_and(|id| self.bindings.forget(&id));
        if forgotten {
            self.refresh(params);
        }
        forgotten
    }

    /// Control change `cc` came in with `value` (0 to 1): binds it to the control learn picked, or
    /// moves what it is bound to, through `setter` as one gesture
    pub fn control(
        &mut self,
        params: &GuitarAmpParams,
        setter: &ParamSetter,
        cc: u8,
        value: f32,
    ) -> Outcome {
        if let Learn::Armed(id) = &self.learn {
            let id = id.clone();
            self.bindings.bind(cc, &id);
            self.learn = Learn::Off;
            self.refresh(params);
            // The controller that picked it moves it from here on, this value included
            self.follow(params, setter, cc, value);
            return Outcome::Learned;
        }
        if self.follow(params, setter, cc, value) {
            Outcome::Moved
        } else {
            Outcome::Ignored
        }
    }

    fn follow(&self, params: &GuitarAmpParams, setter: &ParamSetter, cc: u8, value: f32) -> bool {
        let Some(ptr) = self.bindings.param_of(cc).and_then(|id| param_of(params, id)) else {
            return false;
        };
        if let Some(edit) = edit_for(ptr, value) {
            set_together(setter, &[edit]);
        }
        true
    }

    /// Looks the places of the bound parameters up again
    pub fn refresh(&mut self, params: &GuitarAmpParams) {
        let map = params.param_map();
        let places = self
            .bindings
            .0
            .iter()
            .filter_map(|(_, id)| map.iter().find(|(each, _, _)| each == id))
            .map(|(_, ptr, _)| learn::address(*ptr))
            .collect();
        self.bound = Arc::new(places);
    }
}

/// What a controller at `value` does to the parameter `ptr`: a switch toggles on a press and
/// does nothing on the release, anything else is set to the value
fn edit_for(ptr: ParamPtr, value: f32) -> Option<Edit> {
    if let ParamPtr::BoolParam(_) = ptr {
        if value < PRESSED {
            return None;
        }
        // SAFETY: the pointer comes from the parameters the editor draws, which outlive this call
        let on = unsafe { ptr.unmodulated_normalized_value() } > 0.5;
        return Some(Edit::raw(ptr, if on { 0.0 } else { 1.0 }));
    }
    Some(Edit::raw(ptr, value.clamp(0.0, 1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_presets::tests::Host;
    use nih_plug::prelude::{BoolParam, Param};

    fn place(param: &impl Param) -> usize {
        learn::address(param.as_ptr())
    }

    fn set_to(host: &Host) -> Vec<(ParamPtr, f32)> {
        host.set.lock().unwrap().clone()
    }

    #[test]
    fn test_a_controller_moves_one_thing_and_a_thing_has_one_controller() {
        let mut bindings = Bindings::default();
        bindings.bind(11, "in_gain");
        bindings.bind(7, "out_level");
        assert_eq!(
            (bindings.param_of(11), bindings.cc_of("out_level")),
            (Some("in_gain"), Some(7))
        );
        // The pedal moves to another knob: the first loses it
        bindings.bind(11, "out_level");
        assert_eq!(bindings.param_of(7), None);
        assert_eq!(bindings.param_of(11), Some("out_level"));
        // The knob gets another pedal: the first pedal moves nothing
        bindings.bind(20, "out_level");
        assert_eq!(bindings.param_of(11), None);
        assert_eq!(bindings.len(), 1);
        assert!(bindings.forget("out_level") && !bindings.forget("out_level"));
        assert!(bindings.is_empty());
    }

    #[test]
    fn test_bindings_round_trip_through_their_text() {
        let mut bindings = Bindings::default();
        bindings.bind(7, "in_gain");
        bindings.bind(127, "delay_on");
        bindings.bind(0, "out_level");
        assert_eq!(Bindings::decode(&bindings.encode()), bindings);
        assert_eq!(Bindings::decode(""), Bindings::default());
        // A pair that is no controller and an id is left out, the rest is kept
        let odd = Bindings::decode("7:in_gain;128:out_level;x:y;5:;nonsense;;-1:a;9: delay_on ");
        assert_eq!(
            (odd.param_of(7), odd.param_of(9), odd.len()),
            (Some("in_gain"), Some("delay_on"), 2)
        );
    }

    #[test]
    fn test_a_control_change_sets_the_knob_bound_to_it_as_one_gesture() {
        let params = GuitarAmpParams::default();
        let host = Host::default();
        let setter = ParamSetter::new(&host);
        let mut bindings = Bindings::default();
        bindings.bind(11, "in_gain");
        let mut map = MidiMap::new(bindings, &params);
        assert_eq!(map.control(&params, &setter, 11, 0.75), Outcome::Moved);
        assert_eq!(set_to(&host), [(params.in_gain.as_ptr(), 0.75)]);
        assert_eq!(
            host.told.lock().unwrap().iter().map(|(kind, _)| *kind).collect::<String>(),
            "bse"
        );
        // From one end to the other
        map.control(&params, &setter, 11, 0.0);
        map.control(&params, &setter, 11, 1.0);
        let values: Vec<f32> = set_to(&host).iter().map(|(_, v)| *v).collect();
        assert_eq!(values, [0.75, 0.0, 1.0]);
    }

    #[test]
    fn test_a_control_change_nothing_is_bound_to_does_nothing() {
        let params = GuitarAmpParams::default();
        let host = Host::default();
        let setter = ParamSetter::new(&host);
        let mut bindings = Bindings::default();
        bindings.bind(11, "in_gain");
        let mut map = MidiMap::new(bindings, &params);
        assert_eq!(map.control(&params, &setter, 12, 0.5), Outcome::Ignored);
        assert!(host.told.lock().unwrap().is_empty());
    }

    #[test]
    fn test_learn_binds_the_next_control_change_to_the_knob_picked() {
        let params = GuitarAmpParams::default();
        let host = Host::default();
        let setter = ParamSetter::new(&host);
        let mut map = MidiMap::default();
        // Not learning: a click picks nothing
        assert!(!map.pick(&params, place(&params.in_gain)));
        map.toggle_learn();
        assert_eq!(map.learn, Learn::Waiting);
        // A control change before a knob is picked binds nothing and moves nothing
        assert_eq!(map.control(&params, &setter, 20, 0.5), Outcome::Ignored);
        assert!(map.pick(&params, place(&params.in_gain)));
        assert_eq!(map.armed(&params), Some(place(&params.in_gain)));
        assert_eq!(map.control(&params, &setter, 20, 0.5), Outcome::Learned);
        assert_eq!(map.learn, Learn::Off);
        assert_eq!(map.bindings.param_of(20), Some("in_gain"));
        assert_eq!(map.bound().as_slice(), [place(&params.in_gain)]);
        // The pedal that was moved to bind it moves the knob with the value it came with
        assert_eq!(set_to(&host), [(params.in_gain.as_ptr(), 0.5)]);
        assert_eq!(map.control(&params, &setter, 20, 1.0), Outcome::Moved);
    }

    #[test]
    fn test_learn_ends_when_the_button_is_pressed_again_and_forgetting_unbinds() {
        let params = GuitarAmpParams::default();
        let mut map = MidiMap::default();
        map.toggle_learn();
        map.pick(&params, place(&params.out_level));
        map.toggle_learn();
        assert_eq!((map.learn.clone(), map.armed(&params)), (Learn::Off, None));
        map.bindings.bind(3, "out_level");
        map.refresh(&params);
        assert!(!map.forget(&params, place(&params.in_gain)));
        assert!(map.forget(&params, place(&params.out_level)));
        assert!(map.bindings.is_empty() && map.bound().is_empty());
    }

    #[test]
    fn test_a_footswitch_toggles_its_switch_on_a_press_and_not_on_the_release() {
        let host = Host::default();
        let setter = ParamSetter::new(&host);
        let mut bindings = Bindings::default();
        bindings.bind(64, "delay_on");
        for (was, value) in [(false, 1.0), (true, 1.0), (false, 64.0 / 127.0)] {
            let params = GuitarAmpParams {
                delay_on: BoolParam::new("Delay", was),
                ..GuitarAmpParams::default()
            };
            host.set.lock().unwrap().clear();
            let mut map = MidiMap::new(bindings.clone(), &params);
            assert_eq!(params.delay_on.value(), was);
            map.control(&params, &setter, 64, value);
            let want = if was { 0.0 } else { 1.0 };
            assert_eq!(set_to(&host), [(params.delay_on.as_ptr(), want)], "was {was}");
        }
        // The release, and a press short of 64, change nothing
        host.set.lock().unwrap().clear();
        let params = GuitarAmpParams::default();
        let mut map = MidiMap::new(bindings, &params);
        assert_eq!(map.control(&params, &setter, 64, 0.0), Outcome::Moved);
        assert_eq!(map.control(&params, &setter, 64, 63.0 / 127.0), Outcome::Moved);
        assert!(set_to(&host).is_empty());
    }
}
