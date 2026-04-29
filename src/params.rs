use truce::{Params, params::FloatParam, params::IntParam};

#[derive(Params)]
pub struct XrossGuitarAmpParams {
    // --- 1. Gain Section ---
    #[param(
        name = "Input Gain",
        range = "linear(-20.0, 20.0)",
        default = 0.0,
        unit = " dB",
        smooth = "exp(50)"
    )]
    pub input_gain: FloatParam,

    #[param(
        name = "Drive",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub drive: FloatParam,

    #[param(
        name = "Distortion",
        range = "linear(0.0, 1.0)",
        default = 0.5,
        smooth = "exp(50)"
    )]
    pub distortion: FloatParam,

    #[param(
        name = "Master",
        range = "linear(-60.0, 0.0)",
        default = -6.0,
        unit = " dB",
        smooth = "exp(50)"
    )]
    pub master_gain: FloatParam,

    // --- 2. EQ Section ---
    #[param(
        name = "Low",
        range = "linear(-18.0, 18.0)",
        default = 0.0,
        unit = " dB",
        smooth = "exp(50)"
    )]
    pub low: FloatParam,

    #[param(
        name = "Mid",
        range = "linear(-18.0, 18.0)",
        default = 0.0,
        unit = " dB",
        smooth = "exp(50)"
    )]
    pub mid: FloatParam,

    #[param(
        name = "High",
        range = "linear(-18.0, 18.0)",
        default = 0.0,
        unit = " dB",
        smooth = "exp(50)"
    )]
    pub high: FloatParam,

    #[param(
        name = "Presence",
        range = "linear(0.0, 18.0)",
        default = 0.0,
        unit = " dB",
        smooth = "exp(50)"
    )]
    pub presence: FloatParam,

    #[param(
        name = "Resonance",
        range = "linear(0.0, 18.0)",
        default = 0.0,
        unit = " dB",
        smooth = "exp(50)"
    )]
    pub resonance: FloatParam,

    // --- 3. Cab Section ---
    #[param(
        name = "Speaker Size",
        range = "linear(8.0, 15.0)",
        default = 12.0,
        unit = " inch",
        smooth = "exp(50)"
    )]
    pub speaker_size: FloatParam,

    #[param(name = "Speaker Count", range = "linear(1, 8)", default = 4)]
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
        name = "Sag",
        range = "linear(0.0, 1.0)",
        default = 0.2,
        smooth = "exp(50)"
    )]
    pub sag: FloatParam,

    #[param(
        name = "Tight",
        range = "skewed(20.0, 500.0, 0.5)",
        default = 80.0,
        unit = " Hz",
        smooth = "exp(50)"
    )]
    pub tight: FloatParam,

    #[param(
        name = "Reverb Mix",
        range = "linear(0.0, 1.0)",
        default = 0.1,
        smooth = "exp(50)"
    )]
    pub reverb_mix: FloatParam,
}
