// Patched dependencies are outside the workspace, so compile the production
// border module here to run its regression tests with `cargo test -p needle`.
use gpui_component::ActiveTheme;

#[allow(dead_code)]
#[path = "../../../third-party/patched/gpui-component-0.5.1/src/window_border.rs"]
mod window_border;
