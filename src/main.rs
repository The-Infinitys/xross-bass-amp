use nih_plug::wrapper::standalone::nih_export_standalone;
use xross_guitar_amp::XrossGuitarAmp;

fn main() {
    nih_export_standalone::<XrossGuitarAmp>();
}
