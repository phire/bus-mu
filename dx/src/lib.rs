use std::{any::Any, sync::Arc};

use common::EmulationCore;
use dioxus::{CapturedError, desktop::Config, document::eval, html::{geometry::ClientPoint}, prelude::*};

mod sheet;
mod node;
mod regfile;

pub use sheet::Sheet;

use crate::{node::Item, node::Line, sheet::SheetComponent};

#[derive(Clone)]
struct LaunchConfig {
    core: Option<&'static dyn EmulationCore>,
    config: Option<Arc<dyn Any + Send + Sync>>,
}

pub fn run(core: Option<&'static dyn EmulationCore>, config: Arc<dyn Any + Send + Sync>, cores: Vec<&'static dyn EmulationCore>) -> Result<(), anyhow::Error> {
    dioxus::LaunchBuilder::new()
        .with_cfg(Config::new()
            .with_disable_context_menu(true))
        .with_context(LaunchConfig{ core, config: Some(config) })
        .with_context(cores)
        .launch(App);
    Ok(())
}

const CSS: Asset = asset!("assets/main.css");
const _: Asset = asset!("assets/Monaspace Argon Var.woff2", manganis::AssetOptions::builder().with_hash_suffix(false));

static HOOK_ZOOM_JS: &str = "
    let zoom_value = document.getElementById('_zoom_value');
    let zoom_start = document.getElementById('_zoom_start');
    let zoom_end = document.getElementById('_zoom_end');
    let body = document.body;
    body.addEventListener('gesturestart', function(event) {
      event.preventDefault();
      zoom_start.dispatchEvent(new Event('input', { bubbles: true }));
    });
    body.addEventListener('gesturechange', function(event) {
        event.preventDefault();
        zoom_value.value = event.scale;
        zoom_value.dispatchEvent(new Event('input', { bubbles: true }));
    });
    body.addEventListener('gestureend', function(event) {
        event.preventDefault();
        zoom_end.dispatchEvent(new Event('input', { bubbles: true }));
    });
";

