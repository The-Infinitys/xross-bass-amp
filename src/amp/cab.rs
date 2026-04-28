use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossBassAmpParams;
use std::sync::Arc;

const PHASE_DELAY_SIZE: usize = 2048;
const PHASE_DELAY_MASK: usize = PHASE_DELAY_SIZE - 1;
const MAX_BUFFER_SIZE: usize = 192000;

pub struct CabProcessor {
    pub params: Arc<XrossBassAmpParams>,
    mic_a_filters: [Biquad; 5],
    mic_b_filters: [Biquad; 5],

    // 物理特性
    impedance_resonance: Biquad,
    presence_shelf: Biquad,
    cabinet_thump: Biquad,
    box_resonance: Biquad,
    tight_filter: Biquad,

    // 位相系
    phase_smearer: [Biquad; 2],
    cone_character: [Biquad; 4],
    internal_standing_wave: Biquad,

    // バッファ
    phase_delay_buffer_b: Vec<f32>,
    room_delay_buffer: Vec<f32>,
    write_idx_phase: usize,
    write_idx_room: usize,

    sample_rate: f32,

    // キャッシュ
    last_speaker_size: f32,
    last_speaker_count: i32,
    last_mic_params: [f32; 4],
    last_eq_extras: [f32; 2],
}

