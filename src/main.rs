use nih_plug::wrapper::standalone::nih_export_standalone;
use xross_bass_amp::XrossBassAmp;

fn main() {
    nih_export_standalone::<XrossBassAmp>();
}