#[component]
fn ZoomController(view: Signal<ViewContext>) -> Element {
    let mut zoom_initial = use_signal(|| 1.0);
    let mut zoom = view.read().zoom.clone();

    rsx! {
        div {
            // Hack to allow us to hook gesture events from safari
            display: "none",
            onmounted: move |_| async move {
                let _ = eval(HOOK_ZOOM_JS).await.unwrap();
            },
            input {
                r#type: "text",
                id: "_zoom_value",
                oninput: move |evt| {
                    let value = evt.value().parse::<f64>().unwrap_or(1.0);
                    let initial = *zoom_initial.read();
                    zoom.set(value * initial);
                },
            }
            input {
                r#type: "button",
                id: "_zoom_start",
                oninput: move |_| {
                    zoom_initial.set(*zoom.read());
                    view.write().touchpad = true;
                },
            }
            input { r#type: "button", id: "_zoom_end" }
        }

    }
}

#[derive(Clone, Copy, Debug)]
pub struct ViewContext {
    pub zoom: Signal<f64>,
    pub scroll: Signal<(f64, f64)>,
    drag_start: Option<(ClientPoint, (f64, f64))>,
    touchpad: bool,
}

impl ViewContext {
    fn new() -> Self {
        Self {
            zoom: Signal::new(0.33),
            scroll: Signal::new((0.0, 0.0)),
            drag_start: None,
            touchpad: false,
        }
    }

    fn mousedown(&mut self, evt: &dioxus::events::MouseEvent) {
        if evt.trigger_button()
            == Some(dioxus_elements::input_data::MouseButton::Secondary)
        {
            self.drag_start = Some((evt.data.coordinates().client(), *self.scroll.read()));
        }
    }

    fn mouseup(&mut self, evt: &dioxus::events::MouseEvent) {
        if evt.trigger_button()
            == Some(dioxus_elements::input_data::MouseButton::Secondary)
        {
            self.drag_start = None;
        }
    }

    fn mousemove(&mut self, evt: &dioxus::events::MouseEvent) {
        if let Some((start, scroll_at_start)) = self.drag_start {
            evt.prevent_default();
            evt.stop_propagation();
            let delta = (evt.data.coordinates().client() - start) / *self.zoom.peek();
            let (x, y) = scroll_at_start;
            let x = (x + delta.x).clamp(-800.0, 1600.0);
            let y = (y + delta.y).clamp(-600.0, 1200.0);
            self.scroll.set((x, y));
        }
    }

    fn wheel(&mut self, evt: &dioxus::events::WheelEvent) {
        let zoom = *self.zoom.peek();
        if evt.modifiers().ctrl() {
            let y = match evt.delta() {
                WheelDelta::Pixels(PixelsVector3D{ y, .. }) => y as f64,
                WheelDelta::Lines(LinesVector{ y, .. }) => y as f64 * 16.0,
                WheelDelta::Pages(PagesVector{ y, .. }) => y as f64 * 600.0,
            };
            let zoom = (zoom * (1.0 - y / 300.0)).clamp(0.33, 4.0);
            self.zoom.set(zoom);
        } else if !evt.modifiers().is_empty() {
            return;
        }

        let (x, y) = *self.scroll.peek();
        use dioxus_elements::geometry::*;
        let delta = match evt.delta() {
            WheelDelta::Pixels(PixelsVector3D{ x, y, .. }) => (x as f64, y as f64),
            WheelDelta::Lines(LinesVector{ x, y, .. }) => (x as f64 * 16.0, y as f64 * 16.0),
            WheelDelta::Pages(PagesVector{ x, y, .. }) => (x as f64 * 800.0, y as f64 * 600.0),
        };

        let x = (x - delta.0 / zoom).clamp(-800.0, 1600.0);
        let y = (y - delta.1 / zoom).clamp(-600.0, 1200.0);

        self.scroll.set((x, y));
    }
}

#[component]
fn App() -> Element {
    let mut view = use_signal(|| ViewContext::new());
    use_context_provider(|| view);
    let zoom = view.read().zoom.read().clone();
    let scroll = view.read().scroll.read().clone();

    rsx! {
        document::Stylesheet { href: CSS }
        ZoomController { view }
        div {
            id: "app",
            onwheel: move |evt| {
                view.write().wheel(&evt);
            },
            onmousemove: move |evt| {
                view.write().mousemove(&evt);
            },
            onmousedown: move |evt| view.write().mousedown(&evt),
            onmouseup: move |evt| view.write().mouseup(&evt),
            div { "Zoom: {zoom}" }

            ErrorBoundary {
                handle_error: |error: ErrorContext| {
                    if let Some(err) = error.error() {
                        rsx! {
                            div { "An error occurred: {err}" }
                        }
                    } else {
                        rsx! {
                            div { "An unknown error occurred." }
                        }
                    }
                },
                Emu { zoom, scroll }
            }
        }
    }
}


#[component]
pub fn Emu(zoom: f64, scroll: (f64, f64)) -> Element {
    let LaunchConfig { core, mut config } = use_context();
    // let cores = use_context::<Vec<&'static dyn common::EmulationCore>>();

    let mut status = use_signal(|| common::Status::Paused);

    let instance = use_hook(|| {
        let instance = if let Some(core) = core {
            core.new_threaded(config.take().unwrap())
        } else {
            Err(anyhow::anyhow!("No core specified"))
        };

        instance
            .map(|(instance, mut updates)| {
                let cloned = instance.clone();

                spawn(async move {
                    while let Ok(_) = updates.recv_update().await {
                        status.set(instance.borrow().status());
                    }
                });
                cloned
            })
            .map_err(|e| CapturedError(Arc::new(e)))
    })?;

    let i2 = instance.clone();
    let i3 = instance.clone();

    rsx! {
        if matches!(*status.read(), common::Status::Paused) {
            input {
                r#type: "button",
                value: "continue",
                onclick: move |_| {
                    println!("Starting instance...");
                    let _ = instance.borrow_mut().start();
                },
            }
            input { r#type: "button", value: "step" }
        } else {
            input {
                r#type: "button",
                value: "pause",
                onclick: move |_| {
                    let instance = i3.clone();
                    async move {
                        instance.borrow_mut().pause().await.unwrap();
                    }
                },
            }
        }
        SheetComponent { transform: "scale({zoom}) translate({scroll.0}px, {scroll.1}px)",

            // Line { from: "item1", to: "item2" }
            // Item { id: "item1", "Item Content" }
            // Item { id: "item2", "Another Item" }
            // regfile::RegisterFile {}
            {i2.borrow_mut().ui(core.unwrap())}
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Point {
    x: f64,
    y: f64,
}