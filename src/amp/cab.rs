use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossBassAmpParams;
use std::sync::Arc;

const PHASE_DELAY_SIZE: usize = 2048;
const PHASE_DELAY_MASK: usize = PHASE_DELAY_SIZE - 1;
// 2のべき乗に変更 (192kHzサンプリングでも1秒弱確保できるサイズ)
const MAX_ROOM_BUFFER_SIZE: usize = 262144;
const ROOM_BUFFER_MASK: usize = MAX_ROOM_BUFFER_SIZE - 1;

pub struct CabProcessor {
    pub params: Arc<XrossBassAmpParams>,
    mic_a_filters: [Biquad; 5],
    mic_b_filters: [Biquad; 5],

    impedance_resonance: Biquad,
    port_resonance: Biquad,
    box_resonance: Biquad,
    cone_character: [Biquad; 4],
    phase_smearer: [Biquad; 2],

    phase_delay_buffer_b: Vec<f32>,
    room_delay_buffer: Vec<f32>,
    write_idx_phase: usize,
    write_idx_room: usize,

    sample_rate: f32,
    last_speaker_size: f32,
    last_speaker_count: i64,
    last_mic_params: [f32; 4],
}

impl CabProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        let sr = 44100.0;
        Self {
            params,
            mic_a_filters: std::array::from_fn(|_| Biquad::new(sr)),
            mic_b_filters: std::array::from_fn(|_| Biquad::new(sr)),
            impedance_resonance: Biquad::new(sr),
            port_resonance: Biquad::new(sr),
            box_resonance: Biquad::new(sr),
            cone_character: std::array::from_fn(|_| Biquad::new(sr)),
            phase_smearer: std::array::from_fn(|_| Biquad::new(sr)),
            phase_delay_buffer_b: vec![0.0; PHASE_DELAY_SIZE],
            room_delay_buffer: vec![0.0; MAX_ROOM_BUFFER_SIZE], // 固定長で確保
            write_idx_phase: 0,
            write_idx_room: 0,
            sample_rate: sr,
            last_speaker_size: -1.0,
            last_speaker_count: -1,
            last_mic_params: [-1.0; 4],
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.update_all_filter_rates(sample_rate);
        self.reset();
    }

    fn update_all_filter_rates(&mut self, sr: f32) {
        let filters: Vec<&mut Biquad> = vec![
            &mut self.impedance_resonance,
            &mut self.port_resonance,
            &mut self.box_resonance,
        ];
        for f in filters {
            f.set_sample_rate(sr);
        }
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
    }

    pub fn reset(&mut self) {
        self.phase_delay_buffer_b.fill(0.0);
        self.room_delay_buffer.fill(0.0);
        // 各Biquadの内部状態もリセット（これが重要）
        self.impedance_resonance.reset();
        self.port_resonance.reset();
        self.box_resonance.reset();
        for f in &mut self.mic_a_filters {
            f.reset();
        }
        for f in &mut self.mic_b_filters {
            f.reset();
        }
    }

    fn update_coefficients(&mut self) {
        let s_size = self.params.speaker_size.value();
        let s_count = self.params.speaker_count.value();
        let d_a = self.params.mic_a_distance.value();
        let a_a = self.params.mic_a_axis.value();
        let d_b = self.params.mic_b_distance.value();
        let a_b = self.params.mic_b_axis.value();

        if (s_size - self.last_speaker_size).abs() > 0.001
            || s_count != self.last_speaker_count
            || (self.last_mic_params[0] - d_a).abs() > 0.001
            || (self.last_mic_params[1] - a_a).abs() > 0.001
            || (self.last_mic_params[2] - d_b).abs() > 0.001
            || (self.last_mic_params[3] - a_b).abs() > 0.001
        {
            // --- 物理モデル ---
            let speaker_res_freq = (55.0 * (12.0 / s_size)).clamp(20.0, 200.0);
            let count_scale = (s_count as f32).sqrt();

            self.impedance_resonance
                .set_params(FilterType::Peaking(4.0), speaker_res_freq, 1.0);
            self.port_resonance.set_params(
                FilterType::Peaking(3.0 * count_scale),
                speaker_res_freq * 0.85,
                2.0,
            );

            let box_res_freq = (180.0 * (12.0 / s_size)).clamp(100.0, 500.0);
            self.box_resonance
                .set_params(FilterType::Peaking(2.0), box_res_freq, 1.0);

            // --- Cone Character ---
            self.cone_character[0].set_params(FilterType::Peaking(-3.0), 800.0, 0.7);
            self.cone_character[1].set_params(FilterType::Peaking(4.0), 2500.0, 1.0);
            self.cone_character[2].set_params(FilterType::LowPass, 5500.0, 0.707); // 5kHz以上を急峻にカット

            // --- Mic A ---
            let prox_a = (1.0 - d_a).powi(2) * 12.0;
            self.mic_a_filters[0].set_params(FilterType::Peaking(prox_a), 80.0, 0.5);
            let tilt_a = (1.0 - a_a) * 5.0;
            self.mic_a_filters[1].set_params(FilterType::Peaking(tilt_a), 3500.0, 1.0);

            // --- Mic B ---
            let prox_b = (1.0 - d_b).powi(2) * 15.0;
            self.mic_b_filters[0].set_params(FilterType::Peaking(prox_b), 60.0, 0.4);
            let lp_b = 10000.0 * (1.0 - a_b * 0.7);
            self.mic_b_filters[1].set_params(FilterType::LowPass, lp_b.max(2000.0), 0.707);

            self.last_speaker_size = s_size;
            self.last_speaker_count = s_count;
            self.last_mic_params = [d_a, a_a, d_b, a_b];
        }
    }

    pub fn process(&mut self, input: f32) -> (f32, f32) {
        let bypass_mix = self.params.cab_bypass.value();
        if bypass_mix >= 0.999 {
            return (input, input);
        }

        self.update_coefficients();

        // 1. スピーカーの非線形圧縮（わずかに）
        let mut sig = input.clamp(-1.5, 1.5);
        sig = if sig > 0.0 { sig.tanh() } else { sig };

        // 2. キャビネット共通フィルタ
        sig = self.impedance_resonance.process(sig);
        sig = self.port_resonance.process(sig);
        sig = self.box_resonance.process(sig);
        for f in &mut self.cone_character {
            sig = f.process(sig);
        }
        for f in &mut self.phase_smearer {
            sig = f.process(sig);
        }

        // 3. マイクパラレル
        let mut sig_a = sig;
        for f in &mut self.mic_a_filters {
            sig_a = f.process(sig_a);
        }

        let mut sig_b = sig;
        for f in &mut self.mic_b_filters {
            sig_b = f.process(sig_b);
        }

        // 4. マイクBの距離ディレイ
        let delay_samples =
            ((self.params.mic_b_distance.value() * 0.005) * self.sample_rate) as usize;
        let safe_delay = delay_samples.min(PHASE_DELAY_SIZE - 1);

        self.phase_delay_buffer_b[self.write_idx_phase] = sig_b;
        let read_idx = (self.write_idx_phase + PHASE_DELAY_SIZE - safe_delay) & PHASE_DELAY_MASK;
        sig_b = self.phase_delay_buffer_b[read_idx];
        self.write_idx_phase = (self.write_idx_phase + 1) & PHASE_DELAY_MASK;

        // 5. ステレオミックス（位相干渉を考慮）
        // 1.0/-1.0で混ぜると完全に消える周波数が出るため、わずかにバランスをずらす
        let mut out_l = sig_a * 0.7 + sig_b * 0.3;
        let mut out_r = sig_a * 0.7 - sig_b * 0.2;

        // 6. Room Early Reflections (単発ディレイとして安定化)
        let room_mix = self.params.room_mix.value();
        if room_mix > 0.001 {
            let delay_time = (0.025 * self.sample_rate) as usize; // 25ms
            let read_room =
                (self.write_idx_room + MAX_ROOM_BUFFER_SIZE - delay_time) & ROOM_BUFFER_MASK;
            let reflection = self.room_delay_buffer[read_room];

            out_l += reflection * room_mix * 0.3;
            out_r += reflection * room_mix * 0.3;

            // フィードバックはさせない（発振防止）
            self.room_delay_buffer[self.write_idx_room] = (sig_a + sig_b) * 0.5;
            self.write_idx_room = (self.write_idx_room + 1) & ROOM_BUFFER_MASK;
        }

        // 7. DI/CABミックス
        let final_l = input * bypass_mix + out_l * (1.0 - bypass_mix);
        let final_r = input * bypass_mix + out_r * (1.0 - bypass_mix);

        (final_l.clamp(-1.0, 1.0), final_r.clamp(-1.0, 1.0))
    }
}
