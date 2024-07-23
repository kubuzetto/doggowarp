#![windows_subsystem = "windows"]

use std::time::Duration;
use pixels::{Pixels, SurfaceTexture};
use rayon::prelude::*;
use winit::dpi::{LogicalSize, Size};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;
use anyhow::{Error, Result};
use doggowarp::*;

const DOGGO: &[u8] = include_bytes!("../../doggo.jpg");

fn main() -> Result<()> {
    Ok(<Driver<Warp>>::new(Image::from_jpeg(DOGGO)?).run()?)
}

#[inline(always)]
fn shader(a: &Image, p: Pos, l: Pos, v: Pos) -> Color {
    let m = 1.0 - l.dist(p) / 190.0;
    let m = m.clamp(0.0, 1.0);
    let m = v * m * m * -1.5;

    let mut c = Color::default();
    for j in 0..10 {
        let s = j as f64 * 0.005;
        c = c + Color::new(
            a.sample(p + m * (s + 0.175)).red(),
            a.sample(p + m * (s + 0.200)).green(),
            a.sample(p + m * (s + 0.225)).blue(),
        );
    }
    c * 0.1
}

struct Warp {
    window: Window,
    pixels: Pixels,
    img: Image,
    cursor: Pos,
    last: Pos,
    velocity: Smooth<Pos>,
    fps: Fps,
}

impl Warp {
    fn update(&mut self, delta: Duration) -> (Pos, Pos) {
        let location = self.cursor;
        let velocity = (location - self.last) * 0.2 / delta.as_secs_f64();
        let velocity = self.velocity.update(velocity);
        self.last = location;
        (location, velocity)
    }
}

impl AppState for Warp {
    type StartProps = Image;

    type StartErr = Error;

    fn start(event_loop: &ActiveEventLoop, img: Image) -> Result<Self> {
        let size = Size::Logical(LogicalSize::new(img.width as f64, img.height as f64));
        let window = event_loop.create_window(Window::default_attributes()
            .with_title("doggowarp").with_inner_size(size).with_resizable(false))?;
        let ws = window.inner_size();
        let tx = SurfaceTexture::new(ws.width, ws.height, &window);
        let mut pixels = Pixels::new(img.width as u32, img.height as u32, tx).unwrap();
        // write alpha channel as opaque, it never changes
        pixels.frame_mut().iter_mut().skip(3).step_by(4).for_each(|e| *e = 255);
        Ok(Self {
            pixels,
            window,
            img,
            cursor: Pos::default(),
            last: Pos::default(),
            velocity: Smooth::default(),
            fps: Fps::default(),
        })
    }

    type MouseMoveErr = Error;

    fn mousemove(&mut self, pos: Pos) -> Result<()> {
        self.cursor = pos;
        Ok(())
    }
    type RenderErr = Error;
    fn render(&mut self, delta: Duration) -> Result<()> {
        let (location, velocity) = self.update(delta);
        let width = self.img.width;
        self.pixels.frame_mut()
            .par_chunks_exact_mut(4)
            .enumerate()
            .for_each(|(idx, pixel_bytes)| {
                let pixel = Pos::new((idx % width) as f64, (idx / width) as f64);
                shader(&self.img, pixel, location, velocity).write_bytes(pixel_bytes);
            });
        self.pixels.render()?;
        if let Some(fps) = self.fps.tick() {
            self.window.set_title(&format!("doggowarp | {} fps", fps));
        }
        Ok(())
    }
    fn window(&self) -> &Window { &self.window }
}
