use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossBassAmpParams;
use std::sync::Arc;
use truce::core::AudioBuffer;

const PHASE_DELAY_SIZE: usize = 2048;
const PHASE_DELAY_MASK: usize = PHASE_DELAY_SIZE - 1;
const MAX_ROOM_DELAY: usize = 192000;

pub struct CabProcessor {
    pub params: Arc<XrossBassAmpParams>,

    // 音楽的な質感を形成するフィルター群
    mic_a_tone: [Biquad; 3], // Character, Presence, Air
    mic_b_tone: [Biquad; 3], // Character, Body, Smooth
    body_resonance: Biquad,  // キャビネットの豊かな鳴り
    punch_filter: Biquad,    // 低域の押し出し感 (Thump)
    clarity_shelf: Biquad,   // 高域の抜け
    high_cut: Biquad,        // 滑らかなロールオフ
    low_cut: Biquad,         // 不要な低域の整理 (Tight)

    // ステレオイメージング
    phase_alignment_delay: Vec<f32>,
    room_reflection: Vec<f32>,

    write_idx_phase: usize,
    write_idx_room: usize,

    sample_rate: f32,
    saturation_state: f32,

    last_speaker_size: f32,
    last_speaker_count: i64,
    last_mic_params: [f32; 4],
    last_eq_extras: [f32; 2],
}

