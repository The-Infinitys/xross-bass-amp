use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossBassAmpParams;
use std::sync::Arc;
use truce::core::AudioBuffer;
use truce::params::FloatParamReadF32;

const PHASE_DELAY_SIZE: usize = 2048;
const MAX_ROOM_DELAY: usize = 192000;

pub struct CabProcessor {
    pub params: Arc<XrossBassAmpParams>,

    // --- 物理モデリング・フィルター ---
    body_resonators: [Biquad; 3], // Sub-bass, Box-low, Baffle
    woofer_character: Biquad,     // 低域の太さと粘り
    tweeter_path: Biquad,         // 高域のパキッとした質感 (10-inch+Hornイメージ)

    // [NEW] 低域サチュレーション用のクロスオーバー
    saturation_lpf: Biquad,

    // --- マイクロフォン・セクション ---
    // Mic A: Large Diaphragm Dynamic (D112/RE20 style) - 芯と重さ
    mic_a_tone: [Biquad; 3],
    // Mic B: Condenser/DI-Blend style - 解像度と輪郭
    mic_b_tone: [Biquad; 3],

    // --- Bass Mastering Chain ---
    sub_thump: Biquad,          // 50-60Hzの「地面を揺らす」成分
    growl_shelf: Biquad,        // 800Hz付近の歪みのエッジ
    clank_peak: Biquad,         // [NEW] モダンメタルに不可欠な2.8kHz付近の金属的アタック
    mud_cut: Biquad,            // 250Hz付近の濁り取り
    low_end_stabilizer: Biquad, // 最終的な低域の引き締め

    // [NEW] ノイズゲート用サイドチェーンフィルター (前段にゲートを置く際の流用コア)
    pub gate_sidechain_hpf: Biquad,

    // ステレオ・空間・物理挙動
    phase_alignment_delay: Vec<f32>,
    room_reflection: Vec<f32>,
    write_idx_room: usize,

    sample_rate: f32,
    cone_inertia_state: f32, // スピーカーコーンの慣性による「戻り」の遅れ
    last_params_hash: f32,
}

