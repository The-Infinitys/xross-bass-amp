use crate::modules::filter::{Biquad, FilterType};
use crate::params::XrossBassAmpParams;
use std::sync::Arc;
use truce::params::FloatParamReadF32;

pub struct EqProcessor {
    pub params: Arc<XrossBassAmpParams>,

    // フィルタ群
    low_filter: Biquad,
    mid_filter: Biquad,
    high_filter: Biquad,
    presence_filter: Biquad,
    resonance_filter: Biquad,

    // パラメータ変更検知用キャッシュ
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

    /// パラメータの変更をチェックし、必要であれば係数を更新する
    fn update_coefficients_if_needed(&mut self) {
        let l = self.params.eq_low.value();
        let m = self.params.eq_mid.value();
        let h = self.params.eq_high.value();
        let p = self.params.presence.value();
        let r = self.params.resonance.value();

        // 差分チェック
        if (l - self.last_eq_values[0]).abs() > 0.01
            || (m - self.last_eq_values[1]).abs() > 0.01
            || (h - self.last_eq_values[2]).abs() > 0.01
            || (p - self.last_eq_values[3]).abs() > 0.01
            || (r - self.last_eq_values[4]).abs() > 0.01
        {
            // Low: 150Hz
            self.low_filter
                .set_params(FilterType::LowShelf(l), 150.0, 0.707);

            // Mid: 700Hz
            self.mid_filter
                .set_params(FilterType::Peaking(m), 700.0, 0.4);

            // High: 3kHz
            self.high_filter
                .set_params(FilterType::HighShelf(h), 3000.0, 0.707);

            // Presence: 6.5kHz
            self.presence_filter
                .set_params(FilterType::HighShelf(p), 6500.0, 0.8);

            // Resonance: 80Hz
            self.resonance_filter
                .set_params(FilterType::Peaking(r), 80.0, 1.5);

            self.last_eq_values = [l, m, h, p, r];
        }
    }

    /// バッファを一括処理する
    pub fn process(&mut self, buffer: &mut [f32]) {
        // バッファ処理の開始前に一度だけパラメータチェック
        self.update_coefficients_if_needed();

        // フィルタを直列に適用
        // サンプルごとのループ内で全フィルタを通す
        for sample in buffer.iter_mut() {
            let mut s = *sample;

            s = self.resonance_filter.process(s);
            s = self.low_filter.process(s);
            s = self.mid_filter.process(s);
            s = self.high_filter.process(s);
            s = self.presence_filter.process(s);

            *sample = s;
        }
    }
}
