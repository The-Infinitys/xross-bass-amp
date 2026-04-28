use nih_plug::plugin::Plugin;
use nih_plug::prelude::*;

use crate::XrossGuitarAmp;
use crate::editor::create_editor;

impl Plugin for XrossGuitarAmp {
    const NAME: &'static str = "Xross Guitar Amp";

    const VENDOR: &'static str = "The Infinitys";

    const URL: &'static str = "https://github.com/The-Infinitys/xross-guitar-amp";

    const EMAIL: &'static str = "the.infinity.s.infinity@gmail.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    fn params(&self) -> std::sync::Arc<dyn nih_plug::prelude::Params> {
        self.params()
    }

    const MIDI_INPUT: MidiConfig = MidiConfig::None;

    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    const HARD_REALTIME_ONLY: bool = false;

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        // In the default implementation we can simply ignore the value
        Box::new(|_| ())
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        create_editor(self.params())
    }

    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.initialize(audio_io_layout, buffer_config, context)
    }

    fn reset(&mut self) {
        self.reset();
    }

    fn deactivate(&mut self) {}

    const AUDIO_IO_LAYOUTS: &'static [nih_plug::prelude::AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(1),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    type SysExMessage = ();

    type BackgroundTask = ();

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.process(buffer, aux, context)
    }
}

impl Vst3Plugin for XrossGuitarAmp {
    const VST3_CLASS_ID: [u8; 16] = *b"Xross Guitar Amp";

    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Distortion,
        Vst3SubCategory::Fx,
        Vst3SubCategory::Custom("guitar"),
    ];
}
impl ClapPlugin for XrossGuitarAmp {
    const CLAP_ID: &'static str = "Xross Guitar Amp";

    const CLAP_DESCRIPTION: Option<&'static str> = Some("Modern Guitar Amplifier");

    const CLAP_MANUAL_URL: Option<&'static str> =
        Some("https://github.com/The-Infinitys/xross-guitar-amp");

    const CLAP_SUPPORT_URL: Option<&'static str> =
        Some("https://github.com/The-Infinitys/xross-guitar-amp/issues");

    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,      // ← これを必ず追加（メイン）
        ClapFeature::Distortion,       // 歪みアンプとして重要
        ClapFeature::Custom("guitar"), // または "amp" / "guitar-amp"（ホストでわかりやすく）
    ];
}

nih_export_clap!(XrossGuitarAmp);
nih_export_vst3!(XrossGuitarAmp);
