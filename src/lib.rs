use truce::prelude::*;
mod amp;
mod editor;
mod modules;
mod params;
mod plugin;
mod utils;

use amp::XrossGuitarAmp;
use params::XrossGuitarAmpParams;

truce::plugin! {
    logic: XrossGuitarAmp,
    params: XrossGuitarAmpParams,
    bus_layouts: [BusLayout::new()
        .with_input("Main", ChannelConfig::Mono)
        .with_output("Main", ChannelConfig::Stereo)
    ],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_runs() {
        let result = truce_test::render_effect::<Plugin>(512, 44100.0);
        truce_test::assert_no_nans(&result.output);
    }

    #[test]
    fn renders_nonzero_output() {
        let result = truce_test::render_effect::<Plugin>(512, 44100.0);
        truce_test::assert_nonzero(&result.output);
    }

    #[test]
    fn bus_config_effect() {
        truce_test::assert_bus_config_effect::<Plugin>();
    }
    #[test]
    fn info_is_valid() {
        truce_test::assert_valid_info::<Plugin>();
    }

    #[test]
    fn has_editor() {
        truce_test::assert_has_editor::<Plugin>();
    }

    #[test]
    fn state_round_trips() {
        truce_test::assert_state_round_trip::<Plugin>();
    }

    #[test]
    fn param_defaults_match() {
        truce_test::assert_param_defaults_match::<Plugin>();
    }

    #[test]
    fn no_duplicate_param_ids() {
        truce_test::assert_no_duplicate_param_ids::<Plugin>();
    }

    #[test]
    fn corrupt_state_no_crash() {
        truce_test::assert_corrupt_state_no_crash::<Plugin>();
    }

    #[test]
    fn param_normalized_clamped() {
        truce_test::assert_param_normalized_clamped::<Plugin>();
    }
}