impl CabProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        let sr = 44100.0;
        Self {
            params,
            mic_a_tone: std::array::from_fn(|_| Biquad::new(sr)),
            mic_b_tone: std::array::from_fn(|_| Biquad::new(sr)),
            body_resonance: Biquad::new(sr),
            punch_filter: Biquad::new(sr),
            clarity_shelf: Biquad::new(sr),
            high_cut: Biquad::new(sr),
            low_cut: Biquad::new(sr),

            phase_alignment_delay: vec![0.0; PHASE_DELAY_SIZE],
            room_reflection: vec![0.0; MAX_ROOM_DELAY],

            write_idx_phase: 0,
            write_idx_room: 0,
            sample_rate: sr,
            saturation_state: 0.0,
            last_speaker_size: -1.0,
            last_speaker_count: -1,
            last_mic_params: [-1.0; 4],
            last_eq_extras: [-1.0; 2],
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.update_all_filter_rates(sample_rate);
        self.reset();
    }

    fn update_all_filter_rates(&mut self, sr: f32) {
        let filters: &mut [&mut Biquad] = &mut [
            &mut self.body_resonance,
            &mut self.punch_filter,
            &mut self.clarity_shelf,
            &mut self.high_cut,
            &mut self.low_cut,
        ];
        for f in filters {
            f.set_sample_rate(sr);
        }
        for f in &mut self.mic_a_tone {
            f.set_sample_rate(sr);
        }
        for f in &mut self.mic_b_tone {
            f.set_sample_rate(sr);
        }
    }

    pub fn reset(&mut self) {
        self.phase_alignment_delay.fill(0.0);
        self.room_reflection.fill(0.0);
        self.write_idx_phase = 0;
        self.write_idx_room = 0;
        self.saturation_state = 0.0;
    }

    fn update_coefficients_if_needed(&mut self) {
        let s_size = self.params.speaker_size.value();
        let s_count = self.params.speaker_count.value() as f32;
        let d_a = self.params.mic_a_distance.value();
        let a_a = self.params.mic_a_axis.value();
        let d_b = self.params.mic_b_distance.value();
        let a_b = self.params.mic_b_axis.value();
        let res_val = self.params.resonance.value();
        let pres_val = self.params.presence.value();

        let changed = (s_size - self.last_speaker_size).abs() > 0.001
            || (s_count - self.last_speaker_count as f32).abs() > 0.1
            || (d_a - self.last_mic_params[0]).abs() > 0.001
            || (a_a - self.last_mic_params[1]).abs() > 0.001
            || (d_b - self.last_mic_params[2]).abs() > 0.001
            || (a_b - self.last_mic_params[3]).abs() > 0.001
            || (res_val - self.last_eq_extras[0]).abs() > 0.001
            || (pres_val - self.last_eq_extras[1]).abs() > 0.001;

        if !changed {
            return;
        }

        // --- Speaker/Body Character (良い鳴りを強調) ---
        let base_res_freq = 90.0 * (12.0 / s_size);
        self.body_resonance.set_params(
            FilterType::Peaking(res_val * 4.0),
            base_res_freq,
            0.8, // 音楽的な広めのQ
        );

        let punch_freq = 120.0 + (s_count * 10.0);
        self.punch_filter
            .set_params(FilterType::Peaking(3.0), punch_freq, 1.5);

        self.clarity_shelf
            .set_params(FilterType::HighShelf(pres_val * 3.0), 3200.0, 0.707);

        // --- Microphone A: Clarity & Bite (SM57-style) ---
        // 軸を外すと高域が滑らかになり、近づけると近接効果で太くなる
        let mic_a_prox = (1.0 - d_a).powi(2) * 6.0;
        self.mic_a_tone[0].set_params(FilterType::Peaking(mic_a_prox), 150.0, 0.7);
        let mic_a_presence = (1.0 - a_a) * 4.0;
        self.mic_a_tone[1].set_params(FilterType::Peaking(mic_a_presence), 4500.0, 1.2);
        self.mic_a_tone[2].set_params(FilterType::LowPass, 12000.0 - (a_a * 4000.0), 0.707);

        // --- Microphone B: Warmth & Body (Ribbon-style) ---
        let mic_b_body = (1.0 - d_b) * 5.0;
        self.mic_b_tone[0].set_params(FilterType::Peaking(mic_b_body), 300.0, 0.8);
        let mic_b_smooth = -5.0 * a_b;
        self.mic_b_tone[1].set_params(FilterType::HighShelf(mic_b_smooth), 2500.0, 0.7);
        self.mic_b_tone[2].set_params(FilterType::LowPass, 8000.0 - (d_b * 2000.0), 1.0);

        // --- Final Cleanup ---
        self.low_cut.set_params(FilterType::HighPass, 75.0, 0.6); // Tighten
        self.high_cut
            .set_params(FilterType::LowPass, 15000.0, 0.707); // Smooth Top

        self.last_speaker_size = s_size;
        self.last_speaker_count = s_count as i64;
        self.last_mic_params = [d_a, a_a, d_b, a_b];
        self.last_eq_extras = [res_val, pres_val];
    }

    pub fn process_truce(&mut self, buffer: &mut AudioBuffer) {
        self.update_coefficients_if_needed();

        let num_samples = buffer.num_samples();
        let out_channels = buffer.num_output_channels();

        let room_mix = self.params.room_mix.value();
        let room_size = self.params.room_size.value();

        // わずかな位相差を意図的に作り、ステレオの奥行きを出す
        let d_a = self.params.mic_a_distance.value();
        let d_b = self.params.mic_b_distance.value();
        let diff_ms = (d_b - d_a) * 2.0; // 最大2ms程度の自然な遅延
        let delay_samples =
            (diff_ms.abs() * 0.001 * self.sample_rate).clamp(0.0, PHASE_DELAY_SIZE as f32 - 1.0);
        let delay_int = delay_samples as usize;
        let frac = delay_samples - (delay_int as f32);

        let buf_len = self.room_reflection.len();

        for i in 0..num_samples {
            let input = buffer.output(0)[i];

            // 1. Cabinet Drive (スピーカーの飽和感)
            // 負の位相をわずかに強調し、2次倍音のような温かみを加える
            let drive = 1.1;
            let x = input * drive;
            let saturated = if x > 0.0 {
                x.atan()
            } else {
                (x * 0.95).atan() * 1.05
            };

            // スルーレート制限に近い挙動で高域の角を取る
            let mut sig = self.saturation_state + 0.85 * (saturated - self.saturation_state);
            self.saturation_state = sig;

            // 2. Body & Character
            sig = self.body_resonance.process(sig);
            sig = self.punch_filter.process(sig);
            sig = self.clarity_shelf.process(sig);
            sig = self.low_cut.process(sig);
            sig = self.high_cut.process(sig);

            // 3. Dual Mic Path
            let mut sig_a = sig;
            for f in &mut self.mic_a_tone {
                sig_a = f.process(sig_a);
            }

            let mut sig_b = sig;
            for f in &mut self.mic_b_tone {
                sig_b = f.process(sig_b);
            }

            // 4. Phase Alignment (B側を遅延させて空間を作る)
            self.phase_alignment_delay[self.write_idx_phase] = sig_b;
            let r1 = (self.write_idx_phase + PHASE_DELAY_SIZE - delay_int) & PHASE_DELAY_MASK;
            let r2 = (r1 + PHASE_DELAY_SIZE - 1) & PHASE_DELAY_MASK;
            sig_b = self.phase_alignment_delay[r1] * (1.0 - frac)
                + self.phase_alignment_delay[r2] * frac;
            self.write_idx_phase = (self.write_idx_phase + 1) & PHASE_DELAY_MASK;

            // 5. Stereo Imaging
            // Mic A をセンター寄り、Mic B を広げることでリッチなステレオ感を作る
            let mut out_l = sig_a * 0.7 + sig_b * 0.5;
            let mut out_r = sig_a * 0.7 - sig_b * 0.3;

            // 6. Natural Room (初期反射のみ。濁らせず広がりだけを加える)
            if room_mix > 0.0 {
                let reflect_time = 0.02 + room_size * 0.05;
                let dr = (reflect_time * self.sample_rate) as usize;
                let idx = (self.write_idx_room + buf_len - dr) % buf_len;
                let reflection = self.room_reflection[idx];

                out_l += reflection * room_mix * 0.2;
                out_r -= reflection * room_mix * 0.2; // 逆相で広げる

                self.room_reflection[self.write_idx_room] = (sig_a + sig_b) * 0.5;
                self.write_idx_room = (self.write_idx_room + 1) % buf_len;
            }

            // 出力
            if out_channels >= 2 {
                buffer.output(0)[i] = out_l;
                buffer.output(1)[i] = out_r;
            } else {
                buffer.output(0)[i] = (out_l + out_r) * 0.6;
            }
        }
    }
}
