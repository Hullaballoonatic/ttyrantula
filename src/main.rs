mod app;
mod renderer;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    app::run()
}
