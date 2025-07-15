use std::error::Error;

use videofinder::videofinder_main;

fn main() -> Result<(), Box<dyn Error>> {
    videofinder_main()
}
