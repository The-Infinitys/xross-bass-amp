use crate::amp::XrossBassAmp;
use truce::prelude::*;

impl PluginLogic for XrossBassAmp {
    fn reset(&mut self, sample_rate: f64, max_block_size: usize) {
        self.initialize_truce(sample_rate, max_block_size);
    }

    fn process(
        &mut self,
        buffer: &mut AudioBuffer,
        events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        events
            .iter()
            .map(|event| &event.body)
            .for_each(|event| match event {
                EventBody::ParamChange { id, value } => {
                    self.params().set_normalized(*id, *value);
                }
                _ => {}
            });

        self.process_truce(buffer)
    }

    fn custom_editor(&self) -> Option<Box<dyn Editor>> {
        Some(self.ui())
    }
}
