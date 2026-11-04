mod api;
mod app;
mod ui;

use std::sync::Arc;

use slint::ComponentHandle;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Arc::new(app::App::default());

    let window = ui::MainWindow::new()?;
    ui::bind(&window, app);

    window.window().set_fullscreen(true);
    window.run()?;
    Ok(())
}
