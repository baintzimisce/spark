//! Impeccable (Obsidian & Mist Blue) design tokens, typography, and spacing for Spark.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke,
    TextStyle, Visuals,
};
use std::sync::Arc;

pub const OBSIDIAN_BG: Color32 = Color32::from_rgb(0x10, 0x12, 0x16);
pub const OBSIDIAN_DEEP: Color32 = Color32::from_rgb(0x0B, 0x0D, 0x10);
pub const OBSIDIAN_RAISED: Color32 = Color32::from_rgb(0x17, 0x1A, 0x20);
pub const USER_SURFACE: Color32 = Color32::from_rgb(0x14, 0x17, 0x1D);
pub const SLATE_CONTROL: Color32 = Color32::from_rgb(0x20, 0x24, 0x2C);
pub const SLATE_HOVER: Color32 = Color32::from_rgb(0x29, 0x2E, 0x38);

pub const MIST_BLUE: Color32 = Color32::from_rgb(0x7A, 0xA2, 0xF7);
pub const MIST_BRIGHT: Color32 = Color32::from_rgb(0x93, 0xB4, 0xFA);
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x0E, 0x13, 0x1D);
pub const VERDIGRIS_PATINA: Color32 = Color32::from_rgb(0x4D, 0xB6, 0xAC);

pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(0xEC, 0xEE, 0xF2);
pub const TEXT_BODY: Color32 = Color32::from_rgb(0xD5, 0xD9, 0xE0);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x9D, 0xA5, 0xB4);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x71, 0x78, 0x87);
pub const ERROR_CORAL: Color32 = Color32::from_rgb(0xE0, 0x6C, 0x75);

pub const HAIRLINE: Color32 = Color32::from_rgb(0x27, 0x2C, 0x36);
pub const HAIRLINE_SUBTLE: Color32 = Color32::from_rgb(0x1F, 0x23, 0x2B);
pub const HAIRLINE_HOVER: Color32 = Color32::from_rgb(0x3A, 0x41, 0x50);

pub const ACCENT_SOFT_BG: Color32 = Color32::from_rgb(0x18, 0x21, 0x33);
pub const ACCENT_SOFT_BORDER: Color32 = Color32::from_rgb(0x3B, 0x52, 0x80);
pub const VERDIGRIS_SOFT_BG: Color32 = Color32::from_rgb(0x12, 0x26, 0x24);
pub const CORAL_SOFT_BG: Color32 = Color32::from_rgb(0x2C, 0x18, 0x1B);
pub const MODAL_BACKDROP: Color32 = Color32::from_rgba_premultiplied(0x04, 0x05, 0x08, 0xB8);

/// Apply the Impeccable Obsidian & Mist Blue theme, typography, and spacing to an egui context.
pub fn apply_impeccable_theme(ctx: &egui::Context) {
    install_system_fonts(ctx);

    let mut style = (*ctx.style()).clone();

    // Typography hierarchy
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(17.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(12.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(12.5, FontFamily::Monospace),
    );

    // Ergonomic desktop spacing & thin scrollbars
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.window_margin = egui::Margin::same(16);
    style.spacing.interact_size = egui::vec2(30.0, 24.0);
    style.spacing.combo_height = 260.0;
    style.spacing.scroll = egui::style::ScrollStyle::thin();

    // Visuals: Obsidian surfaces with calm Mist Blue focus & primary actions
    let mut visuals = Visuals::dark();

    visuals.panel_fill = OBSIDIAN_BG;
    visuals.window_fill = OBSIDIAN_RAISED;
    visuals.extreme_bg_color = OBSIDIAN_DEEP;
    visuals.faint_bg_color = USER_SURFACE;
    visuals.code_bg_color = OBSIDIAN_DEEP;

    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.window_stroke = Stroke::new(1.0_f32, HAIRLINE_HOVER);
    visuals.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 150),
    };
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 130),
    };

    visuals.widgets.noninteractive.bg_fill = OBSIDIAN_RAISED;
    visuals.widgets.noninteractive.weak_bg_fill = OBSIDIAN_RAISED;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, HAIRLINE_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_BODY);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = SLATE_CONTROL;
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, HAIRLINE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_BODY);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = SLATE_HOVER;
    visuals.widgets.hovered.weak_bg_fill = SLATE_CONTROL;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, HAIRLINE_HOVER);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_BRIGHT);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
    visuals.widgets.hovered.expansion = 0.0;

    visuals.widgets.active.bg_fill = SLATE_HOVER;
    visuals.widgets.active.weak_bg_fill = SLATE_HOVER;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, MIST_BLUE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT_BRIGHT);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);
    visuals.widgets.active.expansion = 0.0;

    visuals.widgets.open.bg_fill = SLATE_HOVER;
    visuals.widgets.open.weak_bg_fill = SLATE_HOVER;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, ACCENT_SOFT_BORDER);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, TEXT_BRIGHT);
    visuals.widgets.open.corner_radius = CornerRadius::same(6);

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(0x7A, 0xA2, 0xF7, 0x3D);
    visuals.selection.stroke = Stroke::new(1.0_f32, MIST_BLUE);
    visuals.text_cursor.stroke = Stroke::new(1.5_f32, MIST_BLUE);

    style.visuals = visuals;
    ctx.set_style(style);
}

fn install_system_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let sans_candidates = [
        r"C:\Windows\Fonts\SegUIVar.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
        "/System/Library/Fonts/SFNS.ttf",
        "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];
    if let Some((name, bytes)) = load_first_existing_font("spark_ui_sans", &sans_candidates) {
        fonts
            .font_data
            .insert(name.clone(), Arc::new(FontData::from_owned(bytes)));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, name);
        }
    }

    let mono_candidates = [
        r"C:\Windows\Fonts\CascadiaMono.ttf",
        r"C:\Windows\Fonts\CascadiaCode.ttf",
        r"C:\Windows\Fonts\consola.ttf",
        "/System/Library/Fonts/SFNSMono.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ];
    if let Some((name, bytes)) = load_first_existing_font("spark_ui_mono", &mono_candidates) {
        fonts
            .font_data
            .insert(name.clone(), Arc::new(FontData::from_owned(bytes)));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
            list.insert(0, name);
        }
    }

    ctx.set_fonts(fonts);
}

fn load_first_existing_font(font_name: &str, candidates: &[&str]) -> Option<(String, Vec<u8>)> {
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if !bytes.is_empty() {
                return Some((font_name.to_string(), bytes));
            }
        }
    }
    None
}