impl CabProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        let sr = 44100.0;
        Self {
            params,
            mic_a_filters: std::array::from_fn(|_| Biquad::new(sr)),
            mic_b_filters: std::array::from_fn(|_| Biquad::new(sr)),
            impedance_resonance: Biquad::new(sr),
            presence_shelf: Biquad::new(sr),
            cabinet_thump: Biquad::new(sr),
            box_resonance: Biquad::new(sr),
            tight_filter: Biquad::new(sr),
            phase_smearer: std::array::from_fn(|_| Biquad::new(sr)),
            cone_character: std::array::from_fn(|_| Biquad::new(sr)),
            internal_standing_wave: Biquad::new(sr),
            phase_delay_buffer_b: vec![0.0; PHASE_DELAY_SIZE],
            room_delay_buffer: vec![0.0; MAX_BUFFER_SIZE],
            write_idx_phase: 0,
            write_idx_room: 0,
            sample_rate: sr,
            last_speaker_size: -1.0,
            last_speaker_count: -1,
            last_mic_params: [-1.0; 4],
            last_eq_extras: [-1.0; 2],
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.update_all_filter_rates(sample_rate);
        self.room_delay_buffer.resize(sample_rate as usize, 0.0);
        self.reset();
    }

    fn update_all_filter_rates(&mut self, sr: f32) {
        for f in &mut self.mic_a_filters {
            f.set_sample_rate(sr);
        }
        for f in &mut self.mic_b_filters {
            f.set_sample_rate(sr);
        }
        for f in &mut self.cone_character {
            f.set_sample_rate(sr);
        }
        for f in &mut self.phase_smearer {
            f.set_sample_rate(sr);
        }
        self.impedance_resonance.set_sample_rate(sr);
        self.presence_shelf.set_sample_rate(sr);
        self.cabinet_thump.set_sample_rate(sr);
        self.box_resonance.set_sample_rate(sr);
        self.tight_filter.set_sample_rate(sr);
        self.internal_standing_wave.set_sample_rate(sr);
    }

    pub fn reset(&mut self) {
        self.phase_delay_buffer_b.fill(0.0);
        self.room_delay_buffer.fill(0.0);
        self.write_idx_phase = 0;
        self.write_idx_room = 0;
    }

    fn update_coefficients(&mut self) {
        let (s_size, s_count, d_a, a_a, d_b, a_b, res_val, pres_val) = {
            let cab = &self.params.cab_section;
            let eq = &self.params.eq_section;
            (
                cab.speaker_size.value(),
                cab.speaker_count.value(),
                cab.mic_a_distance.value(),
                cab.mic_a_axis.value(),
                cab.mic_b_distance.value(),
                cab.mic_b_axis.value(),
                eq.resonance.value(),
                eq.presence.value(),
            )
        };

        if (s_size - self.last_speaker_size).abs() > 0.001
            || s_count != self.last_speaker_count
            || (d_a - self.last_mic_params[0]).abs() > 0.001
            || (res_val - self.last_eq_extras[0]).abs() > 0.001
        {
            // モダンメタル向け調整: 低域の共鳴をよりタイトに (50Hz-70Hz付近)
            let speaker_res_freq = 65.0 * (10.0 / s_size).sqrt();
            let count_scale = (s_count as f32).sqrt();

            // 1. 低域の押し出し (Impedance & Thump)
            self.impedance_resonance.set_params(
                FilterType::Peaking(res_val * 3.0),
                speaker_res_freq,
                1.0,
            );

            // 2. Presence: 歪みのエッジを整える
            self.presence_shelf
                .set_params(FilterType::HighShelf(pres_val * 2.0), 3200.0, 0.7);

            // 3. 箱鳴り: 150Hz付近の「ムワッ」とする成分を整理
            let box_res_freq = 160.0 * (10.0 / s_size);
            self.box_resonance.set_params(
                FilterType::Peaking(1.5 * count_scale),
                box_res_freq,
                2.0,
            );

            // 4. Cone Breakup: ギターと被る帯域(400-800Hz)を少し整理
            self.cone_character[0].set_params(FilterType::Peaking(-4.0), 500.0, 1.0);
            self.cone_character[1].set_params(FilterType::Peaking(2.0), 1200.0, 1.2);
            self.cone_character[2].set_params(FilterType::Peaking(4.0), 2400.0, 1.5); // Clank強調
            self.cone_character[3].set_params(FilterType::LowPass, 6500.0, 0.707); // 痛い高域をカット

            // 5. Mic A: Dynamic (SM57風 - ガッツのある中域)
            let prox_a = (1.0 - d_a).powi(2) * 12.0;
            self.mic_a_filters[0].set_params(FilterType::Peaking(prox_a), 80.0, 0.7);
            self.mic_a_filters[1].set_params(FilterType::Peaking(3.0 * (1.0 - a_a)), 3000.0, 0.8);

            // 6. Mic B: Condenser (深みと空気感)
            let prox_b = (1.0 - d_b).powi(2) * 8.0;
            self.mic_b_filters[0].set_params(FilterType::LowShelf(prox_b), 100.0, 0.7);

            // 7. Low-End Tightness (重要: 超低域のボワつきをカット)
            self.tight_filter
                .set_params(FilterType::HighPass, 45.0, 0.707);

            self.last_speaker_size = s_size;
            self.last_speaker_count = s_count;
            self.last_mic_params = [d_a, a_a, d_b, a_b];
            self.last_eq_extras = [res_val, pres_val];
        }
    }

    fn apply_speaker_physics(&self, input: f32) -> f32 {
        // スピーカーの飽和: わずかに非対称にして「生」感を出す
        let drive = 1.2;
        if input > 0.0 {
            (input * drive).tanh() * 0.98
        } else {
            (input * drive * 1.05).tanh()
        }
    }

    pub fn process(&mut self, input: f32) -> (f32, f32) {
        self.update_coefficients();

        let mut sig = self.apply_speaker_physics(input);

        // 基本フィルタ
        sig = self.impedance_resonance.process(sig);
        sig = self.presence_shelf.process(sig);
        sig = self.box_resonance.process(sig);
        sig = self.tight_filter.process(sig);

        for ap in &mut self.phase_smearer {
            sig = ap.process(sig);
        }
        for f in &mut self.cone_character {
            sig = f.process(sig);
        }

        // マイク分岐
        let mut sig_a = sig;
        for f in &mut self.mic_a_filters {
            sig_a = f.process(sig_a);
        }

        let mut sig_b = sig;
        for f in &mut self.mic_b_filters {
            sig_b = f.process(sig_b);
        }

        // マイク間遅延 (Time Alignment)
        let d_a = self.params.cab_section.mic_a_distance.value();
        let d_b = self.params.cab_section.mic_b_distance.value();
        let diff_samples = (d_b - d_a).abs() * 0.001 * self.sample_rate * 5.0; // 5msレンジ

        let delay_int = (diff_samples as usize).min(PHASE_DELAY_SIZE - 2);
        let frac = diff_samples - (delay_int as f32);

        self.phase_delay_buffer_b[self.write_idx_phase] = sig_b;
        let r1 = (self.write_idx_phase + PHASE_DELAY_SIZE - delay_int) & PHASE_DELAY_MASK;
        let r2 = (r1 + PHASE_DELAY_SIZE - 1) & PHASE_DELAY_MASK;
        sig_b = self.phase_delay_buffer_b[r1] * (1.0 - frac) + self.phase_delay_buffer_b[r2] * frac;
        self.write_idx_phase = (self.write_idx_phase + 1) & PHASE_DELAY_MASK;

        // ステレオミックス: Aは芯、Bは広がり
        let out_l = sig_a * 0.8 + sig_b * 0.4;
        let out_r = sig_a * 0.8 - sig_b * 0.2;

        // ルームリバーブ (アーリーリフレクション)
        let room_mix = self.params.cab_section.room_mix.value();
        let (final_l, final_r) = if room_mix > 0.0 {
            let room_size = self.params.cab_section.room_size.value();
            let buf_len = self.room_delay_buffer.len();

            // シンプルなマルチタップ
            let mut ref_l = 0.0;
            let mut ref_r = 0.0;
            let taps = [0.015, 0.029, 0.055]; // ms
            for &t in &taps {
                let d = ((t + room_size * 0.04) * self.sample_rate) as usize;
                let val = self.room_delay_buffer[(self.write_idx_room + buf_len - d) % buf_len];
                ref_l += val;
                ref_r += val * 0.8; // わずかな左右差
            }

            self.room_delay_buffer[self.write_idx_room] = (out_l + out_r) * 0.5;
            self.write_idx_room = (self.write_idx_room + 1) % buf_len;

            (
                out_l + ref_l * room_mix * 0.3,
                out_r + ref_r * room_mix * 0.3,
            )
        } else {
            (out_l, out_r)
        };

        (final_l * 1.3, final_r * 1.3) // 最終音量補正
    }
}
