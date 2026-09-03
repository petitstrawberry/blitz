use anyrender::{PaintScene as _, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::{DocumentConfig, util::Color};
use blitz_html::HtmlDocument;
use blitz_paint::paint_scene;
use blitz_traits::shell::{ColorScheme, Viewport};
use peniko::{Fill, kurbo::Rect};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

const HTML: &str = r#"
<!doctype html>
<html>
  <head>
    <style>
      html, body { height: 100%; margin: 0; }
      body {
        display: grid;
        place-items: center;
        background: #181825;
      }
      main {
        width: 520px;
        padding: 40px;
        border-radius: 24px;
        background: #313244;
        color: #cdd6f4;
      }
      h1 { color: #f38ba8; }
    </style>
  </head>
  <body>
    <main>
      <h1>Blitz on Scarlet</h1>
      <p>Native HTML and CSS rendering is alive.</p>
    </main>
  </body>
</html>
"#;

fn main() {
    let mut document = HtmlDocument::from_html(
        HTML,
        DocumentConfig {
            viewport: Some(Viewport::new(WIDTH, HEIGHT, 1.0, ColorScheme::Dark)),
            ..Default::default()
        },
    );

    document.as_mut().resolve(0.0);

    let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| {
            scene.fill(
                Fill::NonZero,
                Default::default(),
                Color::WHITE,
                Default::default(),
                &Rect::new(0.0, 0.0, f64::from(WIDTH), f64::from(HEIGHT)),
            );
            paint_scene(scene, document.as_mut(), 1.0, WIDTH, HEIGHT, 0, 0);
        },
        WIDTH,
        HEIGHT,
    );

    let checksum = buffer.iter().fold(0_u64, |sum, byte| {
        sum.wrapping_mul(16_777_619).wrapping_add(u64::from(*byte))
    });
    println!(
        "[blitz-browser] rendered {}x{} RGBA bytes={} checksum={checksum:016x}",
        WIDTH,
        HEIGHT,
        buffer.len()
    );
}
