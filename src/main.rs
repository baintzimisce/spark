//! Spark — Pure-Rust Multi-Agent Desktop Command Deck.

use eframe::egui;
use spark::{app, theme};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Spark — Multi-Agent Command Deck")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([840.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Spark",
        options,
        Box::new(|cc| {
            theme::apply_impeccable_theme(&cc.egui_ctx);
            Ok(Box::new(app::SparkApp::new(cc.egui_ctx.clone())))
        }),
    )
}

#[cfg(test)]
mod tests {
    use spark::theme;

    #[test]
    fn theme_tokens_are_distinct() {
        assert_ne!(theme::OBSIDIAN_BG, theme::OBSIDIAN_RAISED);
        assert_ne!(theme::MIST_BLUE, theme::VERDIGRIS_PATINA);
    }
}
