use dioxus::prelude::*;

use crate::ViewContext;

#[component]
pub fn Register(num: u32, zoom: f64) -> Element {
    rsx! {
        div { class: "reg",
            if zoom > 0.4 {
                span { "{0:016}" }
            }
        }
    }
}

#[component]
pub fn RegisterFile() -> Element {
    let zoom = *use_context::<ViewContext>().zoom.read();
    let mix = ((zoom - 0.4) * 50.0 * (zoom - 0.4) * 50.0) as i32;

    rsx! {
        div {
            class: "horizontal-file",
            "hide-text": zoom < 0.4,
            style: if zoom > 0.4 && zoom < 0.6 { "color: color-mix(var(--bg-color) {mix}%, var(--text-color) {100 - mix}%);" } else { "color: var(--bg-color)" },
            div {
                class: "reg zero",
                style: if zoom > 0.4 && zoom < 0.6 { "color: color-mix(var(--text-color) {mix}%, var(--bg-color) {100 - mix}%);" } else { "color: var(--text-color)" },
                span { "ZERO" }
            }
            for i in 1..32 {
                Register { num: i, zoom }
            }
        }
    }
}