impl CabProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        let sr = 44100.0;
        Self {
            params,
            body_resonators: std::array::from_fn(|_| Biquad::new(sr)),
            woofer_character: Biquad::new(sr),
            tweeter_path: Biquad::new(sr),
            saturation_lpf: Biquad::new(sr),
            mic_a_tone: std::array::from_fn(|_| Biquad::new(sr)),
            mic_b_tone: std::array::from_fn(|_| Biquad::new(sr)),
            sub_thump: Biquad::new(sr),
            growl_shelf: Biquad::new(sr),
            clank_peak: Biquad::new(sr),
            mud_cut: Biquad::new(sr),
            low_end_stabilizer: Biquad::new(sr),
            gate_sidechain_hpf: Biquad::new(sr),

            phase_alignment_delay: vec![0.0; PHASE_DELAY_SIZE],
            room_reflection: vec![0.0; MAX_ROOM_DELAY],
            write_idx_room: 0,
            sample_rate: sr,
            cone_inertia_state: 0.0,
            last_params_hash: -1.0,
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let filters: &mut [&mut Biquad] = &mut [
            &mut self.woofer_character,
            &mut self.tweeter_path,
            &mut self.saturation_lpf,
            &mut self.sub_thump,
            &mut self.growl_shelf,
            &mut self.clank_peak,
            &mut self.mud_cut,
            &mut self.low_end_stabilizer,
            &mut self.gate_sidechain_hpf,
        ];
        for f in filters {
            f.set_sample_rate(sample_rate);
        }
        for f in &mut self.body_resonators {
            f.set_sample_rate(sample_rate);
        }
        for f in &mut self.mic_a_tone {
            f.set_sample_rate(sample_rate);
        }
        for f in &mut self.mic_b_tone {
            f.set_sample_rate(sample_rate);
        }
        self.reset();
    }

    pub fn reset(&mut self) {
        self.phase_alignment_delay.fill(0.0);
        self.room_reflection.fill(0.0);
        self.cone_inertia_state = 0.0;
    }

    fn update_coefficients_if_needed(&mut self) {
        let size = self.params.speaker_size.value();
        let res_mod = self.params.resonance.value();
        let pres_mod = self.params.presence.value();

        let current_hash = size + res_mod * 1.1 + pres_mod * 0.9;
        if (current_hash - self.last_params_hash).abs() < 0.0001 {
            return;
        }

        // 1. キャビネット共鳴 (メタル用に低域を少しタイトに、Qを高めに)
        // Sub: 5弦・DDropのボトムを支える (ルーズになりすぎないようQを1.5に引き上げ)
        self.body_resonators[0].set_params(
            FilterType::Peaking(4.0 * res_mod),
            50.0 * (15.0 / size),
            1.5,
        );
        // Low-Mid: 箱鳴り成分。300Hz付近のモタつきを避けるため160Hz付近をタイトに
        self.body_resonators[1].set_params(FilterType::Peaking(1.5 * res_mod), 160.0, 2.5);
        // Baffle: アタックの跳ね返り
        self.body_resonators[2].set_params(FilterType::Peaking(1.5), 900.0, 1.0);

        // 慣性サチュレーション用のクロスオーバーLPF (300Hz以下のみをサチュレートさせる)
        self.saturation_lpf
            .set_params(FilterType::LowPass, 300.0, 0.7);

        // 2. ウーファーとツイーターの役割分担
        // メタル特有のスラップ・ピックの「カリカリ感」を出すため、カットオフを3.2kHzに微調整
        self.tweeter_path
            .set_params(FilterType::HighShelf(pres_mod * 7.0), 3200.0, 0.7);

        // 3. Mic A (Dynamic: メタル定番のダークかつ強烈なパンチ)
        let dist_a = self.params.mic_a_distance.value();
        self.mic_a_tone[0].set_params(FilterType::Peaking((1.0 - dist_a) * 5.0), 65.0, 0.8); // 近接効果の重心を下げる
        self.mic_a_tone[1].set_params(FilterType::Peaking(3.0), 1500.0, 1.0); // ゴツゴツしたミッド
        self.mic_a_tone[2].set_params(FilterType::LowPass, 5000.0, 0.7); // ギターと被る超高域をカット

        // 4. Mic B (Condenser/DI: ピックの金属摩擦・アタックの解像度)
        let dist_b = self.params.mic_b_distance.value();
        self.mic_b_tone[0].set_params(FilterType::Peaking(2.0), 700.0, 0.8); // ドライブが絡むミッド
        self.mic_b_tone[1].set_params(FilterType::HighShelf(pres_mod * 4.5), 3500.0, 0.7); // 輪郭のギラつき
        self.mic_b_tone[2].set_params(FilterType::LowPass, 10000.0 - (dist_b * 3000.0), 0.7);

        // 5. ミックスを助ける最終処理 (Mastering Chain)
        self.sub_thump
            .set_params(FilterType::Peaking(2.0), 60.0, 2.0);
        self.mud_cut
            .set_params(FilterType::Peaking(-4.0), 220.0, 1.8); // メタルの200Hz付近の濁りは容赦なくカット

        // [NEW] 2.8kHz付近のメタル・クランク成分（Dingwall等のパキパキしたエッジ）
        self.clank_peak
            .set_params(FilterType::Peaking(pres_mod * 3.5), 2800.0, 1.2);

        self.growl_shelf
            .set_params(FilterType::HighShelf(pres_mod * 1.5), 1000.0, 0.5);

        // サブベースのボトムが破綻しないよう、HPFのカットオフを30Hzにして急峻に（Q=0.9）引き締め
        self.low_end_stabilizer
            .set_params(FilterType::HighPass, 30.0, 0.9);

        // [NEW] ゲート流用時のためのサイドチェーンHPF設定 (120Hz以下を感知させない)
        self.gate_sidechain_hpf
            .set_params(FilterType::HighPass, 120.0, 0.7);

        self.last_params_hash = current_hash;
    }

    pub fn process_truce(&mut self, buffer: &mut AudioBuffer) {
        self.update_coefficients_if_needed();

        let num_samples = buffer.num_samples();
        let room_mix = self.params.room_mix.value();
        let size = self.params.speaker_size.value();

        for i in 0..num_samples {
            let mut sig = buffer.output(0)[i];

            // --- 1. Frequency-Dependent Cone Inertia (改良版・物理的な「粘り」) ---
            // 全帯域を一括で遅らせるとアタックが鈍るため、低域成分のみを取り出してサチュレートさせる
            let low_component = self.saturation_lpf.process(sig);
            let high_component = sig - low_component; // 完全に位相の合うハイパス成分

            let inertia = (0.94 - (size * 0.004)).clamp(0.85, 0.96);
            let saturated_low = if low_component > 0.0 {
                low_component.atan()
            } else {
                (low_component * 0.96).atan() * 1.04
            };

            // 低域のみ慣性遅れを適用
            self.cone_inertia_state = self.cone_inertia_state
                + (1.0 - inertia) * (saturated_low - self.cone_inertia_state);

            // 鋭い高域（ピックアタック）と、粘りのある重低音を再結合
            sig = self.cone_inertia_state + high_component;

            // --- 2. Cabinet Resonances ---
            for res in &mut self.body_resonators {
                sig = res.process(sig);
            }

            // --- 3. Parallel Path (Woofer & Tweeter) ---
            let woofer_sig = sig;
            let tweeter_sig = self.tweeter_path.process(sig);
            // メタル用にツイーターのブレンド量を 0.4 -> 0.55 に引き上げ、エッジを明快に
            let combined_sig = woofer_sig + tweeter_sig * 0.55;

            // --- 4. Dual Mic Path ---
            let mut sig_a = combined_sig;
            for f in &mut self.mic_a_tone {
                sig_a = f.process(sig_a);
            }

            let mut sig_b = combined_sig;
            for f in &mut self.mic_b_tone {
                sig_b = f.process(sig_b);
            }

            // --- 5. Mixing & Stabilization ---
            let mut out_l = sig_a * 0.8 + sig_b * 0.4;
            let mut out_r = sig_a * 0.8 - sig_b * 0.2;

            // 最終的なトーン補正 (Mastering Logic)
            out_l = self.sub_thump.process(out_l);
            out_r = self.sub_thump.process(out_r);
            out_l = self.mud_cut.process(out_l);
            out_r = self.mud_cut.process(out_r);

            // [NEW] Clank成分のインサート
            out_l = self.clank_peak.process(out_l);
            out_r = self.clank_peak.process(out_r);

            out_l = self.growl_shelf.process(out_l);
            out_r = self.growl_shelf.process(out_r);
            out_l = self.low_end_stabilizer.process(out_l);
            out_r = self.low_end_stabilizer.process(out_r);

            // --- 6. Room Reflection (硬いコンクリートの反射壁イメージ) ---
            if room_mix > 0.0 {
                let reflect_time = 0.015 + self.params.room_size.value() * 0.025; // メタル用に少し短くタイトに
                let dr = (reflect_time * self.sample_rate) as usize;
                let buf_len = self.room_reflection.len();
                let idx = (self.write_idx_room + buf_len - dr) % buf_len;

                // フィードバックを高めにして金属的な響き（アンビエンス）をシミュレート
                let reflection = (self.room_reflection[idx] * 0.85).tanh() * 0.25;
                out_l += reflection * room_mix;
                out_r -= reflection * room_mix; // 逆相で広げる

                // 入力信号をルームバッファへ記憶
                self.room_reflection[self.write_idx_room] = (out_l + out_r) * 0.5;
                self.write_idx_room = (self.write_idx_room + 1) % buf_len;
            }

            // 出力
            if buffer.num_output_channels() >= 2 {
                buffer.output(0)[i] = out_l;
                buffer.output(1)[i] = out_r;
            } else {
                buffer.output(0)[i] = (out_l + out_r) * 0.5;
            }
        }
    }
}
