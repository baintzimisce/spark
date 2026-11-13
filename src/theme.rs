//! Impeccable (Neo Kinpaku) design tokens for Spark.

use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};

pub const LACQUER_BLACK: Color32 = Color32::from_rgb(0x12, 0x11, 0x0E);
pub const LACQUER_DEEP: Color32 = Color32::from_rgb(0x0A, 0x09, 0x07);
pub const RAISED_LACQUER: Color32 = Color32::from_rgb(0x1C, 0x1A, 0x16);
pub const GRAPHITE: Color32 = Color32::from_rgb(0x26, 0x24, 0x1F);
pub const GRAPHITE_2: Color32 = Color32::from_rgb(0x30, 0x2E, 0x28);

pub const KINPAKU_GOLD: Color32 = Color32::from_rgb(0xEA, 0xB3, 0x08);
pub const DARK_INK: Color32 = Color32::from_rgb(0x24, 0x22, 0x1B);
pub const VERDIGRIS_PATINA: Color32 = Color32::from_rgb(0x4D, 0xB6, 0xAC);

pub const CHAMPAGNE: Color32 = Color32::from_rgb(0xE8, 0xE8, 0xE8);
pub const TEXT_WARM: Color32 = Color32::from_rgb(0xDF, 0xDF, 0xDF);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0xB8, 0xB8, 0xB8);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x9E, 0x9E, 0x9E);
pub const ERROR_CORAL: Color32 = Color32::from_rgb(0xE0, 0x6C, 0x75);
pub const HAIRLINE: Color32 = Color32::from_rgb(0x2E, 0x2C, 0x26);

/// Apply the Impeccable Neo Kinpaku dark lacquer theme to an egui context.
pub fn apply_impeccable_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.panel_fill = LACQUER_BLACK;
    visuals.window_fill = RAISED_LACQUER;
    visuals.extreme_bg_color = LACQUER_DEEP;
    visuals.faint_bg_color = RAISED_LACQUER;

    visuals.widgets.noninteractive.bg_fill = RAISED_LACQUER;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, HAIRLINE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_WARM);

    visuals.widgets.inactive.bg_fill = GRAPHITE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, HAIRLINE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_WARM);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = GRAPHITE_2;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, KINPAKU_GOLD);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, CHAMPAGNE);

    visuals.widgets.active.bg_fill = KINPAKU_GOLD;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, DARK_INK);

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(0xEA, 0xB3, 0x08, 0x40);
    visuals.selection.stroke = Stroke::new(1.0_f32, KINPAKU_GOLD);

    ctx.set_visuals(visuals);
}
