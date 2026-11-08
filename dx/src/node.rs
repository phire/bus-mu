

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

#[component]
pub fn Line(
    from: String,
    to: String,
    #[props(extends = GlobalAttributes)]
    attributes: Vec<Attribute>
) -> Element {
    let sheet = use_context::<Signal<Sheet>>();
    let to = sheet.read().get_state(&to).map(|s| s.read().position);
    let from = sheet.read().get_state(&from).map(|s| s.read().position);

    rsx! {
        if let (Some(from), Some(to)) = (from, to) {
            {
                let abs_width = (to.x - from.x).abs();
                let abs_height = (to.y - from.y).abs();
                let (start_x, end_x) = match to.x > from.x {
                    true => (abs_width, 0.0),
                    false => (0.0, abs_width),
                };
                let (start_y, end_y) = match to.y > from.y {
                    true => (abs_height, 0.0),
                    false => (0.0, abs_height),
                };
                rsx! {
                    div {
                        position: "absolute",
                        top: "{std::cmp::min(from.y as i64, to.y as i64)}px",
                        left: "{std::cmp::min(from.x as i64, to.x as i64)}px",
                        z_index: "-100",
                        ..attributes,

                        svg { width: "{abs_width + 30.0}px", height: "{abs_height + 30.0}px",

                            line {
                                x1: "{start_x}",
                                y1: "{start_y}",
                                x2: "{end_x}",
                                y2: "{end_y}",
                                stroke: "#ccc",
                                stroke_width: "2",
                            }
                        }
                    }
                }
            }
        }
    }
}