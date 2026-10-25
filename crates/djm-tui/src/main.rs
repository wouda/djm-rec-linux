use djm_common::config::AppConfig;
use djm_common::protocol::default_socket_path;
use djm_tui::app::TuiApp;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = AppConfig::default_config_path();
    let config = AppConfig::load_or_create(&config_path).unwrap_or_default();
    let socket_path = default_socket_path();

    let mut app = TuiApp::new(socket_path, config);
    app.run().await?;
    Ok(())
}
