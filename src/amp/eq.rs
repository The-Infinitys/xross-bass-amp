use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossBassAmpParams;
use std::sync::Arc;

pub struct EqProcessor {
    pub params: Arc<XrossBassAmpParams>,
    low_filter: Biquad,
    mid_filter: Biquad,
    high_filter: Biquad,
    presence_filter: Biquad,
    resonance_filter: Biquad,
    last_eq_values: [f32; 5],
}

impl EqProcessor {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        let sr = 44100.0;
        Self {
            params,
            low_filter: Biquad::new(sr),
            mid_filter: Biquad::new(sr),
            high_filter: Biquad::new(sr),
            presence_filter: Biquad::new(sr),
            resonance_filter: Biquad::new(sr),
            last_eq_values: [-999.0; 5],
        }
    }

    pub fn initialize(&mut self, sample_rate: f32) {
        self.low_filter = Biquad::new(sample_rate);
        self.mid_filter = Biquad::new(sample_rate);
        self.high_filter = Biquad::new(sample_rate);
        self.presence_filter = Biquad::new(sample_rate);
        self.resonance_filter = Biquad::new(sample_rate);
        self.reset();
    }

    pub fn reset(&mut self) {
        self.low_filter.reset();
        self.mid_filter.reset();
        self.high_filter.reset();
        self.presence_filter.reset();
        self.resonance_filter.reset();
        self.last_eq_values = [-999.0; 5];
    }

    fn update_coefficients(&mut self) {
        // パラメータ借用を最小限にする
        let (l, m, h, p, r) = {
            let eq = &self.params.eq_section;
            (
                eq.low.value(),
                eq.mid.value(),
                eq.high.value(),
                eq.presence.value(),
                eq.resonance.value(),
            )
        };

        if (l - self.last_eq_values[0]).abs() > 0.01
            || (m - self.last_eq_values[1]).abs() > 0.01
            || (h - self.last_eq_values[2]).abs() > 0.01
            || (p - self.last_eq_values[3]).abs() > 0.01
            || (r - self.last_eq_values[4]).abs() > 0.01
        {
            // --- モダンメタル・ベース・セッティング ---

            // Resonance (Sub-Bass): 60Hz.
            // 45Hzだと低すぎてスピーカーが飛ばし気味になるため、
            // 60Hz付近をPeakingで調整し、重低音の「圧」をコントロール。
            self.resonance_filter
                .set_params(FilterType::Peaking(r * 0.8), 60.0, 1.2);

            // Low: 120Hz.
            // 80Hzより少し上の120Hzをシェルフで動かすことで、
            // キックドラムとの住み分けをしながらベースの「ボディ」を太くします。
            self.low_filter
                .set_params(FilterType::LowShelf(l), 120.0, 0.707);

            // Mid: 350Hz - 500Hz (Variable focus).
            // モダンメタルではこの付近を「削る（Scoop）」ことで、歪みの濁りを取り、
            // 逆にブーストすると「ゴリッ」としたパーカッシブな質感が出ます。
            // Q値を少し広め(0.4)にして自然な変化に。
            self.mid_filter
                .set_params(FilterType::Peaking(m), 450.0, 0.4);

            // High: 1.5kHz - 2.5kHz.
            // ダークグラス系サウンドの核心。ピックのガリガリ音（Clank）が出る帯域。
            // ここをブーストすることで、ハイゲインギターの中でもベースが埋もれません。
            self.high_filter
                .set_params(FilterType::Peaking(h), 2200.0, 0.8);

            // Presence: 5kHz High Shelf.
            // 指が弦を擦る音や、ディストーションのジリジリした「エッジ」を調整。
            self.presence_filter
                .set_params(FilterType::HighShelf(p), 5000.0, 0.707);

            self.last_eq_values = [l, m, h, p, r];
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.update_coefficients();

        // 処理順序も重要：低い周波数から順に整えていく
        let mut signal = input;

        signal = self.resonance_filter.process(signal);
        signal = self.low_filter.process(signal);
        signal = self.mid_filter.process(signal);
        signal = self.high_filter.process(signal);
        signal = self.presence_filter.process(signal);

        signal
    }
}
