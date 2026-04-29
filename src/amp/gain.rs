use crate::params::XrossBassAmpParams;
use std::f32::consts::PI;
use std::sync::Arc;

/// TPT (Topology Preserving Transform) Filter
/// 係数の急激な変化に強く、数値的に非常に安定したフィルタ
struct StableFilter {
    s: f32,
}

impl StableFilter {
    fn new() -> Self {
        Self { s: 0.0 }
    }

    #[inline]
    fn process_lp(&mut self, input: f32, g: f32) -> f32 {
        // g は事前計算され、安全な範囲にクランプされている前提
        let v = (input - self.s) * g / (1.0 + g);
        let res = v + self.s;
        self.s = res + v;

        // デノーマル対策
        if self.s.abs() < 1e-18 {
            self.s = 0.0;
        }
        res
    }

    #[inline]
    fn process_hp(&mut self, input: f32, g: f32) -> f32 {
        input - self.process_lp(input, g)
    }

    fn reset(&mut self) {
        self.s = 0.0;
    }
}

pub struct GainProcessor {
    pub params: Arc<XrossBassAmpParams>,
    sample_rate: f32,

    // Filters
    input_dc_block: StableFilter,
    output_dc_block: StableFilter,
    crossover_lpf: StableFilter,
    pre_drive_hp: StableFilter,
    post_drive_lpf: StableFilter,

    // Dynamics State
    comp_env: f32,
    gate_env: f32,
}

impl GainProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        Self {
            params,
            sample_rate: 44100.0,
            input_dc_block: StableFilter::new(),
            output_dc_block: StableFilter::new(),
            crossover_lpf: StableFilter::new(),
            pre_drive_hp: StableFilter::new(),
            post_drive_lpf: StableFilter::new(),
            comp_env: 0.0,
            gate_env: 0.0,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.input_dc_block.reset();
        self.output_dc_block.reset();
        self.crossover_lpf.reset();
        self.pre_drive_hp.reset();
        self.post_drive_lpf.reset();
        self.comp_env = 0.0;
        self.gate_env = 0.0;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // 1. パラメータのフェッチと事前計算
        let input_gain = 10.0f32.powf(self.params.input_gain.value() / 20.0);
        let drive = self.params.drive.value().clamp(0.0, 1.0);
        let blend = self.params.blend.value().clamp(0.0, 1.0);
        let master_gain = 10.0f32.powf(self.params.master_gain.value() / 20.0);

        let tight = self.params.tight.value().clamp(0.0, 1.0);
        let presence = self.params.presence.value().clamp(0.0, 1.0);
        let compression = self.params.compression.value().clamp(0.0, 1.0);
        let gate_threshold = self.params.noise_gate.value().clamp(0.0, 1.0);

        // フィルタ係数の計算 (サンプリングレートに対して安全な範囲に制限)
        let calc_g = |freq: f32| (PI * freq / self.sample_rate).tan().min(0.99);
        let g_dc = calc_g(15.0);
        let g_cross = calc_g(250.0);
        let g_post = calc_g(7000.0);

        // 2. 入力処理 (DC Block & Gain)
        let mut sig = self.input_dc_block.process_hp(input, g_dc);
        sig *= input_gain;

        // 3. Noise Gate (歪ませる前にノイズを断つ)
        sig = self.apply_gate(sig, gate_threshold);

        // 4. Compression
        sig = self.apply_compression(sig, compression);

        // 5. Parallel Crossover (Darkglass B7K 構造)
        // 250Hz以下をクリーンとして保持し、残りを歪みセクションへ
        let clean_low = self.crossover_lpf.process_lp(sig, g_cross);
        let dirty_input = sig - clean_low;

        // 6. Drive Section
        let mut dirty_path = self.drive_core(dirty_input, drive, tight, presence);

        // 7. Post Filter (歪んだ高域を整える)
        dirty_path = self.post_drive_lpf.process_lp(dirty_path, g_post);

        // 8. Final Mix
        // 歪み音(blend) + 重厚な低域(clean_low) + 元の芯(sig)
        let mixed = (dirty_path * blend) + (clean_low * 1.1) + (sig * (1.0 - blend) * 0.4);

        // 9. 出力処理 (DC Block & Master)
        let final_sig = self.output_dc_block.process_hp(mixed, g_dc);

        (final_sig * master_gain).clamp(-1.0, 1.0)
    }

    fn apply_gate(&mut self, sig: f32, threshold_param: f32) -> f32 {
        if threshold_param < 0.001 {
            return sig;
        }

        // パラメータ 0.0-1.0 を実用的な閾値 (-60dB to -20dB) に変換
        let threshold_db = -60.0 + (threshold_param * 40.0);
        let threshold = 10.0f32.powf(threshold_db / 20.0);

        let abs_sig = sig.abs();

        // Attack 1ms / Release 100ms 程度
        let att = (-1.0 / (0.001 * self.sample_rate)).exp();
        let rel = (-1.0 / (0.100 * self.sample_rate)).exp();

        let coeff = if abs_sig > self.gate_env { att } else { rel };
        self.gate_env = abs_sig + coeff * (self.gate_env - abs_sig);

        if self.gate_env < threshold {
            // 閾値以下ならスムーズにミュート
            let reduction = (self.gate_env / threshold).powi(2);
            sig * reduction
        } else {
            sig
        }
    }

    fn apply_compression(&mut self, sig: f32, amount: f32) -> f32 {
        if amount < 0.001 {
            return sig;
        }

        let threshold = 0.12;
        let ratio = 1.0 + amount * 10.0;
        let abs_sig = sig.abs();

        let att = (-1.0 / (0.002 * self.sample_rate)).exp();
        let rel = (-1.0 / (0.080 * self.sample_rate)).exp();

        let coeff = if abs_sig > self.comp_env { att } else { rel };
        self.comp_env = abs_sig + coeff * (self.comp_env - abs_sig);

        if self.comp_env > threshold {
            let gr = (self.comp_env / threshold).powf((1.0 / ratio) - 1.0);
            sig * gr * (1.0 + amount * 0.3) // メイクアップゲインを少し追加
        } else {
            sig
        }
    }

    fn drive_core(&mut self, input: f32, drive: f32, tight: f32, presence: f32) -> f32 {
        // --- Pre-Distortion ---
        let f_tight = 60.0 + tight * 600.0;
        let g_tight = (PI * f_tight / self.sample_rate).tan().min(0.99);
        let mut x = self.pre_drive_hp.process_hp(input, g_tight);

        // Presence (Attackブースト)
        x *= 1.0 + (presence * 2.0);

        // --- Drive Gain ---
        let gain = 10.0f32.powf((drive * 45.0) / 20.0);
        x *= gain;

        // --- CMOS Modeling Distortion ---
        // tanhベースだが、正負でわずかに曲率を変えて実機のアナログ感を出す
        let mut out = if x > 0.0 {
            (x * 1.1).tanh()
        } else {
            (x * 0.95).tanh() * 1.05
        };

        // ハードなピッキング時のクリップ安定化
        if out.abs() > 0.9 {
            out = out.signum() * (0.9 + (out.abs() - 0.9) * 0.1);
        }

        out
    }
}
