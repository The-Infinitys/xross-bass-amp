use nih_plug::params::{FloatParam, IntParam, Params};
use nih_plug::prelude::*;
use std::sync::Arc;

#[derive(Params, Default)]
pub struct XrossGuitarAmpParams {
    /// 1. ゲインセクション
    #[nested(group = "Gain Section", id_prefix = "gain_")]
    pub gain_section: GainParams,

    /// 2. イコライジングセクション
    #[nested(group = "EQ Section", id_prefix = "eq_")]
    pub eq_section: EqParams,

    /// 3. キャビネットセクション (物理モデリング)
    #[nested(group = "Cab Section", id_prefix = "cab_")]
    pub cab_section: CabParams,

    /// 4. エフェクトセクション
    #[nested(group = "Effects Section", id_prefix = "fx_")]
    pub fx_section: EffectsParams,
}

// --- 1. Gain Section ---
#[derive(Params)]
pub struct GainParams {
    #[id = "input"]
    pub input_gain: FloatParam,
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "dist"]
    pub distortion: FloatParam,
    #[id = "master"]
    pub master_gain: FloatParam,
}

impl Default for GainParams {
    fn default() -> Self {
        Self {
            input_gain: FloatParam::new(
                "Input Gain",
                0.0,
                FloatRange::Linear {
                    min: -20.0,
                    max: 20.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(Arc::new(|x| format!("{:.1}", x))),
            drive: FloatParam::new("Drive", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(Arc::new(|x| format!("{:.2}", x))),
            distortion: FloatParam::new(
                "Distortion",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(Arc::new(|x| format!("{:.2}", x))),
            master_gain: FloatParam::new(
                "Master",
                -6.0,
                FloatRange::Linear {
                    min: -60.0,
                    max: 0.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(Arc::new(|x| format!("{:.1}", x))),
        }
    }
}

// --- 2. EQ Section ---
#[derive(Params)]
pub struct EqParams {
    #[id = "low"]
    pub low: FloatParam,
    #[id = "mid"]
    pub mid: FloatParam,
    #[id = "high"]
    pub high: FloatParam,
    #[id = "presence"]
    pub presence: FloatParam,
    #[id = "resonance"]
    pub resonance: FloatParam,
}

impl Default for EqParams {
    fn default() -> Self {
        let db = 18.0;
        Self {
            low: FloatParam::new("Low", 0.0, FloatRange::Linear { min: -db, max: db })
                .with_unit(" dB")
                .with_value_to_string(Arc::new(|x| format!("{:.0}", x))),
            mid: FloatParam::new("Mid", 0.0, FloatRange::Linear { min: -db, max: db })
                .with_unit(" dB")
                .with_value_to_string(Arc::new(|x| format!("{:.0}", x))),
            high: FloatParam::new("High", 0.0, FloatRange::Linear { min: -db, max: db })
                .with_unit(" dB")
                .with_value_to_string(Arc::new(|x| format!("{:.0}", x))),
            presence: FloatParam::new("Presence", 0.0, FloatRange::Linear { min: 0.0, max: db })
                .with_unit(" dB")
                .with_value_to_string(Arc::new(|x| format!("{:.0}", x))),
            resonance: FloatParam::new("Resonance", 0.0, FloatRange::Linear { min: 0.0, max: db })
                .with_unit(" dB")
                .with_value_to_string(Arc::new(|x| format!("{:.0}", x))),
        }
    }
}
// --- 3. Cab Section (Physical Modeling & Dual Mics) ---
#[derive(Params)]
pub struct CabParams {
    #[id = "spk_size"]
    pub speaker_size: FloatParam, // 8" to 15"

    #[id = "spk_count"]
    pub speaker_count: IntParam, // 1, 2, 4 speakers

    // --- Microphone A (Primary) ---
    #[id = "mic_a_dist"]
    pub mic_a_distance: FloatParam,
    #[id = "mic_a_axis"]
    pub mic_a_axis: FloatParam,

    // --- Microphone B (Secondary) ---
    #[id = "mic_b_dist"]
    pub mic_b_distance: FloatParam,
    #[id = "mic_b_axis"]
    pub mic_b_axis: FloatParam,

    // --- Room / Ambience ---
    #[id = "room_size"]
    pub room_size: FloatParam,
    #[id = "room_mix"]
    pub room_mix: FloatParam,
}

impl Default for CabParams {
    fn default() -> Self {
        Self {
            speaker_size: FloatParam::new(
                "Speaker Size",
                12.0,
                FloatRange::Linear {
                    min: 8.0,
                    max: 15.0,
                },
            )
            .with_unit(" inch")
            .with_value_to_string(Arc::new(|x| format!("{:.1}", x))),

            // スピーカーの数は 1, 2, 4 の切り替えを想定
            speaker_count: IntParam::new("Speaker Count", 4, IntRange::Linear { min: 1, max: 8 }),

            // Microphone A
            mic_a_distance: FloatParam::new(
                "Mic A Distance",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(Arc::new(|x| format!("{:.2}", x))),

            mic_a_axis: FloatParam::new(
                "Mic A Axis",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(Arc::new(|x| format!("{:.2}", x))),

            // Microphone B
            mic_b_distance: FloatParam::new(
                "Mic B Distance",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(Arc::new(|x| format!("{:.2}", x))),

            mic_b_axis: FloatParam::new(
                "Mic B Axis",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(Arc::new(|x| format!("{:.2}", x))),

            // Room Settings
            room_size: FloatParam::new("Room Size", 0.3, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(Arc::new(|x| format!("{:.2}", x))),

            room_mix: FloatParam::new("Room Mix", 0.1, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(Arc::new(|x| format!("{:.2}", x))),
        }
    }
}
// --- 4. Effects Section ---
#[derive(Params)]
pub struct EffectsParams {
    #[id = "sag"]
    pub sag: FloatParam,
    #[id = "tight"]
    pub tight: FloatParam,
    #[id = "reverb_mix"]
    pub reverb_mix: FloatParam,
}

impl Default for EffectsParams {
    fn default() -> Self {
        Self {
            sag: FloatParam::new("Sag", 0.2, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(Arc::new(|x| format!("{:.1}", x))),
            tight: FloatParam::new(
                "Tight",
                80.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 500.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" Hz")
            .with_value_to_string(Arc::new(|x| format!("{:.0}", x))),
            reverb_mix: FloatParam::new(
                "Reverb Mix",
                0.1,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(Arc::new(|x| format!("{:.2}", x))),
        }
    }
}

// --- Main Implementation ---
