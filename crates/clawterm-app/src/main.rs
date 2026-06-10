mod app;
mod backend;
mod state;

use eframe::egui;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "clawterm=info".into()),
        )
        .init();

    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (ev_tx, ev_rx) = std::sync::mpsc::channel();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([640.0, 400.0])
            .with_title("ClawTerm"),
        ..Default::default()
    };

    eframe::run_native(
        "ClawTerm",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            let repaint_ctx = cc.egui_ctx.clone();
            backend::spawn(cmd_rx, ev_tx, move || repaint_ctx.request_repaint());
            Ok(Box::new(app::ClawTermApp::new(cmd_tx, ev_rx)))
        }),
    )
}
