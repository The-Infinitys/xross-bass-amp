use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossBassAmpParams;
use std::sync::Arc;

const PHASE_DELAY_SIZE: usize = 2048;
const PHASE_DELAY_MASK: usize = PHASE_DELAY_SIZE - 1;
const MAX_BUFFER_SIZE: usize = 192000;

pub struct CabProcessor {
    pub params: Arc<XrossBassAmpParams>,

    // --- フィルタ群 ---
    mic_a_filters: [Biquad; 5],
    mic_b_filters: [Biquad; 5],

    // 物理特性
    impedance_resonance: Biquad, // スピーカーの自己共振
    port_resonance: Biquad,      // バスレフポートの共鳴 (ベース特有)
    cabinet_thump: Biquad,       // 100Hz付近のパンチ
    box_resonance: Biquad,       // 箱鳴り
    tight_filter: Biquad,

    // 位相特性
    phase_smearer: [Biquad; 2],
    cone_character: [Biquad; 4],
    internal_standing_wave: Biquad,

    // --- バッファ群 ---
    phase_delay_buffer_b: Vec<f32>,
    room_delay_buffer: Vec<f32>,
    write_idx_phase: usize,
    write_idx_room: usize,

    sample_rate: f32,

    // --- キャッシュ ---
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
        self.port_resonance.set_sample_rate(sr);
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
        let s_size = self.params.speaker_size.value();
        let s_count = self.params.speaker_count.value();
        let d_a = self.params.mic_a_distance.value();
        let a_a = self.params.mic_a_axis.value();
        let d_b = self.params.mic_b_distance.value();
        let a_b = self.params.mic_b_axis.value();

        // いずれかのパラメータに変更があれば再計算
        if (s_size - self.last_speaker_size).abs() > 0.001
            || s_count != self.last_speaker_count
            || (self.last_mic_params[0] - d_a).abs() > 0.001
            || (self.last_mic_params[1] - a_a).abs() > 0.001
            || (self.last_mic_params[2] - d_b).abs() > 0.001
            || (self.last_mic_params[3] - a_b).abs() > 0.001
        {
            // --- ベースキャビネット特有の低域共振計算 ---
            let speaker_res_freq = 55.0 * (12.0 / s_size);
            let count_scale = (s_count as f32).sqrt();

            // 1. スピーカー自己共振
            self.impedance_resonance
                .set_params(FilterType::Peaking(4.0), speaker_res_freq, 1.5);

            // 2. ポート共鳴 (Bass Reflex Port)
            self.port_resonance.set_params(
                FilterType::Peaking(3.0 * count_scale),
                speaker_res_freq * 0.8,
                2.0,
            );

            // 3. キャビネット内部の定在波
            let box_res_freq = 180.0 * (12.0 / s_size);
            self.box_resonance
                .set_params(FilterType::Peaking(2.0), box_res_freq, 1.0);

            // 4. Cone Breakup
            self.cone_character[0].set_params(FilterType::Peaking(-3.0), 800.0, 1.0);
            self.cone_character[1].set_params(FilterType::Peaking(4.0), 2500.0, 1.5);
            self.cone_character[2].set_params(FilterType::Peaking(-12.0), 5000.0, 2.0);

            // 5. Mic A (Dynamic - 芯のあるアタック)
            let prox_a = (1.0 - d_a).powi(2) * 15.0;
            self.mic_a_filters[0].set_params(FilterType::Peaking(prox_a), 60.0, 0.7);
            let click_a = (1.0 - a_a) * 6.0;
            self.mic_a_filters[1].set_params(FilterType::Peaking(click_a), 3000.0, 1.2);

            // 6. Mic B (Ribbon - 太い低域)
            let prox_b = (1.0 - d_b).powi(2) * 10.0;
            self.mic_b_filters[0].set_params(FilterType::Peaking(prox_b), 100.0, 0.5);
            self.mic_b_filters[4].set_params(
                FilterType::LowPass,
                8000.0 * (1.0 - a_b * 0.5),
                0.707,
            );

            // キャッシュ更新
            self.last_speaker_size = s_size;
            self.last_speaker_count = s_count;
            self.last_mic_params = [d_a, a_a, d_b, a_b];
        }
    }

    pub fn process(&mut self, input: f32) -> (f32, f32) {
        let bypass_mix = self.params.cab_bypass.value(); // 0.0 = CAB, 1.0 = DI

        // bypass_mix が 1.0 (完全なDI) の場合は無駄な処理をスキップ
        if bypass_mix >= 0.999 {
            return (input, input);
        }

        self.update_coefficients();

        // --- キャビネットシミュレーション処理 ---

        // 1. スピーカーの物理的飽和
        let mut sig = if input > 0.0 {
            input.tanh()
        } else {
            (input * 0.98).tanh() * 1.02
        };

        // 2. 共通物理フィルタリング
        sig = self.impedance_resonance.process(sig);
        sig = self.port_resonance.process(sig);
        sig = self.box_resonance.process(sig);
        for ap in &mut self.phase_smearer {
            sig = ap.process(sig);
        }
        for f in &mut self.cone_character {
            sig = f.process(sig);
        }

        // 3. マイクパラレル処理
        let mut sig_a = sig;
        for f in &mut self.mic_a_filters {
            sig_a = f.process(sig_a);
        }

        let mut sig_b = sig;
        for f in &mut self.mic_b_filters {
            sig_b = f.process(sig_b);
        }

        // 4. マイクの距離による遅延
        let delay_samples = (self.params.mic_b_distance.value() * 10.0) as usize;
        self.phase_delay_buffer_b[self.write_idx_phase] = sig_b;
        let read_idx = (self.write_idx_phase + PHASE_DELAY_SIZE - delay_samples) & PHASE_DELAY_MASK;
        sig_b = self.phase_delay_buffer_b[read_idx];
        self.write_idx_phase = (self.write_idx_phase + 1) & PHASE_DELAY_MASK;

        // 5. ステレオミックス
        let mut out_l = sig_a * 0.8 + sig_b * 0.4;
        let mut out_r = sig_a * 0.8 - sig_b * 0.2;

        // 6. Room Early Reflections
        let room_mix = self.params.room_mix.value();
        if room_mix > 0.001 {
            let buf_len = self.room_delay_buffer.len();
            let delay_time = (0.02 * self.sample_rate) as usize;
            let reflection =
                self.room_delay_buffer[(self.write_idx_room + buf_len - delay_time) % buf_len];

            out_l += reflection * room_mix * 0.5;
            out_r += reflection * room_mix * 0.5;

            self.room_delay_buffer[self.write_idx_room] = (sig_a + sig_b) * 0.5;
            self.write_idx_room = (self.write_idx_room + 1) % buf_len;
        }

        // --- DI と CAB の最終ミックス ---
        // bypass_mix = 0.0 なら CABのみ、1.0 なら DIのみ
        let final_l = (input * bypass_mix) + (out_l * (1.0 - bypass_mix));
        let final_r = (input * bypass_mix) + (out_r * (1.0 - bypass_mix));

        (final_l, final_r)
    }
}
