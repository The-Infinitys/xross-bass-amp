use truce::{Params, params::FloatParam, params::IntParam};

#[derive(Params)]
pub struct XrossBassAmpParams {
    // --- 1. Gain & Drive Section ---
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
        name = "Blend", // ベースにおいて非常に重要：歪みとクリーンの比率
        range = "linear(0.0, 1.0)",
        default = 1.0,
        smooth = "exp(50)"
    )]
    pub blend: FloatParam,

    #[param(
        name = "Master",
        range = "linear(-60.0, 0.0)",
        default = -6.0,
        unit = " dB",
        smooth = "exp(50)"
    )]
    pub master_gain: FloatParam,

    // --- 2. Dynamics (Added for Bass) ---
    #[param(
        name = "Compression",
        range = "linear(0.0, 1.0)",
        default = 0.0,
        smooth = "exp(50)"
    )]
    pub compression: FloatParam,

    #[param(
        name = "Noise Gate",
        range = "linear(0.0, 1.0)",
        default = 0.0,
        smooth = "exp(50)"
    )]
    pub noise_gate: FloatParam,

    // --- 3. EQ Section ---
    #[param(
        name = "Sub / Ultra Low", // 30-60Hz付近の重量感
        range = "linear(-18.0, 18.0)",
        default = 0.0,
        unit = " dB",
        smooth = "exp(50)"
    )]
    pub sub_low: FloatParam,

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
        name = "Attack / Presence", // スラップのパキパキ感などを調整
        range = "linear(0.0, 18.0)",
        default = 0.0,
        unit = " dB",
        smooth = "exp(50)"
    )]
    pub presence: FloatParam,

    // --- 4. Cab Section ---
    #[param(
        name = "Speaker Size",
        range = "linear(10.0, 18.0)", // ベース向けに10〜18インチに変更
        default = 10.0, // 4x10は定番
        unit = " inch",
        smooth = "exp(50)"
    )]
    pub speaker_size: FloatParam,

    #[param(name = "Speaker Count", range = "linear(1, 8)", default = 4)]
    pub speaker_count: IntParam,

    // マイク周りはギター版を継承
    #[param(
        name = "Mic A Distance",
        range = "linear(0.0, 1.0)",
        default = 0.2,
        smooth = "exp(50)"
    )]
    pub mic_a_distance: FloatParam,

    #[param(
        name = "Mic A Axis",
        range = "linear(0.0, 1.0)",
        default = 0.0,
        smooth = "exp(50)"
    )]
    pub mic_a_axis: FloatParam,
    #[param(
        name = "Mic B Distance",
        range = "linear(0.0, 1.0)",
        default = 0.2,
        smooth = "exp(50)"
    )]
    pub mic_b_distance: FloatParam,

    #[param(
        name = "Mic B Axis",
        range = "linear(0.0, 1.0)",
        default = 0.0,
        smooth = "exp(50)"
    )]
    pub mic_b_axis: FloatParam,
    #[param(name = "Room Mix", range = "linear(0.0, 1.0)", default = 0.0)]
    pub room_mix: FloatParam,
    // --- 5. Utilities & Effects ---
    #[param(
        name = "Cab Bypass", // DI（ダイレクトボックス）サウンドをシミュレートするため
        default = 0.0,
        range = "linear(0.0, 1.0)",
        smooth = "exp(50)"
    )]
    pub cab_bypass: FloatParam,

    #[param(
        name = "Tight",
        range = "linear(20.0, 200.0)", // ベース用なので上限を下げて調整しやすく
        default = 40.0,
        unit = " Hz",
        smooth = "exp(50)"
    )]
    pub tight: FloatParam,

    #[param(
        name = "Limiter", // 最終段でのピーク抑制
        default = 0.0,
        range = "linear(0.0, 1.0)",
        smooth = "exp(50)"
    )]
    pub limiter_on: FloatParam,
    #[param(
        name = "Sag",
        range = "linear(0.0, 1.0)",
        default = 0.0,
        smooth = "exp(50)"
    )]
    pub sag: FloatParam,
}
