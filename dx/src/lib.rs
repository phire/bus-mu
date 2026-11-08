use std::any::Any;

use common::EmulationCore;
use dioxus::{document::eval, prelude::*};

mod sheet;
mod node;

pub use sheet::Sheet;

use crate::{node::Item, sheet::SheetComponent};

pub fn run(core: Option<&'static dyn EmulationCore>, config: Box<dyn Any>, cores: Vec<&'static dyn EmulationCore>) -> Result<(), anyhow::Error> {
    dioxus::launch(App);
    Ok(())
}

const CSS: Asset = asset!("assets/main.css");

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
fn ZoomController(zoom: Signal<f64>) -> Element {
    let mut zoom_initial = use_signal(|| 1.0);
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
                },
            }
            input { r#type: "button", id: "_zoom_end" }
        }

    }
}

fn apply_scroll(mut scroll: Signal<(f64, f64)>, evt: &dioxus::events::WheelEvent) {
    if !evt.modifiers().is_empty() {
        return;
    }

    let (x, y) = *scroll.read();
    use dioxus_elements::geometry::*;
    let delta = match evt.delta() {
        WheelDelta::Pixels(PixelsVector3D{ x, y, .. }) => (x as f64, y as f64),
        WheelDelta::Lines(LinesVector{ x, y, .. }) => (x as f64 * 16.0, y as f64 * 16.0),
        WheelDelta::Pages(PagesVector{ x, y, .. }) => (x as f64 * 800.0, y as f64 * 600.0),
    };

    let x = (x + delta.0).clamp(-800.0, 1600.0);
    let y = (y + delta.1).clamp(-600.0, 1200.0);

    scroll.set((x, y));
}

#[component]
fn App() -> Element {
    let zoom = use_signal(|| 1.0);
    let scroll = use_signal(|| (0.0, 0.0));

    rsx! {
        document::Stylesheet { href: CSS }
        ZoomController { zoom }
        div {
            id: "app",
            onwheel: move |evt| {
                apply_scroll(scroll, &evt);
            },
            div { "Zoom: {*zoom.read()}" }

            SheetComponent { transform: "scale({*zoom.read()}) translate({-scroll.read().0}px, {-scroll.read().1}px)",
                Item { id: "item1", "Item Content" }
                Item { id: "item2", "Another Item" }
                Item { id: "item3", "a third item" }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Point {
    x: f64,
    y: f64,
}