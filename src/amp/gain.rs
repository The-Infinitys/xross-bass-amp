use crate::params::XrossGuitarAmpParams;
use std::f32::consts::PI;
use std::sync::Arc;

struct Biquad {
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn new() -> Self {
        Self { z1: 0.0, z2: 0.0 }
    }

    #[inline]
    fn process(&mut self, input: f32, a1: f32, a2: f32, b0: f32, b1: f32, b2: f32) -> f32 {
        let mut out = b0 * input + self.z1;
        // デノーマル対策
        if out.abs() < 1e-18 {
            out = 0.0;
        }
        self.z1 = b1 * input - a1 * out + self.z2;
        self.z2 = b2 * input - a2 * out;
        out
    }
}

pub struct GainProcessor {
    pub params: Arc<XrossGuitarAmpParams>,
    pre_hp: f32,
    slew_state: f32,
    dc_block: f32,
    input_dc_block: f32,
    envelope: f32,
    prev_input: f32,
    low_resonance: f32,
    post_tight: f32,
    feedback_state: f32,
    os_lpf_biquad: Biquad,
    sample_rate: f32,
}

impl GainProcessor {
    pub fn new(params: Arc<XrossGuitarAmpParams>) -> Self {
        Self {
            params,
            pre_hp: 0.0,
            slew_state: 0.0,
            dc_block: 0.0,
            input_dc_block: 0.0,
            envelope: 0.0,
            prev_input: 0.0,
            low_resonance: 0.0,
            post_tight: 0.0,
            feedback_state: 0.0,
            os_lpf_biquad: Biquad::new(),
            sample_rate: 44100.0,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.pre_hp = 0.0;
        self.slew_state = 0.0;
        self.dc_block = 0.0;
        self.input_dc_block = 0.0;
        self.envelope = 0.0;
        self.prev_input = 0.0;
        self.low_resonance = 0.0;
        self.post_tight = 0.0;
        self.feedback_state = 0.0;
        self.os_lpf_biquad = Biquad::new();
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let input_gain_db = self.params.gain_section.input_gain.value();
        let master_gain_db = self.params.gain_section.master_gain.value();
        let drive = self.params.gain_section.drive.value();
        let dist = self.params.gain_section.distortion.value();

        // 0. Input DC Block (入力段でのオフセット除去)
        let in_dc_fix = input - self.input_dc_block;
        self.input_dc_block = input + 0.998 * (self.input_dc_block - input);

        // EQ値を 0.0 ~ 1.0 に正規化 (-18dB ~ +18dB -> 0.0 ~ 1.0)
        let s_low = (self.params.eq_section.low.value() + 18.0) / 36.0;
        let s_mid = (self.params.eq_section.mid.value() + 18.0) / 36.0;
        let s_high = (self.params.eq_section.high.value() + 18.0) / 36.0;

        let sag = self.params.fx_section.sag.value();
        let tight = self.params.fx_section.tight.value();

        let input_gain = 10.0f32.powf(input_gain_db / 20.0);
        let in_signal = in_dc_fix * input_gain * 1.3; // 初段への入りを強化

        self.envelope += (in_signal.abs() - self.envelope) * 0.25;
        let current_env = self.envelope;

        let dynamic_gain = 1.0 - (current_env * sag * 0.45).min(0.5);

        let os_factor = 4;
        let mut output_sum = 0.0;
        let inv_os = 1.0 / os_factor as f32;

        for i in 0..os_factor {
            let fraction = i as f32 * inv_os;
            let sub_sample =
                (self.prev_input + (in_signal - self.prev_input) * fraction) * dynamic_gain;
            output_sum += self.drive_core(
                sub_sample,
                drive,
                dist,
                s_low,
                s_mid,
                s_high,
                tight,
                current_env,
            );
        }
        self.prev_input = in_signal;

        let raw_out = output_sum * inv_os;

        // 2次フィルタ (Butterworth LPF)
        let (a1, a2, b0, b1, b2) = self.calculate_biquad_lpf(18000.0);
        let filtered_out = self.os_lpf_biquad.process(raw_out, a1, a2, b0, b1, b2);

        // DC Block
        let out = filtered_out;
        let dc_fix = out - self.dc_block;
        self.dc_block = out + 0.996 * (self.dc_block - out);

        let master_gain = 10.0f32.powf(master_gain_db / 20.0);
        dc_fix * 0.25 * master_gain
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn drive_core(
        &mut self,
        input: f32,
        drive: f32,  // drive (0-1)
        dist: f32,   // distortion (0-1)
        s_low: f32,  // normalized low (0-1)
        s_mid: f32,  // normalized mid (0-1)
        s_high: f32, // normalized high (0-1)
        tight: f32,  // tight param (Hz)
        env: f32,
    ) -> f32 {
        // 1. DYNAMIC PRE-HP (Xross Metal logic)
        let tight_norm = (tight - 20.0) / 480.0;
        let hp_freq = 0.05 + (1.0 - s_low) * 0.18 + (tight_norm * 0.2) + (env * 0.25);
        self.pre_hp += hp_freq * (input - self.pre_hp);
        let mut x = input - self.pre_hp;

        // 2. GAIN STAGING (Metal Hot-Rodded)
        // ゲート閾値を設定し、低レベルの背景ノイズを完全にカットする
        let gate_threshold = 0.005;
        let gated_env = (env - gate_threshold).max(0.0) / (1.0 - gate_threshold);
        let noise_gate_scale = (gated_env * 22.0).min(1.0).powf(1.2);

        // gain(drive)とdistの両方を歪み量に反映。ベース倍率を22.0まで強化。
        let drive_amt = ((drive * 8.0).exp() * 22.0) * (0.8 + dist * 1.5) * noise_gate_scale;
        x *= drive_amt;

        // 3. MULTI-STAGE SATURATION (Xross Metal logic + Feedback)
        x += self.feedback_state * (0.22 + dist * 0.1); // distで絡みを強化

        // 非対称サチュレーション
        x = if x > 0.0 {
            (x * 1.2).tanh()
        } else {
            (x * 1.1).tanh() * 0.98
        };

        // Style Mid Scoop (Xross Metal logic: 3次倍音操作)
        let mid_scoop = (0.55 - s_mid).max(0.0) * (1.2 + dist * 0.8);
        if mid_scoop > 0.0 {
            x -= (x - x.powi(3)) * mid_scoop;
        }

        // Hybrid Square Blend (Xross Metal logic: エッジの硬さ)
        let soft_out = (x * 1.2).atan() * 0.9;
        let hard_limit = 0.82 - (s_high * 0.2) - (dist * 0.1);
        let hard_out = x.clamp(-hard_limit, hard_limit);

        // distをsquare_mixの支配的な要素にする
        let square_mix = (s_high * 0.4 + dist * 0.6).min(0.95);
        x = (soft_out * (1.0 - square_mix)) + (hard_out * square_mix);

        self.feedback_state = x;

        // 4. POST-PROCESSING (Xross Metal logic)
        let lpf_cutoff = 0.25 + (s_high * 0.55);
        self.post_tight += lpf_cutoff * (x - self.post_tight);
        x = self.post_tight;

        // Low Resonance (重厚感)
        let low_boost = s_low * 0.7 * (1.0 - env.min(0.75));
        self.low_resonance += 0.18 * (x - self.low_resonance);
        x += self.low_resonance * low_boost;

        // 5. SLEW RATE (アタックの質感)
        let max_step = 0.025 + (s_high * 0.9) + (dist * 0.2);
        let diff = x - self.slew_state;
        self.slew_state += diff.clamp(-max_step, max_step);

        self.slew_state
    }

    fn calculate_biquad_lpf(&self, cutoff: f32) -> (f32, f32, f32, f32, f32) {
        let ff = (cutoff / self.sample_rate).min(0.45);
        let omega = 2.0 * PI * ff;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0f32).sqrt(); // Q = 0.707

        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cs / a0;
        let a2 = (1.0 - alpha) / a0;
        let b1 = (1.0 - cs) / a0;
        let b0 = b1 * 0.5;
        let b2 = b0;

        (a1, a2, b0, b1, b2)
    }
}
