use crate::params::XrossBassAmpParams;
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
    pub params: Arc<XrossBassAmpParams>,
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
    
    // Bass specific
    comp_envelope: f32,
    gate_envelope: f32,
    crossover_lpf: f32, // For clean path
    crossover_hpf: f32, // For dirty path
}

impl GainProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
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
            comp_envelope: 0.0,
            gate_envelope: 0.0,
            crossover_lpf: 0.0,
            crossover_hpf: 0.0,
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
        self.comp_envelope = 0.0;
        self.gate_envelope = 0.0;
        self.crossover_lpf = 0.0;
        self.crossover_hpf = 0.0;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let input_gain_db = self.params.gain_section.input_gain.value();
        let master_gain_db = self.params.gain_section.master_gain.value();
        let drive = self.params.gain_section.drive.value();
        let grind = self.params.gain_section.grind.value();
        let blend = self.params.gain_section.blend.value();

        // 0. Input DC Block
        let in_dc_fix = input - self.input_dc_block;
        self.input_dc_block = input + 0.998 * (self.input_dc_block - input);

        let input_gain = 10.0f32.powf(input_gain_db / 20.0);
        let mut in_signal = in_dc_fix * input_gain;

        // 1. Bass Compressor (One-knob style)
        let comp_amount = self.params.fx_section.compressor.value();
        if comp_amount > 0.01 {
            let threshold = 0.1 / (1.0 + comp_amount * 2.0);
            let ratio = 1.0 + comp_amount * 10.0;
            let attack = 0.005; // 5ms
            let release = 0.050; // 50ms
            
            let abs_in = in_signal.abs();
            let coeff = if abs_in > self.comp_envelope {
                1.0 - (-1.0 / (attack * self.sample_rate)).exp()
            } else {
                1.0 - (-1.0 / (release * self.sample_rate)).exp()
            };
            self.comp_envelope += coeff * (abs_in - self.comp_envelope);
            
            if self.comp_envelope > threshold {
                let gain_reduction = (threshold / self.comp_envelope).powf(1.0 - 1.0 / ratio);
                in_signal *= gain_reduction;
            }
        }

        // 2. Parallel Processing (Clean Blend & Crossover)
        // Clean path: LPF at ~250Hz to preserve sub-bass
        let cross_freq = 250.0;
        let cross_coeff = 2.0 * PI * cross_freq / self.sample_rate;
        self.crossover_lpf += cross_coeff * (in_signal - self.crossover_lpf);
        let clean_path = self.crossover_lpf;

        // Dirty path: HPF at ~200Hz to prevent farty distortion
        self.crossover_hpf += cross_coeff * (in_signal - self.crossover_hpf);
        let dirty_input = in_signal - self.crossover_hpf;

        // 3. Drive Core (Oversampled)
        let s_low = (self.params.eq_section.low.value() + 18.0) / 36.0;
        let s_mid = (self.params.eq_section.mid.value() + 18.0) / 36.0;
        let s_high = (self.params.eq_section.high.value() + 18.0) / 36.0;
        let tight = self.params.fx_section.tight.value();

        self.envelope += (dirty_input.abs() - self.envelope) * 0.25;
        let current_env = self.envelope;

        let os_factor = 4;
        let mut dirty_sum = 0.0;
        let inv_os = 1.0 / os_factor as f32;

        for i in 0..os_factor {
            let fraction = i as f32 * inv_os;
            let sub_sample = self.prev_input + (dirty_input - self.prev_input) * fraction;
            dirty_sum += self.drive_core(
                sub_sample,
                drive,
                grind,
                s_low,
                s_mid,
                s_high,
                tight,
                current_env,
            );
        }
        self.prev_input = dirty_input;
        let dirty_path = dirty_sum * inv_os;

        // 4. Mixing & Post-FX
        let mixed = (clean_path * (1.0 - blend * 0.7)) + (dirty_path * blend * 1.5);
        
        // Noise Gate
        let gate_amount = self.params.fx_section.noise_gate.value();
        let mut final_out = mixed;
        if gate_amount > 0.01 {
            let gate_threshold = 0.001 * (gate_amount * 50.0).exp();
            let abs_mixed = final_out.abs();
            let gate_coeff = 1.0 - (-1.0 / (0.01 * self.sample_rate)).exp(); // 10ms
            self.gate_envelope += gate_coeff * (abs_mixed - self.gate_envelope);
            
            if self.gate_envelope < gate_threshold {
                let gate_gain = (self.gate_envelope / gate_threshold).powi(2);
                final_out *= gate_gain;
            }
        }

        // Output filtering & DC Block
        let (a1, a2, b0, b1, b2) = self.calculate_biquad_lpf(16000.0);
        let filtered_out = self.os_lpf_biquad.process(final_out, a1, a2, b0, b1, b2);

        let out = filtered_out;
        let dc_fix = out - self.dc_block;
        self.dc_block = out + 0.996 * (self.dc_block - out);

        let master_gain = 10.0f32.powf(master_gain_db / 20.0);
        dc_fix * 0.5 * master_gain
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn drive_core(
        &mut self,
        input: f32,
        drive: f32,
        grind: f32,
        s_low: f32,
        s_mid: f32,
        s_high: f32,
        tight: f32,
        env: f32,
    ) -> f32 {
        // 1. DYNAMIC PRE-HP (Bass Optimized)
        let tight_norm = (tight - 20.0) / 480.0;
        let hp_freq = 0.08 + (tight_norm * 0.25) + (env * 0.15);
        self.pre_hp += hp_freq * (input - self.pre_hp);
        let mut x = input - self.pre_hp;

        // 2. GAIN STAGING (Modern Bass Grind)
        if x > 0.0 {
            x *= 1.0 + grind * 0.5;
        }

        let drive_amt = ((drive * 6.0).exp() * 15.0) * (1.0 + grind * 2.0);
        x *= drive_amt;

        // 3. MULTI-STAGE SATURATION
        x += self.feedback_state * (0.15 + grind * 0.2);

        // Non-symmetrical saturation for tube-like growl
        x = if x > 0.0 {
            (x * 1.1).tanh()
        } else {
            (x * 0.9).tanh() * 0.95
        };

        // Style Mid Scoop / Growl
        let mid_growl = (s_mid - 0.5).abs() * 1.5;
        x -= (x - x.powi(3)) * 0.4 * mid_growl;

        // Hybrid Square Blend (Grind makes it sharper)
        let soft_out = (x * 1.2).atan() * 0.9;
        let hard_limit = 0.85 - (grind * 0.15);
        let hard_out = x.clamp(-hard_limit, hard_limit);

        let square_mix = (s_high * 0.3 + grind * 0.5).min(0.9);
        x = (soft_out * (1.0 - square_mix)) + (hard_out * square_mix);

        self.feedback_state = x;

        // 4. POST-PROCESSING
        let lpf_cutoff = 0.3 + (s_high * 0.4);
        self.post_tight += lpf_cutoff * (x - self.post_tight);
        x = self.post_tight;

        // Low Resonance (Bass Weight)
        let low_boost = s_low * 0.8 * (1.0 - env.min(0.6));
        self.low_resonance += 0.15 * (x - self.low_resonance);
        x += self.low_resonance * low_boost;

        // 5. SLEW RATE
        let max_step = 0.03 + (s_high * 0.7) + (grind * 0.3);
        let diff = x - self.slew_state;
        self.slew_state += diff.clamp(-max_step, max_step);

        self.slew_state
    }

    fn calculate_biquad_lpf(&self, cutoff: f32) -> (f32, f32, f32, f32, f32) {
        let ff = (cutoff / self.sample_rate).min(0.45);
        let omega = 2.0 * PI * ff;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0f32).sqrt();

        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cs / a0;
        let a2 = (1.0 - alpha) / a0;
        let b1 = (1.0 - cs) / a0;
        let b0 = b1 * 0.5;
        let b2 = b0;

        (a1, a2, b0, b1, b2)
    }
}