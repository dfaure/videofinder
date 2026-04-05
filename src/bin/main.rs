use std::error::Error;

use videofinder::videofinder_main;

fn main() -> Result<(), Box<dyn Error>> {
    // Prevent tracing (used by winit) from forwarding debug spam to the log crate.
    // Installing any subscriber disables the tracing->log bridge.
    tracing::subscriber::set_global_default(tracing::subscriber::NoSubscriber::new()).ok();

    // Log everything to stderr
    // (I use flexi_logger because on Android I need to log to a file)
    let _logger = flexi_logger::Logger::with(flexi_logger::LevelFilter::Debug).start().unwrap();

    videofinder_main()
}
