use crate::params::XrossBassAmpParams;
use std::f32::consts::PI;
use std::sync::Arc;

/// 2次フィルタ（Biquad）の状態保持用
#[derive(Default, Clone, Copy)]
pub struct Biquad {
    z1: f32,
    z2: f32,
}

impl Biquad {
    #[inline(always)]
    fn process(&mut self, input: f32, a1: f32, a2: f32, b0: f32, b1: f32, b2: f32) -> f32 {
        let out = b0 * input + self.z1;
        self.z1 = b1 * input - a1 * out + self.z2;
        self.z2 = b2 * input - a2 * out;
        out
    }
}

pub struct DarkDistortion {
    // 信号経路
    pre_hp_dist: f32,
    clank_peak: Biquad,
    feedback_state: f32,
    post_tight: f32,

    // 状態管理
    envelope: f32,
    prev_input: f32,
    low_resonance: f32,
    dc_block: f32,
    os_lpf_biquad: Biquad,
    sample_rate: f32,
}

impl DarkDistortion {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            pre_hp_dist: 0.0,
            clank_peak: Biquad::default(),
            feedback_state: 0.0,
            post_tight: 0.0,
            envelope: 0.0,
            prev_input: 0.0,
            low_resonance: 0.0,
            dc_block: 0.0,
            os_lpf_biquad: Biquad::default(),
            sample_rate,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    /// 歪み回路の核心部（ドライブ・ステージ）
    #[inline(always)]
    fn drive_core(&mut self, input: f32, params: &Arc<XrossBassAmpParams>) -> f32 {
        let drive = params.gain.value();
        let grit = params.grit.value();
        let tight = params.tight.value();
        let env = self.envelope;

        // 1. PRE-HP (歪み専用のタイトネス調整)
        // 歪み回路に入る成分のみをカットし、DI信号の低域とは干渉させない
        let tight_hz = 150.0 + tight * 900.0;
        let tight_norm = (tight_hz * 2.0 * PI / self.sample_rate).clamp(0.01, 0.9);
        self.pre_hp_dist += tight_norm * (input - self.pre_hp_dist);
        let mut x = input - self.pre_hp_dist;

        // 2. PRE-DISTORTION GAIN & CLANK
        // 指数関数的な強烈なゲイン。ベースのダイナミックレンジに合わせて調整
        let pre_gain = (drive * 8.5).exp() * 25.0;
        x *= pre_gain;

        // 攻撃的なアタック（2.6kHz付近のブースト）
        let (a1, a2, b0, b1, b2) =
            Self::calculate_peaking_eq(self.sample_rate, 2600.0, 1.2, 16.0 * params.attack.value());
        x = self.clank_peak.process(x, a1, a2, b0, b1, b2);

        // 3. MULTI-STAGE SATURATION
        // フィードバックによる回路的な「粘り」
        x += self.feedback_state * (0.45 * grit);

        // 非対称クリッピング。ベースの太さを残すため正負でカーブを変更
        x = if x > 0.0 {
            (x * 2.8).tanh() * 1.3
        } else {
            (x * 2.2).atan() * 1.15
        };

        // Grit連動のミッド・スクープ
        let scoop = (1.1 - grit).max(0.0) * 0.7;
        x -= (x - x.powi(3)) * scoop;

        self.feedback_state = x;

        // 4. POST-PROCESSING (Dynamic LPF)
        let lpf_hz = 3000.0 + (grit * 6000.0) + (env * 1500.0);
        let post_cutoff = (lpf_hz * 2.0 * PI / self.sample_rate).clamp(0.01, 0.9);
        self.post_tight += post_cutoff * (x - self.post_tight);

        self.post_tight
    }

    pub fn process_sample(&mut self, input: f32, params: &Arc<XrossBassAmpParams>) -> f32 {
        let drive = params.gain.value();
        let di_mix = params.di_mix.value();

        // エンベロープ検出
        self.envelope += (input.abs() - self.envelope) * 0.005;

        // ゲイン量に応じてオーバーサンプリング倍率を 1x -> 2x -> 4x -> 8x に動的変更
        let os_factor = if drive < 0.15 {
            1
        } else if drive < 0.45 {
            2
        } else if drive < 0.75 {
            4
        } else {
            8
        };
        let inv_os = 1.0 / os_factor as f32;

        let mut dist_sum = 0.0;
        for i in 0..os_factor {
            let fraction = i as f32 * inv_os;
            let sub_sample = self.prev_input + (input - self.prev_input) * fraction;
            dist_sum += self.drive_core(sub_sample, params);
        }
        self.prev_input = input;

        // 歪み成分の平均化と音量補正（Make-up）
        let dist_out = (dist_sum * inv_os) * 1.5;

        // --- FINAL MIXING (DI BLEND) ---
        // input = 純粋なDI信号（歪みなし、フィルタなしの芯）
        // dist_out = 歪み回路を通過したサウンド
        let mut out = (dist_out * (1.0 - di_mix)) + (input * di_mix * 1.2);

        // 最終段のレゾナンス調整
        let res_amt = params.resonance.value() * 1.4;
        self.low_resonance += 0.15 * (out - self.low_resonance);
        out += (out - self.low_resonance) * res_amt;

        // アンチエイリアシング LPF (16kHz)
        let (a1, a2, b0, b1, b2) = Self::calculate_biquad_lpf(self.sample_rate, 16000.0);
        let filtered_out = self.os_lpf_biquad.process(out, a1, a2, b0, b1, b2);

        // DC Block
        let dc_fix = filtered_out - self.dc_block;
        self.dc_block = filtered_out + 0.998 * (self.dc_block - filtered_out);

        dc_fix * 0.8 // 出力バランス調整
    }

    // --- Helper Functions ---
    fn calculate_peaking_eq(sr: f32, freq: f32, q: f32, gain_db: f32) -> (f32, f32, f32, f32, f32) {
        let a = 10.0f32.powf(gain_db / 40.0);
        let omega = 2.0 * PI * freq / sr;
        let alpha = omega.sin() / (2.0 * q);
        let a0 = 1.0 + alpha / a;
        (
            -2.0 * omega.cos() / a0,
            (1.0 - alpha / a) / a0,
            (1.0 + alpha * a) / a0,
            -2.0 * omega.cos() / a0,
            (1.0 - alpha * a) / a0,
        )
    }

    fn calculate_biquad_lpf(sr: f32, cutoff: f32) -> (f32, f32, f32, f32, f32) {
        let omega = 2.0 * PI * (cutoff / sr).min(0.49);
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0 * 0.707);
        let a0 = 1.0 + alpha;
        (
            -2.0 * cs / a0,
            (1.0 - alpha) / a0,
            (1.0 - cs) / 2.0 / a0,
            (1.0 - cs) / a0,
            (1.0 - cs) / 2.0 / a0,
        )
    }
}
