use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_POINT_2F, D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_UNKNOWN, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_NONE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
    D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT,
    D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE, D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget,
    ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
    IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows::core::{PCWSTR, Result, w};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayTheme {
    Light,
    Dark,
}

pub struct OverlayVisual {
    pub theme: OverlayTheme,
    pub title: String,
    pub subtitle: String,
    pub hint: String,
    pub badge: String,
}

struct OverlayRenderer {
    d2d_factory: ID2D1Factory,
    dwrite_factory: IDWriteFactory,
    render_target: Option<ID2D1HwndRenderTarget>,
    badge_format: IDWriteTextFormat,
    title_format: IDWriteTextFormat,
    subtitle_format: IDWriteTextFormat,
    keycap_format: IDWriteTextFormat,
    hint_format: IDWriteTextFormat,
    target_size: D2D_SIZE_U,
    target_dpi: u32,
}

#[derive(Clone, Copy)]
struct Palette {
    background: D2D1_COLOR_F,
    badge: D2D1_COLOR_F,
    title: D2D1_COLOR_F,
    subtitle: D2D1_COLOR_F,
    key_fill: D2D1_COLOR_F,
    key_border: D2D1_COLOR_F,
    key_text: D2D1_COLOR_F,
    hint: D2D1_COLOR_F,
}

static RENDERER: OnceLock<Mutex<OverlayRenderer>> = OnceLock::new();

pub fn discard_overlay_renderer() {
    if let Some(lock) = RENDERER.get() {
        if let Ok(mut renderer) = lock.lock() {
            renderer.render_target = None;
            renderer.target_size = D2D_SIZE_U {
                width: 0,
                height: 0,
            };
            renderer.target_dpi = 0;
        }
    }
}

pub unsafe fn draw_overlay(hwnd: HWND, visual: &OverlayVisual, dpi: u32) -> Result<()> {
    let lock = RENDERER.get_or_init(|| Mutex::new(OverlayRenderer::new().unwrap()));
    let mut renderer = lock.lock().unwrap();
    renderer.draw(hwnd, visual, dpi.max(96))
}

impl OverlayRenderer {
    fn new() -> Result<Self> {
        unsafe {
            let d2d_factory =
                D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite_factory = DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED)?;

            let badge_format = create_text_format(
                &dwrite_factory,
                "Segoe UI Variable Text",
                "Segoe UI",
                DWRITE_FONT_WEIGHT_MEDIUM,
                10.5,
            )?;
            let title_format = create_text_format(
                &dwrite_factory,
                "Segoe UI Variable Display",
                "Segoe UI",
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                17.5,
            )?;
            let subtitle_format = create_text_format(
                &dwrite_factory,
                "Segoe UI Variable Text",
                "Segoe UI",
                DWRITE_FONT_WEIGHT_MEDIUM,
                12.5,
            )?;
            let keycap_format = create_text_format(
                &dwrite_factory,
                "Segoe UI Variable Text",
                "Segoe UI",
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                12.5,
            )?;
            let hint_format = create_text_format(
                &dwrite_factory,
                "Segoe UI Variable Text",
                "Segoe UI",
                DWRITE_FONT_WEIGHT_MEDIUM,
                12.0,
            )?;

            keycap_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            keycap_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            Ok(Self {
                d2d_factory,
                dwrite_factory,
                render_target: None,
                badge_format,
                title_format,
                subtitle_format,
                keycap_format,
                hint_format,
                target_size: D2D_SIZE_U {
                    width: 0,
                    height: 0,
                },
                target_dpi: 0,
            })
        }
    }

    unsafe fn draw(&mut self, hwnd: HWND, visual: &OverlayVisual, dpi: u32) -> Result<()> {
        self.ensure_target(hwnd, dpi)?;

        let palette = palette_for(visual.theme);
        let rt = self.render_target.as_ref().unwrap();
        let size = rt.GetSize();
        let width = size.width;
        let height = size.height;

        let keycap_alt = D2D1_ROUNDED_RECT {
            rect: rectf(18.0, height - 36.0, 56.0, height - 12.0),
            radiusX: 9.0,
            radiusY: 9.0,
        };
        let keycap_q = D2D1_ROUNDED_RECT {
            rect: rectf(64.0, height - 36.0, 94.0, height - 12.0),
            radiusX: 9.0,
            radiusY: 9.0,
        };

        rt.BeginDraw();
        rt.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
        rt.Clear(Some(&palette.background));

        let badge = solid_brush(rt, palette.badge)?;
        let title = solid_brush(rt, palette.title)?;
        let subtitle = solid_brush(rt, palette.subtitle)?;
        let key_fill = solid_brush(rt, palette.key_fill)?;
        let key_border = solid_brush(rt, palette.key_border)?;
        let key_text = solid_brush(rt, palette.key_text)?;
        let hint = solid_brush(rt, palette.hint)?;

        draw_text(
            &self.dwrite_factory,
            rt,
            &visual.badge,
            &self.badge_format,
            rectf(18.0, 12.0, width - 18.0, 24.0),
            &badge,
        )?;
        draw_text(
            &self.dwrite_factory,
            rt,
            &visual.title,
            &self.title_format,
            rectf(18.0, 28.0, width - 18.0, 52.0),
            &title,
        )?;
        draw_text(
            &self.dwrite_factory,
            rt,
            &visual.subtitle,
            &self.subtitle_format,
            rectf(18.0, 52.0, width - 18.0, 70.0),
            &subtitle,
        )?;

        rt.FillRoundedRectangle(&keycap_alt, &key_fill);
        rt.FillRoundedRectangle(&keycap_q, &key_fill);
        rt.DrawRoundedRectangle(&keycap_alt, &key_border, 1.0, None);
        rt.DrawRoundedRectangle(&keycap_q, &key_border, 1.0, None);

        draw_text(
            &self.dwrite_factory,
            rt,
            "Alt",
            &self.keycap_format,
            rectf(18.0, height - 36.0, 56.0, height - 12.0),
            &key_text,
        )?;
        draw_text(
            &self.dwrite_factory,
            rt,
            "Q",
            &self.keycap_format,
            rectf(64.0, height - 36.0, 94.0, height - 12.0),
            &key_text,
        )?;
        draw_text(
            &self.dwrite_factory,
            rt,
            &visual.hint,
            &self.hint_format,
            rectf(108.0, height - 34.0, width - 18.0, height - 10.0),
            &hint,
        )?;

        if rt.EndDraw(None, None).is_err() {
            self.render_target = None;
        }

        Ok(())
    }

    unsafe fn ensure_target(&mut self, hwnd: HWND, dpi: u32) -> Result<()> {
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        let width = (client.right - client.left).max(1) as u32;
        let height = (client.bottom - client.top).max(1) as u32;
        let size = D2D_SIZE_U { width, height };

        let needs_recreate = self.render_target.is_none() || self.target_dpi != dpi;
        if needs_recreate {
            self.render_target = Some(create_render_target(&self.d2d_factory, hwnd, size, dpi)?);
            self.target_size = size;
            self.target_dpi = dpi;
            return Ok(());
        }

        if self.target_size.width != size.width || self.target_size.height != size.height {
            if let Some(target) = &self.render_target {
                target.Resize(&size)?;
            }
            self.target_size = size;
        }

        Ok(())
    }
}

