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
    pub valid: bool,
    pub position: Point,
    pub size: Point,
}

impl ItemPos {
    pub fn new(i: usize) -> Self {
        Self {
            valid: true,
            position: Point { x: (i * 110) as f64, y: (i * 110) as f64 },
            size: Point { x: 100.0, y: 100.0 },
        }
    }
}

impl Default for ItemPos {
    fn default() -> Self {
        Self {
            valid: false,
            position: Point { x: 0.0, y: 0.0 },
            size: Point { x: 0.0, y: 0.0 },
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
        let state = ItemPos::new(idx);
        if let Some(id) = id {
            // If we have an id, allocate in sheet's scope and put it in the map
            let mut state_sig = self.get_state_entry(id.clone());
            *state_sig.write() = state;
            state_sig

        } else {
            // Otherwise, just return a local signal
            Signal::new(state)
        }
    }

    /// Get item state for an id. Creates a placeholder if it doesn't exist yet.
    fn get_state_entry(&mut self, id: String) -> Signal<ItemPos> {
        self.map
            .entry(id)
            .or_insert_with(|| {
                // Insert with parents scope, so it's siblings can access it
                Signal::new_in_scope(ItemPos::default(), self.scope_id)
            })
            .clone()
    }

    pub fn read_state(&mut self, id: String) -> Option<ItemPos> {
        let sig = self.get_state_entry(id);
        let state = sig.read();
        if state.valid {
            Some(*state)
        } else {
            None
        }
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