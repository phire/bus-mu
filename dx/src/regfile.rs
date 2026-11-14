use dioxus::prelude::*;

use crate::ViewContext;

#[component]
pub fn Register(num: u32, show_text: bool) -> Element {
    let val = (num | num << 8) as u64;
    let val = val | val << 16 | val << 32 | val << 48;
    rsx! {
        div { class: "reg",
            if show_text {
                "{val:016X}"
            }
        }
    }
}

#[component]
pub fn RegisterFile() -> Element {
    let zoom = *use_context::<Signal<ViewContext>>().peek().zoom.read();
    let mix = ((zoom - 0.4) * 50.0 * (zoom - 0.4) * 50.0) as i32;
    let fading = mix >= 0 && mix < 100;
    let show_text = mix >= 0;

    rsx! {
        div {
            class: "horizontal-file",
            style: if fading { "color: color-mix(var(--bg-color) {mix}%, var(--text-color) {100 - mix}%);" } else { "color: var(--bg-color)" },
            div {
                class: "reg zero",
                style: if fading { "color: color-mix(var(--text-color) {mix}%, var(--bg-color) {100 - mix}%);" } else { "color: var(--text-color)" },
                if show_text {
                    "ZERO"
                }
            }
            for i in 1..32 {
                Register { num: i, show_text }
            }
        }
    }
}