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

    /// 完全なクリーン音（インプット直）の保持用
    clean_buffer: Vec<f32>,
    /// Gain/Eqを通った後の「キャビなし歪み音」の保持用
    head_buffer: Vec<f32>,
}

impl XrossBassAmp {
    pub fn new(params: Arc<XrossBassAmpParams>) -> Self {
        Self {
            gain_proc: GainProcessor::new(params.clone()),
            eq_proc: EqProcessor::new(params.clone()),
            cab_proc: CabProcessor::new(params.clone()),
            params,
            clean_buffer: Vec::with_capacity(512),
            head_buffer: Vec::with_capacity(512),
        }
    }

    pub fn initialize_truce(&mut self, sr: f64, max_block_size: usize) {
        let sample_rate = sr as f32;
        self.gain_proc.initialize(sample_rate);
        self.eq_proc.initialize(sample_rate);
        self.cab_proc.initialize(sample_rate);

        self.clean_buffer.resize(max_block_size, 0.0);
        self.head_buffer.resize(max_block_size, 0.0);
    }

    pub fn process_truce(&mut self, buffer: &mut AudioBuffer) -> ProcessStatus {
        let num_samples = buffer.num_samples();
        let out_channels = buffer.num_output_channels();

        if num_samples == 0 || out_channels == 0 {
            return ProcessStatus::Normal;
        }

        // バッファサイズの安全確保
        if self.clean_buffer.len() < num_samples {
            self.clean_buffer.resize(num_samples, 0.0);
            self.head_buffer.resize(num_samples, 0.0);
        }

        // --- 1. 原音(Clean DI)をキャプチャ ---
        {
            let input = buffer.input(0);
            self.clean_buffer[..num_samples].copy_from_slice(&input[..num_samples]);
        }

        // --- 2. Amp Head処理 (Gain -> EQ) ---
        // head_bufferに歪みサウンドを作成する
        self.head_buffer[..num_samples].copy_from_slice(&self.clean_buffer[..num_samples]);
        self.gain_proc.process(&mut self.head_buffer[..num_samples]);
        self.eq_proc.process(&mut self.head_buffer[..num_samples]);

        // --- 3. DI Mix (Clean vs 歪みライン音) ---
        // ここで「歪んでいるがキャビを通っていない音」を決定する
        let di_mix = self.params.di_mix.value();
        let mut mixed_head_signal = vec![0.0f32; num_samples]; // 一時的なミックス用

        for i in 0..num_samples {
            // di_mix = 0.0 で全歪み、1.0 で全クリーン（一般的なDIブレンドの逆ならここを調整）
            // ここでは di_mix 0.0(Drive 100%) ~ 1.0(Clean 100%) と仮定
            mixed_head_signal[i] =
                self.head_buffer[i] * (1.0 - di_mix) + self.clean_buffer[i] * di_mix;
        }

        // --- 4. Cabinet Processing ---
        // mixed_head_signal を output(0) に戻してキャビに通す
        {
            let output_l = buffer.output(0);
            output_l[..num_samples].copy_from_slice(&mixed_head_signal);
        }

        // CabProcessor内でステレオ化やIR畳み込みが行われる想定
        self.cab_proc.process_truce(buffer);

        // --- 5. Speaker Mix (Final Dry/Wet) ---
        // キャビを通した後の音と、さきほどの mixed_head_signal (キャビなし) を混ぜる
        let speaker_mix = self.params.speaker_mix.value();
        if speaker_mix < 1.0 {
            for ch in 0..out_channels {
                let out = buffer.output(ch);
                for (i, sample) in out.iter_mut().enumerate().take(num_samples) {
                    // speaker_mix = 1.0 でフルキャビ、0.0 でキャビなし（ライン歪み/クリーン）
                    *sample = *sample * speaker_mix + mixed_head_signal[i] * (1.0 - speaker_mix);
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
