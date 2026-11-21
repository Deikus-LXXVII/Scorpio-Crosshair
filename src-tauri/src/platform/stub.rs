//! Other systems (development on macOS): nothing is shown or listened to, so
//! the core and the settings window can still be built and tested.

use super::{Command, Event, Platform, Unsupported};
use std::sync::mpsc::{channel, Sender};

pub fn start(_events: Sender<Event>) -> Result<Platform, Unsupported> {
    let (commands, _receiver) = channel::<Command>();
    std::mem::forget(_receiver);
    Ok(Platform { commands, wake: Box::new(|| {}), list_windows: Box::new(Vec::new) })
}
