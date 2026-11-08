

use dioxus::{core::AttributeValue, html::geometry::PagePoint, prelude::*};

use crate::{Point, Sheet, sheet};

static DRAG_ORIGIN: GlobalSignal<Option<PagePoint>> = GlobalSignal::new(|| None);

#[component]
pub fn Item(
    children: Element,
    #[props(extends = GlobalAttributes)]
    attributes: Vec<Attribute>
) -> Element {
    let mut sheet = use_context::<Signal<Sheet>>();

    let mut item_state = use_hook(|| {
        let id = attributes
          .iter()
          .find_map(|attr| match attr.value {
            AttributeValue::Text(ref text) if attr.name == "id" => Some(text.clone()),
            _ => None,
          });
        sheet.write().make_state(id)
    });
    let sheet::ItemPos { position: pos, size } = *item_state.read();

    rsx! {
        div {
            position: "absolute",
            top: "{pos.y}px",
            left: "{pos.x}px",
            border: "1px solid #ccc",
            width: "{size.x}px",
            height: "{size.y}px",
            "onpointerdown": "event.currentTarget.setPointerCapture(event.pointerId)",
            "onpointerup": "event.currentTarget.releasePointerCapture(event.pointerId)",
            onpointerdown: move |evt| {
                evt.prevent_default();
                let x = evt.coordinates().page().x - pos.x;
                let y = evt.coordinates().page().y - pos.y;
                *DRAG_ORIGIN.write() = Some(PagePoint::new(x, y));
            },
            onpointerup: move |evt| {
                evt.prevent_default();
                *DRAG_ORIGIN.write() = None;
            },
            onpointermove: move |evt| {
                evt.prevent_default();
                if let Some(origin) = *DRAG_ORIGIN.peek() {
                    let current = evt.coordinates().page();
                    let delta = current - origin;
                    // Handle dragging logic with delta
                    item_state.write().position.x = delta.x;
                    item_state.write().position.y = delta.y;
                }
            },
            ..attributes,
            {children}
        }
    }
}