use std::sync::Arc;
use truce::prelude::*;

pub mod cab;
pub mod eq;
pub mod gain;

pub use cab::CabProcessor;
pub use eq::EqProcessor;
pub use gain::GainProcessor;

use crate::params::XrossBassAmpParams;

pub struct XrossBassAmp {
    params: Arc<XrossBassAmpParams>,
    gain_proc: GainProcessor,
    eq_proc: EqProcessor,
    cab_proc: CabProcessor,

    // 内部処理用のモノラル一時バッファ（ヒープ確保を避けるため再利用）
    internal_buffer: Vec<f32>,
}

impl XrossBassAmp {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        Self {
            gain_proc: GainProcessor::new(params.clone()),
            eq_proc: EqProcessor::new(params.clone()),
            cab_proc: CabProcessor::new(params.clone()),
            params,
            internal_buffer: Vec::with_capacity(512), // 一般的なバッファサイズで初期化
        }
    }

    pub fn initialize_truce(&mut self, sr: f64, max_block_size: usize) {
        let sample_rate = sr as f32;
        self.gain_proc.initialize(sample_rate);
        self.eq_proc.initialize(sample_rate);
        self.cab_proc.initialize(sample_rate);

        // 最大ブロックサイズに合わせてバッファを確保
        self.internal_buffer.resize(max_block_size, 0.0);
    }

    pub fn process_truce(&mut self, buffer: &mut AudioBuffer) -> ProcessStatus {
        let num_samples = buffer.num_samples();
        let input_channels = buffer.num_input_channels();
        let out_channels = buffer.num_output_channels();

        if num_samples == 0 || input_channels == 0 || out_channels == 0 {
            return ProcessStatus::Normal;
        }
        {
            let (input, output) = buffer.io(0);
            for i in 0..num_samples {
                output[i] = input[i];
            }
        }
        let output_l = buffer.output(0);
        self.gain_proc.process(output_l);
        self.eq_proc.process(output_l);

        // // 3. キャビネット・ステレオ展開処理 (AudioBufferを直接渡す)
        self.cab_proc.process_truce(buffer);

        ProcessStatus::Normal
    }

    pub fn params(&self) -> Arc<XrossBassAmpParams> {
        self.params.clone()
    }

    pub fn ui(&self) -> Box<dyn Editor> {
        crate::editor::create_editor(self.params())
    }
}
