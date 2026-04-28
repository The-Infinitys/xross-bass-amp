use nih_plug::prelude::*;
use std::sync::Arc;

pub mod cab;
pub mod eq;
pub mod gain;

pub use cab::CabProcessor;
pub use eq::EqProcessor;
pub use gain::GainProcessor;

use crate::params::XrossGuitarAmpParams;
pub struct XrossGuitarAmp {
    params: Arc<XrossGuitarAmpParams>,
    gain_proc: GainProcessor,
    eq_proc: EqProcessor,
    cab_proc: CabProcessor,
}

impl Default for XrossGuitarAmp {
    fn default() -> Self {
        let params = Arc::new(XrossGuitarAmpParams::default());
        Self {
            gain_proc: GainProcessor::new(params.clone()),
            eq_proc: EqProcessor::new(params.clone()),
            cab_proc: CabProcessor::new(params.clone()),
            params,
        }
    }
}

impl XrossGuitarAmp {
    pub fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let sample_rate = buffer_config.sample_rate;
        self.gain_proc.initialize(sample_rate);
        self.eq_proc.initialize(sample_rate);
        self.cab_proc.initialize(sample_rate);
        true
    }
    pub fn reset(&mut self) {
        self.gain_proc.reset();
        self.eq_proc.reset();
        self.cab_proc.reset();
    }
    pub fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // チャンネル数の確認（安全のため）
        let num_channels = buffer.channels();

        for mut channel_samples in buffer.iter_samples() {
            // 1. 入力信号の取得 (モノラル入力なのでインデックス0を固定参照)
            let input = *channel_samples.get_mut(0).map_or(&0.0, |s| s);

            // 2. モノラル処理チェーン
            // Gain, EQ は内部で self.params を参照して処理
            let mut mono_signal = self.gain_proc.process(input);
            mono_signal = self.eq_proc.process(mono_signal);

            // 3. ステレオ分岐処理
            // CabProcessor がステレオ出力を返すように設計されている場合
            let (left_out, right_out) = self.cab_proc.process(mono_signal);

            // 4. 出力バッファへの書き込み
            // Lチャンネル (0)
            if let Some(l) = channel_samples.get_mut(0) {
                *l = left_out;
            }
            // Rチャンネル (1) - ここでステレオ化が完了
            if num_channels >= 2
                && let Some(r) = channel_samples.get_mut(1)
            {
                *r = right_out;
            }
        }

        ProcessStatus::Normal
    }
    pub fn params(&self) -> Arc<XrossGuitarAmpParams> {
        self.params.clone()
    }
}
