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

        // --- 1. Copy Input for DI/Dry Blending ---
        if self.internal_buffer.len() < num_samples {
            self.internal_buffer.resize(num_samples, 0.0);
        }
        {
            let input = buffer.input(0);
            for i in 0..num_samples {
                self.internal_buffer[i] = input[i];
            }
        }

        // --- 2. Main Signal Path (Amp Head) ---
        // Copy input to output channel 0 for processing
        {
            let (input, output) = buffer.io(0);
            for i in 0..num_samples {
                output[i] = input[i];
            }
        }
        let output_l = buffer.output(0);
        self.gain_proc.process(output_l);
        self.eq_proc.process(output_l);

        // --- 3. DI Blending (Clean Mix) ---
        let di_mix = self.params.di_mix.value();
        if di_mix > 0.0 {
            let output_l = buffer.output(0);
            for i in 0..num_samples {
                // DI信号（クリーン）を歪み/EQ後の信号にミックス
                output_l[i] = output_l[i] * (1.0 - di_mix) + self.internal_buffer[i] * di_mix;
            }
        }

        // --- 4. Cabinet & Stereo Processing ---
        // CabProcessor handles stereo expansion
        self.cab_proc.process_truce(buffer);

        // --- 5. Final Dry/Wet Mix ---
        let final_mix = self.params.mix.value();
        if final_mix < 1.0 {
            for ch in 0..out_channels {
                let out = buffer.output(ch);
                for i in 0..num_samples {
                    out[i] = out[i] * final_mix + self.internal_buffer[i] * (1.0 - final_mix);
                }
            }
        }

        ProcessStatus::Normal
    }

    pub fn params(&self) -> Arc<XrossBassAmpParams> {
        self.params.clone()
    }
    pub fn ui(&self) -> Box<dyn Editor> {
        crate::editor::create_editor(self.params())
    }
}
