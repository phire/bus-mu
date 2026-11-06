use std::any::Any;

use common::EmulationCore;
use dioxus::{document::eval, prelude::*};

mod sheet;
mod node;

pub use sheet::Sheet;

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

#[component]
fn App() -> Element {
    let zoom = use_signal(|| 1.0);

    rsx! {
        document::Stylesheet { href: CSS }
        ZoomController { zoom }
        div {
            class: "app",
            id: "app",
            transform: "scale({*zoom.read()})",
            ontouchmove: move |evt| {
                println!("Touch move event: {:?}", evt);
            },
            div { "Hello, World!" }
            div { "Zoom: {*zoom.read()}" }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Point {
    x: f32,
    y: f32,
}