unsafe fn create_render_target(
    factory: &ID2D1Factory,
    hwnd: HWND,
    size: D2D_SIZE_U,
    dpi: u32,
) -> Result<ID2D1HwndRenderTarget> {
    let props = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_UNKNOWN,
        },
        dpiX: dpi as f32,
        dpiY: dpi as f32,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
        hwnd,
        pixelSize: size,
        presentOptions: D2D1_PRESENT_OPTIONS_NONE,
    };
    let target = factory.CreateHwndRenderTarget(&props, &hwnd_props)?;
    target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
    target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
    Ok(target)
}

unsafe fn create_text_format(
    factory: &IDWriteFactory,
    primary_family: &str,
    fallback_family: &str,
    weight: windows::Win32::Graphics::DirectWrite::DWRITE_FONT_WEIGHT,
    size: f32,
) -> Result<IDWriteTextFormat> {
    let primary = to_wide(primary_family);
    let fallback = to_wide(fallback_family);
    let locale = w!("zh-CN");
    let font_collection = Option::<&IDWriteFontCollection>::None;
    let format = factory
        .CreateTextFormat(
            PCWSTR(primary.as_ptr()),
            font_collection,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            locale,
        )
        .or_else(|_| {
            factory.CreateTextFormat(
                PCWSTR(fallback.as_ptr()),
                Option::<&IDWriteFontCollection>::None,
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                locale,
            )
        })?;
    format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
    format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
    format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
    Ok(format)
}

unsafe fn draw_text(
    factory: &IDWriteFactory,
    rt: &ID2D1HwndRenderTarget,
    text: &str,
    format: &IDWriteTextFormat,
    rect: D2D_RECT_F,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    let wide = text.encode_utf16().collect::<Vec<_>>();
    let layout = factory.CreateTextLayout(
        &wide,
        format,
        (rect.right - rect.left).max(1.0),
        (rect.bottom - rect.top).max(1.0),
    )?;
    rt.DrawTextLayout(
        point(rect.left, rect.top),
        &layout,
        brush,
        D2D1_DRAW_TEXT_OPTIONS_NONE,
    );
    Ok(())
}

unsafe fn solid_brush(
    rt: &ID2D1HwndRenderTarget,
    color: D2D1_COLOR_F,
) -> Result<ID2D1SolidColorBrush> {
    rt.CreateSolidColorBrush(&color, None)
}

fn palette_for(theme: OverlayTheme) -> Palette {
    match theme {
        OverlayTheme::Dark => Palette {
            background: rgba(36, 38, 43, 1.0),
            badge: rgba(170, 176, 186, 0.85),
            title: rgba(248, 249, 251, 1.0),
            subtitle: rgba(194, 199, 207, 0.95),
            key_fill: rgba(255, 255, 255, 0.06),
            key_border: rgba(255, 255, 255, 0.16),
            key_text: rgba(244, 246, 248, 1.0),
            hint: rgba(214, 220, 228, 1.0),
        },
        OverlayTheme::Light => Palette {
            background: rgba(248, 250, 252, 1.0),
            badge: rgba(116, 126, 141, 0.88),
            title: rgba(26, 31, 38, 1.0),
            subtitle: rgba(103, 112, 124, 0.96),
            key_fill: rgba(255, 255, 255, 0.95),
            key_border: rgba(94, 109, 130, 0.16),
            key_text: rgba(30, 35, 42, 1.0),
            hint: rgba(74, 87, 104, 1.0),
        },
    }
}

fn point(x: f32, y: f32) -> D2D_POINT_2F {
    D2D_POINT_2F { x, y }
}

fn rectf(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
