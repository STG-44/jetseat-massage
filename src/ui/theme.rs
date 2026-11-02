use eframe::egui::{self, Color32, FontId, Stroke, Vec2};
pub const BG: Color32 = Color32::from_rgb(20, 22, 25);
pub const CARD: Color32 = Color32::from_rgb(26, 29, 33);
pub const ACCENT: Color32 = Color32::from_rgb(17, 194, 174);
pub const MUTED: Color32 = Color32::from_rgb(151, 160, 169);
pub const LINE: Color32 = Color32::from_rgb(48, 54, 61);
pub const WARNING: Color32 = Color32::from_rgb(237, 174, 101);
pub fn configure(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = BG;
    visuals.override_text_color = Some(Color32::from_rgb(234, 237, 240));
    visuals.selection.bg_fill = Color32::from_rgb(27, 72, 67);
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(37, 44, 49);
    visuals.widgets.active.bg_fill = Color32::from_rgb(29, 76, 69);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(4);
    }
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.animation_time = 0.0;
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(22.0));
    });
}
