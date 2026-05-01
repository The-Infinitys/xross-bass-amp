use truce::{Params, params::FloatParam, params::IntParam};

#[derive(Params)]
pub struct XrossBassAmpParams {
    // --- 1. Gain Section ---
    #[param(
        name = "Input Gain",
        range = "linear(-20.0, 20.0)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(50)"
    )]
    pub input_gain: FloatParam,

    #[param(
        name = "Gain",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub gain: FloatParam,

    #[param(
        name = "Grit",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub grit: FloatParam,

    #[param(
        name = "Master",
        range = "linear(-60.0, 0.0)",
        default = -6.0,
        unit = "dB",
        smooth = "exp(50)"
    )]
    pub master_gain: FloatParam,

    #[param(
        name = "Low Comp",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub low_comp: FloatParam,

    #[param(
        name = "Focus",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub focus: FloatParam,

    #[param(
        name = "Attack",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub attack: FloatParam,

    // --- 2. EQ Section ---
    #[param(
        name = "Eq Low",
        range = "linear(-18.0, 18.0)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(50)"
    )]
    pub eq_low: FloatParam,

    #[param(
        name = "Eq Mid",
        range = "linear(-18.0, 18.0)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(50)"
    )]
    pub eq_mid: FloatParam,

    #[param(
        name = "Eq High",
        range = "linear(-18.0, 18.0)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(50)"
    )]
    pub eq_high: FloatParam,

    #[param(
        name = "Presence",
        range = "linear(0.0, 18.0)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(50)"
    )]
    pub presence: FloatParam,

    #[param(
        name = "Resonance",
        range = "linear(0.0, 18.0)",
        default = 0.0,
        unit = "dB",
        smooth = "exp(50)"
    )]
    pub resonance: FloatParam,

    // --- 3. Cab Section ---
    #[param(
        name = "Speaker Size",
        range = "linear(10.0,20.0)",
        default = 15.0,
        smooth = "exp(50)"
    )]
    pub speaker_size: FloatParam,

    #[param(name = "Speaker Count", range = "linear(1, 16)", default = 4)]
    pub speaker_count: IntParam,

    #[param(
        name = "Mic A Distance",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub mic_a_distance: FloatParam,

    #[param(
        name = "Mic A Axis",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub mic_a_axis: FloatParam,

    #[param(
        name = "Mic B Distance",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub mic_b_distance: FloatParam,

    #[param(
        name = "Mic B Axis",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub mic_b_axis: FloatParam,

    #[param(
        name = "Room Size",
        range = "linear(0.0, 1.0)",
        default = 0.3,
        smooth = "exp(50)"
    )]
    pub room_size: FloatParam,

    #[param(
        name = "Room Mix",
        range = "linear(0.0, 1.0)",
        default = 0.1,
        smooth = "exp(50)"
    )]
    pub room_mix: FloatParam,

    // --- 4. Effects Section ---
    #[param(
        name = "Mix",
        range = "linear(0.0, 1.0)",
        default = 0.2,
        smooth = "exp(50)"
    )]
    pub mix: FloatParam,

    #[param(
        name = "Tight",
        range = "linear(20.0, 100.0)",
        default = 80.0,
        unit = "Hz",
        smooth = "exp(50)"
    )]
    pub tight: FloatParam,

    #[param(
        name = "DI Mix",
        range = "linear(0.0, 1.0)",
        default = 0.25,
        smooth = "exp(50)"
    )]
    pub di_mix: FloatParam,
}
