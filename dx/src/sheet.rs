use dioxus::prelude::*;

use crate::{Point};


#[derive(Clone)]
pub struct Sheet {
    scope_id: ScopeId,
    count: usize,
    map: std::collections::HashMap<String, Signal<ItemPos>>,
}
#[derive(Clone, Copy, Debug)]
pub struct ItemPos {
    pub position: Point,
    pub size: Point,
}

impl ItemPos {
    pub fn new(i: usize) -> Self {
        Self {
            position: Point { x: (i * 110) as f64, y: 0.0 },
            size: Point { x: 100.0, y: 100.0 },
        }
    }
}

impl Sheet {
    fn new(scope_id: ScopeId) -> Self {
        Self {
            scope_id,
            count: 0,
            map: std::collections::HashMap::new(),
        }
    }

    pub fn make_state(&mut self, id: Option<String>) -> Signal<ItemPos> {
        let idx = self.count;
        self.count += 1;
        let state = Signal::new_in_scope(ItemPos::new(idx), self.scope_id);
        if let Some(id) = id {
            self.map.insert(id, state.clone());
        }
        state
    }
}

#[component]
pub fn SheetComponent(
    children: Element,
    #[props(extends = GlobalAttributes)]
    attributes: Vec<Attribute>
) -> Element {
    use_context_provider(|| Signal::new( Sheet::new( dioxus::core::current_scope_id() )));

    rsx! {
        div { id: "sheet", ..attributes, {children} }
    }
}