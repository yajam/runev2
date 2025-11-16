use std::sync::Arc;
use swash::font::{FontRef};

#[derive(Clone)]
pub struct RuneFont {
    pub bytes: Arc<Vec<u8>>,
    pub font: FontRef<'static>,
}

impl RuneFont {
    pub fn new(bytes: Arc<Vec<u8>>) -> Self {
        let static_slice: &'static [u8] =
            Box::leak(bytes.clone().into_boxed_slice());
        let font = FontRef::from_index(static_slice, 0).expect("Invalid font");
        Self { bytes, font }
    }
}
