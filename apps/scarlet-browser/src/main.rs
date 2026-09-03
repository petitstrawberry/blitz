use anyrender::{PaintScene as _, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::{DocumentConfig, build_single_font_ctx, util::Color};
use blitz_html::HtmlDocument;
use blitz_paint::paint_scene;
use blitz_traits::shell::{ColorScheme, Viewport};
use peniko::{Fill, kurbo::Rect};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;
const FONT: &[u8] = include_bytes!("../../../examples/seven_guis/assets/DejaVuSans.woff2");

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

fn render(width: u32, height: u32) -> Vec<u8> {
    let mut document = HtmlDocument::from_html(
        HTML,
        DocumentConfig {
            viewport: Some(Viewport::new(width, height, 1.0, ColorScheme::Dark)),
            font_ctx: Some(build_single_font_ctx(FONT)),
            ..Default::default()
        },
    );

    document.as_mut().resolve(0.0);

    render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| {
            scene.fill(
                Fill::NonZero,
                Default::default(),
                Color::WHITE,
                Default::default(),
                &Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
            );
            paint_scene(scene, document.as_mut(), 1.0, width, height, 0, 0);
        },
        width,
        height,
    )
}

fn checksum(buffer: &[u8]) -> u64 {
    buffer.iter().fold(0_u64, |sum, byte| {
        sum.wrapping_mul(16_777_619).wrapping_add(u64::from(*byte))
    })
}

#[cfg(not(target_os = "scarlet"))]
fn main() {
    let buffer = render(WIDTH, HEIGHT);
    let checksum = checksum(&buffer);
    println!(
        "[blitz-browser] rendered {}x{} RGBA bytes={} checksum={checksum:016x}",
        WIDTH,
        HEIGHT,
        buffer.len()
    );
}

#[cfg(target_os = "scarlet")]
fn main() -> std::process::ExitCode {
    match scarlet::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[blitz-browser] error: {error:?}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(target_os = "scarlet")]
mod scarlet {
    use std::thread;
    use std::time::Duration;

    use sws_client::{Connection, Error, Event, SurfaceBuilder};

    use super::{HEIGHT, WIDTH, checksum, render};

    const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(8);

    pub(super) fn run() -> Result<(), Error> {
        let connection = Connection::connect_default()?;
        let surface_id = SurfaceBuilder::new()
            .app_id("org.scarlet-os.blitz-browser")
            .app_name("Blitz Browser")
            .size(WIDTH, HEIGHT)
            .build(&connection)?;
        let events = connection.subscribe_window_events(surface_id);

        redraw(&connection, surface_id)?;

        loop {
            connection.dispatch()?;
            while let Some(event) = events.poll_event() {
                match event {
                    Event::SurfaceConfigure {
                        surface_id: configured_id,
                        width,
                        height,
                    } if configured_id == surface_id => {
                        connection.resize_window(surface_id, width.max(1), height.max(1))?;
                        redraw(&connection, surface_id)?;
                    }
                    Event::SurfaceDestroyed {
                        surface_id: destroyed_id,
                    } if destroyed_id == surface_id => return Ok(()),
                    _ => {}
                }
            }

            thread::sleep(EVENT_POLL_INTERVAL);
        }
    }

    fn redraw(connection: &Connection, surface_id: u32) -> Result<(), Error> {
        let (width, height) = connection
            .with_surface(surface_id, |surface| (surface.width(), surface.height()))
            .ok_or(Error::SurfaceNotFound)?;
        let rgba = render(width, height);

        connection
            .with_surface_mut(surface_id, |surface| {
                for (destination, source) in surface
                    .buffer_mut()
                    .chunks_exact_mut(4)
                    .zip(rgba.chunks_exact(4))
                {
                    destination[0] = source[2];
                    destination[1] = source[1];
                    destination[2] = source[0];
                    destination[3] = source[3];
                }
            })
            .ok_or(Error::SurfaceNotFound)?;
        connection.commit(surface_id)?;

        let checksum = checksum(&rgba);
        println!(
            "[blitz-browser] presented {width}x{height} RGBA bytes={} checksum={checksum:016x}",
            rgba.len()
        );
        Ok(())
    }
}
