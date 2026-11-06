

use dioxus::prelude::*;

use crate::{Point, Sheet, sheet};

pub struct NodeProps {
    pub position: Point,

    pub children: Element,
}

#[derive(Clone, Copy)]
pub struct ItemState {
    pub pos: sheet::ItemPos,
}

#[component]
pub fn Item(children: Element) -> Element {
    let mut state = use_hook(|| ItemState {
        pos: sheet::ItemPos {
            position: Point { x: 0.0, y: 0.0 },
            size: Point { x: 100.0, y: 100.0 },
        },
    });

    rsx! {
        div {
            position: "absolute",
            onmousedown: move |evt| {
                let mut sheet: Sheet = consume_context();
                sheet.start_drag(evt, &mut state.pos);
            },

            {children}
        }
    }
}