//! The in-game console which allows changing cvars at runtime.
//!
//! LATER Split into a reusable crate: cvars-console-fyrox.

mod shared;

use fyrox::{
    dpi::PhysicalSize,
    engine::Engine,
    event::{ElementState, KeyboardInput, ScanCode},
    gui::{
        border::BorderBuilder,
        brush::Brush,
        formatted_text::WrapMode,
        message::{KeyCode, MessageDirection, UiMessage},
        stack_panel::StackPanelBuilder,
        text::{TextBuilder, TextMessage},
        text_box::{TextBoxBuilder, TextBoxMessage, TextCommitMode},
        widget::{WidgetBuilder, WidgetMessage},
        Orientation, UiNode,
    },
};

use shared::*;

use crate::{cvars::Cvars, prelude::*};

const UP_ARROW: ScanCode = 103;
const PG_UP: ScanCode = 104;
const DOWN_ARROW: ScanCode = 108;
const PG_DOWN: ScanCode = 109;

/// In-game console for the Fyrox game engine.
pub(crate) struct FyroxConsole {
    is_open: bool,
    was_mouse_grabbed: bool,
    console: Console,
    history: Handle<UiNode>,
    prompt_text_box: Handle<UiNode>,
    layout: Handle<UiNode>,
}

impl FyroxConsole {
    pub(crate) fn new(engine: &mut Engine) -> Self {
        let history = TextBuilder::new(WidgetBuilder::new())
            // Word wrap doesn't work if there's an extremely long word.
            .with_wrap(WrapMode::Letter)
            .build(&mut engine.user_interface.build_ctx());

        let prompt_arrow = TextBuilder::new(WidgetBuilder::new())
            .with_text("> ")
            .build(&mut engine.user_interface.build_ctx());

        let prompt_text_box = TextBoxBuilder::new(WidgetBuilder::new())
            .with_text_commit_mode(TextCommitMode::Immediate)
            .with_text("help")
            .build(&mut engine.user_interface.build_ctx());

        let prompt_line = StackPanelBuilder::new(
            WidgetBuilder::new().with_children([prompt_arrow, prompt_text_box]),
        )
        .with_orientation(Orientation::Horizontal)
        .build(&mut engine.user_interface.build_ctx());

        // StackPanel doesn't support colored background so we wrap it in a Border.
        let layout = BorderBuilder::new(
            WidgetBuilder::new()
                .with_visibility(false)
                .with_background(Brush::Solid(Color::BLACK.with_new_alpha(220)))
                .with_child(
                    StackPanelBuilder::new(
                        WidgetBuilder::new().with_children([history, prompt_line]),
                    )
                    .with_orientation(Orientation::Vertical)
                    .build(&mut engine.user_interface.build_ctx()),
                ),
        )
        .build(&mut engine.user_interface.build_ctx());

        // engine.user_interface.send_message(TextMessage::text(
        //     history,
        //     MessageDirection::ToWidget,
        //     "test".to_owned(),
        // ));

        FyroxConsole {
            is_open: false,
            was_mouse_grabbed: false,
            console: Console::new(),
            history,
            prompt_text_box,
            layout,
        }
    }

    pub(crate) fn resized(&mut self, engine: &mut Engine, size: PhysicalSize<u32>) {
        engine.user_interface.send_message(WidgetMessage::width(
            self.layout,
            MessageDirection::ToWidget,
            size.width as f32,
        ));

        engine.user_interface.send_message(WidgetMessage::height(
            self.layout,
            MessageDirection::ToWidget,
            (size.height / 2) as f32,
        ));
    }

    pub(crate) fn keyboard_input(
        &mut self,
        _cvars: &mut Cvars,
        _engine: &mut Engine,
        input: KeyboardInput,
    ) {
        // TODO engine needed?
        // LATER After fyrox can force focus to prompt, this should use the normal input system.
        if let ElementState::Pressed = input.state {
            match input.scancode {
                UP_ARROW => self.console.history_back(),
                DOWN_ARROW => self.console.history_forward(),
                PG_UP => self.console.history_scroll_up(10),
                PG_DOWN => self.console.history_scroll_down(10),

                _ => (),
            }
        }
    }

    pub(crate) fn ui_message(&mut self, engine: &mut Engine, cvars: &mut Cvars, msg: UiMessage) {
        // We could just listen for KeyboardInput and get the text from the prompt via
        // ```
        // let node = engine.user_interface.node(self.prompt_text_box);
        // let text = node.query_component::<TextBox>().unwrap().text();
        // ```
        // But this is the intended way to use the UI, even if it's more verbose.
        // At least it should reduce issues with the prompt reacting to some keys
        // but not others given KeyboardInput doesn't require focus.

        if let Some(TextBoxMessage::Text(text)) = msg.data() {
            self.update_prompt(text);
        }
        if let Some(WidgetMessage::KeyDown(KeyCode::Return | KeyCode::NumpadEnter)) = msg.data() {
            self.enter(engine, cvars);
        }
    }

    pub(crate) fn update_prompt(&mut self, text: &str) {
        dbg_logf!("update_prompt {}", text);
        self.console.prompt = text.to_owned();
    }

    pub(crate) fn enter(&mut self, engine: &mut Engine, cvars: &mut Cvars) {
        dbg!("ENTER", &self.console.prompt);
        self.console.process_input_text(cvars);

        let mut hist = String::new();
        // TODO history view index Option
        let hi = self.console.history_view_index;
        let lo = hi.saturating_sub(15); // TODO measure hight
        for line in &self.console.history[lo..hi] {
            hist.push_str(&line.text);
            hist.push('\n');
        }

        engine.user_interface.send_message(TextMessage::text(
            self.history,
            MessageDirection::ToWidget,
            hist,
        ));

        engine.user_interface.send_message(TextBoxMessage::text(
            self.prompt_text_box,
            MessageDirection::ToWidget,
            "new prompt".to_owned(), // TODO These spaces are so we can click on it
        ));
    }

    pub(crate) fn is_open(&self) -> bool {
        self.is_open
    }

    pub(crate) fn open(&mut self, engine: &mut Engine, was_mouse_grabbed: bool) {
        self.is_open = true;
        self.was_mouse_grabbed = was_mouse_grabbed;

        engine.user_interface.send_message(WidgetMessage::visibility(
            self.layout,
            MessageDirection::ToWidget,
            true,
        ));

        // TODO how to set focus?
    }

    /// Returns whether the mouse was grabbed before opening the console.
    ///
    /// It's #[must_use] so you don't forget to restore it.
    #[must_use]
    pub(crate) fn close(&mut self, engine: &mut Engine) -> bool {
        engine.user_interface.send_message(WidgetMessage::visibility(
            self.layout,
            MessageDirection::ToWidget,
            false,
        ));

        self.is_open = false;
        self.was_mouse_grabbed
    }
}

// TODO CvarAccess to cvars crate
impl CvarAccess for Cvars {
    fn get_string(&self, cvar_name: &str) -> Result<String, String> {
        self.get_string(cvar_name)
    }

    fn set_str(&mut self, cvar_name: &str, cvar_value: &str) -> Result<(), String> {
        self.set_str(cvar_name, cvar_value)
    }
}
