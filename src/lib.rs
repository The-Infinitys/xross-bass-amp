use truce::prelude::*;
mod amp;
mod editor;
mod modules;
mod params;
mod plugin;
mod utils;

use amp::XrossBassAmp;
use params::XrossBassAmpParams;

truce::plugin! {
    logic: XrossBassAmp,
    params: XrossBassAmpParams,
}
