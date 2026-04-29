use crate::params::XrossBassAmpParams;
use std::f32::consts::PI;
use std::sync::Arc;

/// TPT (Topology Preserving Transform) Filter
struct StableFilter {
    s: f32,
}

impl StableFilter {
    fn new() -> Self {
        Self { s: 0.0 }
    }

    #[inline]
    fn process_lp(&mut self, input: f32, g: f32) -> f32 {
        let v = (input - self.s) * g / (1.0 + g);
        let res = v + self.s;
        self.s = res + v;
        if self.s.abs() < 1e-12 {
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
    gate_release_state: f32, // 可変リリースのための内部状態
    initialized: bool,
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
            gate_release_state: 1.0,
            initialized: false,
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
        self.gate_release_state = 1.0;
        self.initialized = false;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // --- 1. Parameter Fetch & Smoothing ---
        let input_gain = 10.0f32.powf(self.params.input_gain.value() / 20.0);
        let drive = self.params.drive.value().clamp(0.0, 1.0);
        let blend = self.params.blend.value().clamp(0.0, 1.0);
        let master_gain = 10.0f32.powf(self.params.master_gain.value() / 20.0);
        let tight = self.params.tight.value().clamp(0.0, 1.0);
        let presence = self.params.presence.value().clamp(0.0, 1.0);
        let compression = self.params.compression.value().clamp(0.0, 1.0);
        let gate_threshold = self.params.noise_gate.value().clamp(0.0, 1.0);

        let calc_g = |freq: f32| (PI * freq / self.sample_rate).tan().min(0.99);
        let g_dc = calc_g(10.0);
        let g_cross = calc_g(140.0); // 芯をより重厚にするためクロスオーバーを少し下げた
        let g_post = calc_g(5000.0);

        if !self.initialized && input.abs() > 0.0001 {
            self.gate_env = input.abs();
            self.comp_env = input.abs();
            self.initialized = true;
        }

        // --- 2. Pre-Processing ---
        let mut sig = self.input_dc_block.process_hp(input, g_dc);
        sig *= input_gain;

        // --- 3. Intelligent Noise Gate ---
        sig = self.apply_adaptive_gate(sig, gate_threshold);

        // --- 4. Pre-Drive Compression ---
        sig = self.apply_compression(sig, compression);

        // --- 5. Signal Splitting ---
        let clean_low = self.crossover_lpf.process_lp(sig, g_cross);
        let dirty_input = sig - clean_low;

        // --- 6. Dirty Path ---
        let mut dirty_path = self.drive_core(dirty_input, drive, tight, presence);
        dirty_path = self.post_drive_lpf.process_lp(dirty_path, g_post);

        // --- 7. Clean Path (The "Bone" of Bass) ---
        // 芯を太く残すため、低域に非対称の飽和感を与え、音圧を安定させる
        let clean_path = if clean_low > 0.0 {
            (clean_low * 1.4).tanh() * 0.85
        } else {
            (clean_low * 1.2).tanh() * 0.95
        };

        // --- 8. Final Mix Logic ---
        // Blend 100%でもボトムが痩せないよう、cleanをベースミックスとして常時保持
        let dry_mix = 1.0 - blend;
        let mixed = (dirty_path * blend) + (clean_path * (0.4 + dry_mix * 0.6));

        // --- 9. Output Stage ---
        let final_sig = self.output_dc_block.process_hp(mixed, g_dc);
        (final_sig * master_gain).clamp(-1.0, 1.0)
    }

    fn apply_adaptive_gate(&mut self, sig: f32, threshold_param: f32) -> f32 {
        if threshold_param < 0.005 {
            return sig;
        }

        let threshold = 10.0f32.powf((-65.0 + threshold_param * 45.0) / 20.0);
        let abs_sig = sig.abs();

        // 信号の変化をトラッキング
        let attack_coeff = (-1.0 / (0.001 * self.sample_rate)).exp();
        let prev_env = self.gate_env;
        self.gate_env = abs_sig + attack_coeff * (self.gate_env - abs_sig);

        // スルーレート（信号の減衰速度）に基づいてリリースタイムを可変
        // 急に下がったときはスルーレートが大きくなる
        let delta = (prev_env - self.gate_env).max(0.0);

        // 減衰が速い(deltaが大きい)ほど高速リリース(20ms)、遅いほど低速リリース(300ms)
        let fast_rel = (-1.0 / (0.020 * self.sample_rate)).exp();
        let slow_rel = (-1.0 / (0.300 * self.sample_rate)).exp();
        let rel_coeff = if delta > 0.001 { fast_rel } else { slow_rel };

        if self.gate_env < threshold {
            self.gate_release_state *= rel_coeff;
        } else {
            // ゲートが開くときは一瞬で
            self.gate_release_state = 1.0;
        }

        sig * self.gate_release_state
    }

    fn apply_compression(&mut self, sig: f32, amount: f32) -> f32 {
        if amount < 0.001 {
            return sig;
        }

        let threshold = 0.12;
        let ratio = 1.0 + amount * 15.0;
        let abs_sig = sig.abs();

        // 2ms Attack / 120ms Release
        let att = (-1.0 / (0.002 * self.sample_rate)).exp();
        let rel = (-1.0 / (0.120 * self.sample_rate)).exp();

        let coeff = if abs_sig > self.comp_env { att } else { rel };
        self.comp_env = abs_sig + coeff * (self.comp_env - abs_sig);

        if self.comp_env > threshold {
            let gr = (self.comp_env / threshold).powf((1.0 / ratio) - 1.0);
            sig * gr * (1.0 + amount * 0.6)
        } else {
            sig
        }
    }

    fn drive_core(&mut self, input: f32, drive: f32, tight: f32, presence: f32) -> f32 {
        // Tight: 歪みの前の低域をカットし、濁り（Mud）を取る
        let f_tight = 70.0 + tight * 600.0;
        let g_tight = (PI * f_tight / self.sample_rate).tan().min(0.99);
        let mut x = self.pre_drive_hp.process_hp(input, g_tight);

        // Presence: アタックの食いつき（3.5kHz付近の鋭さ）
        x *= 1.0 + (presence * 3.0);

        // Drive Gain (最大 55dB)
        let gain = 10.0f32.powf((drive * 55.0) / 20.0);
        x *= gain;

        // CMOS Non-linear Saturation (正負で特性を変える)
        let mut out = if x > 0.0 {
            (x * 1.15).tanh()
        } else {
            (x * 0.85).tanh() * 1.2 // 負の波形を少し太くしてアナログ的な粘りを出す
        };

        // 超高域のチリチリしたノイズをわずかに丸める
        if out.abs() > 0.8 {
            let soft = 0.8;
            let excess = out.abs() - soft;
            out = out.signum() * (soft + excess / (1.0 + excess.powi(2)));
        }

        out
    }
}
