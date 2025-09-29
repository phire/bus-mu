use std::any::Any;

use common::EmulationCore;
use dioxus::prelude::*;

pub fn run(core: Option<&'static dyn EmulationCore>, config: Box<dyn Any>, cores: Vec<&'static dyn EmulationCore>) -> Result<(), anyhow::Error> {
    dioxus::launch(App);
    Ok(())
}

const CSS: Asset = asset!("assets/main.css");

#[component]
fn App() -> Element {
    rsx! {
        document::Stylesheet { href: CSS }
        div { "Hello, World!" }
    }
}