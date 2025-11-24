//! Basic UI elements rendered via rune-surface Canvas.
//! These helpers focus on visuals only; input/state is up to the caller.

pub mod alert;
pub mod button;
pub mod caret;
pub mod caret_renderer;
pub mod checkbox;
pub mod confirm_dialog;
pub mod date_picker;
pub mod file_input;
pub mod image;
pub mod input_box;
pub mod label;
pub mod link;
pub mod modal;
pub mod multiline_text;
pub mod radio;
pub mod select;
pub mod selection_renderer;
pub mod table;
pub mod text;
pub mod text_area;
pub mod webview;

pub use alert::{Alert, AlertPosition};
pub use button::{Button, ButtonClickResult};
pub use checkbox::{Checkbox, CheckboxClickResult};
pub use confirm_dialog::{ConfirmClickResult, ConfirmDialog};
pub use date_picker::{DatePicker, DatePickerKey, DatePickerKeyResult};
pub use file_input::{FileInput, FileInputClickResult};
pub use image::{ImageBox, ImageFit};
pub use input_box::InputBox;
pub use label::Label;
pub use link::Link;
pub use modal::{Modal, ModalButton, ModalClickResult};
pub use multiline_text::MultilineText;
pub use radio::{Radio, RadioClickResult};
pub use select::Select;
pub use table::{Alignment, Column, Table, TableCell, TableRow};
pub use text::Text;
pub use text_area::TextArea;
#[cfg(feature = "webview-cef")]
pub use webview::WebView;
// Export webview layout and native CEF view functions for FFI use
pub use webview::{
    clear_webview_rect, get_native_cef_view, get_native_cef_view_rect, get_webview_rect,
    has_native_cef_view, position_native_cef_view, set_native_cef_view,
    set_native_cef_view_position_callback, set_webview_rect,
};
