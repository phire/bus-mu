use crate::Point;


#[derive(Clone)]
pub struct Sheet {

}

#[derive(Clone, Copy, Debug)]
pub struct ItemPos {
    pub position: Point,
    pub size: Point,
}

impl Sheet {
    pub fn start_drag(&mut self, evt: dioxus::prelude::MouseEvent, state: &mut ItemPos) {
        // TODO
    }
}