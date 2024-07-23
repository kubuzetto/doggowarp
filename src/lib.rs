use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Sub};
use std::time::{Duration, Instant};
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use std::vec::Vec;
use winit::application::ApplicationHandler;
use winit::error::EventLoopError;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

// region Elapsed

pub struct Elapsed {
    last: Instant,
}

impl Elapsed {
    pub fn new() -> Self { Self { last: Instant::now() } }

    pub fn elapsed(&mut self) -> Duration {
        let now = Instant::now();
        let d = now.duration_since(self.last);
        self.last = now;
        d
    }
}

impl Default for Elapsed {
    fn default() -> Self { Self::new() }
}

// endregion
// region Fps

pub struct Fps {
    count: usize,
    last: Instant,
}

impl Fps {
    pub fn new() -> Self { Self { count: 0, last: Instant::now() } }

    pub fn tick(&mut self) -> Option<usize> {
        const SECOND: Duration = Duration::from_secs(1);
        let now = Instant::now();
        let d = now.duration_since(self.last);
        if d < SECOND {
            self.count += 1;
            None
        } else {
            let c = self.count;
            self.last = now;
            self.count = 0;
            Some(c)
        }
    }
}

impl Default for Fps {
    fn default() -> Self { Self::new() }
}

// endregion
// region Smooth

pub struct Smooth<T> {
    t: T,
}

impl<W, T> Smooth<T>
where
    T: Copy,
    T: Mul<f64, Output=W>,
    W: Add<W, Output=T>,
{
    pub fn new(t: T) -> Self { Self { t } }

    pub fn update(&mut self, t: T) -> T {
        let t = self.t * 0.6 + t * 0.4;
        self.t = t;
        t
    }
}

impl<W, T> Default for Smooth<T>
where
    T: Default + Copy,
    T: Mul<f64, Output=W>,
    W: Add<W, Output=T>,
{
    fn default() -> Self { Self::new(T::default()) }
}

// endregion
// region Image

pub struct Image {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl Image {
    pub fn new(width: usize, height: usize, data: Vec<u8>) -> Self {
        Self { width, height, data }
    }
    pub fn from_jpeg(jpeg_data: &[u8]) -> Result<Self, DecodeErrors> {
        let options = DecoderOptions::default().
            jpeg_set_out_colorspace(ColorSpace::RGBA);
        let mut decoder = JpegDecoder::new_with_options(jpeg_data, options);
        let data = decoder.decode()?;
        let (width, height) = decoder.dimensions().
            ok_or_else(|| DecodeErrors::Format(String::from("cannot get dimensions")))?;
        Ok(Self::new(width, height, data))
    }

    pub fn sample(&self, pos: Pos) -> Sampler {
        let x = pos.x.trunc() as usize;
        let x = x.clamp(0, self.width - 1);
        let y = pos.y.trunc() as usize;
        let y = y.clamp(0, self.height - 1);
        Sampler { data: &self.data, idx: 4 * (x + self.width * y) }
    }
}

pub struct Sampler<'a> {
    data: &'a [u8],
    idx: usize,
}

impl<'a> Sampler<'a> {
    pub fn red(self) -> f64 { self.data[self.idx] as f64 }
    pub fn green(self) -> f64 { self.data[self.idx + 1] as f64 }
    pub fn blue(self) -> f64 { self.data[self.idx + 2] as f64 }
}

// endregion
// region Pos

#[derive(Copy, Clone, Default)]
pub struct Pos {
    x: f64,
    y: f64,
}

impl Pos {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn len(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn dist(&self, r: Self) -> f64 {
        self.sub(r).len()
    }
}

impl Add for Pos {
    type Output = Pos;
    fn add(self, r: Pos) -> Pos {
        Pos { x: self.x + r.x, y: self.y + r.y }
    }
}

impl Sub for Pos {
    type Output = Pos;
    fn sub(self, r: Pos) -> Pos {
        Pos { x: self.x - r.x, y: self.y - r.y }
    }
}

impl Mul<f64> for Pos {
    type Output = Pos;
    fn mul(self, f: f64) -> Pos {
        Pos { x: self.x * f, y: self.y * f }
    }
}

impl Div<f64> for Pos {
    type Output = Pos;
    fn div(self, f: f64) -> Pos {
        Pos { x: self.x / f, y: self.y / f }
    }
}

// endregion
// region App

pub trait AppState: Sized {
    type StartProps;
    type StartErr: Debug;
    fn start(event_loop: &ActiveEventLoop, props: Self::StartProps) -> Result<Self, Self::StartErr>;
    type MouseMoveErr: Debug;
    fn mousemove(&mut self, pos: Pos) -> Result<(), Self::MouseMoveErr>;
    type RenderErr: Debug;
    fn render(&mut self, delta: Duration) -> Result<(), Self::RenderErr>;
    fn window(&self) -> &Window;
}

pub struct Driver<State: AppState> {
    props: Option<State::StartProps>,
    state: Option<State>,
    elapsed: Elapsed,
}

impl<State: AppState> Driver<State> {
    pub fn new(props: State::StartProps) -> Self {
        Self {
            props: Some(props),
            state: None,
            elapsed: Elapsed::new(),
        }
    }

    pub fn run(&mut self) -> Result<(), EventLoopError> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Wait);
        event_loop.run_app(self)
    }
}

impl<State: AppState> ApplicationHandler for Driver<State> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let state = State::start(event_loop, self.props.take().unwrap()).unwrap();
            self.state = Some(state);
            self.elapsed.elapsed();
            self.state.as_ref().unwrap().window().request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => {
                let delta = self.elapsed.elapsed();
                let state = self.state.as_mut().unwrap();
                state.render(delta).unwrap();
                state.window().request_redraw();
            }
            WindowEvent::CursorMoved { device_id: _, position: pos } => {
                let state = self.state.as_mut().unwrap();
                let p = pos.to_logical(state.window().scale_factor());
                state.mousemove(Pos::new(p.x, p.y)).unwrap();
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => (),
        }
    }
}

// endregion
// region Color

#[derive(Copy, Clone, Default)]
pub struct Color {
    red: f64,
    green: f64,
    blue: f64,
}

impl Color {
    pub fn new(red: f64, green: f64, blue: f64) -> Self {
        Self { red, green, blue }
    }

    pub fn write_bytes(self, p: &mut [u8]) {
        p[0] = self.red.floor() as u8;
        p[1] = self.green.floor() as u8;
        p[2] = self.blue.floor() as u8;
    }
}

impl Add for Color {
    type Output = Self;
    fn add(self, r: Self) -> Self {
        Self {
            red: self.red + r.red,
            green: self.green + r.green,
            blue: self.blue + r.blue,
        }
    }
}

impl Mul<f64> for Color {
    type Output = Self;
    fn mul(self, f: f64) -> Self {
        Self {
            red: self.red * f,
            green: self.green * f,
            blue: self.blue * f,
        }
    }
}

// endregion