mod login;
mod server;

use std::sync::Arc;

use crate::app::App;
use crate::ui::MainWindow;

pub fn bind(window: &MainWindow, app: Arc<App>) {
    server::bind(window, app.clone());
    login::bind(window, app);
}
