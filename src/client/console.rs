//! The in-game console which allows changing cvars at runtime.
//!
//! LATER Split into a reusable crate: cvars-console-fyrox.

mod shared;

use fyrox::{
    dpi::PhysicalSize,
    engine::Engine,
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

/// In-game console for the Fyrox game engine.
pub(crate) struct FyroxConsole {
    is_open: bool,
    first_open: bool,
    was_mouse_grabbed: bool,
    console: Console,
    height: u32,
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

        FyroxConsole {
            is_open: false,
            first_open: true,
            was_mouse_grabbed: false,
            console: Console::new(),
            height: 0,
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

        self.height = size.height / 2;
        engine.user_interface.send_message(WidgetMessage::height(
            self.layout,
            MessageDirection::ToWidget,
            self.height as f32,
        ));

        // This actually goes beyond the screen but who cares.
        // It, however, still won't let me put the cursor at the end by clicking after the text:
        // https://github.com/FyroxEngine/Fyrox/issues/361
        engine.user_interface.send_message(WidgetMessage::width(
            self.prompt_text_box,
            MessageDirection::ToWidget,
            size.width as f32,
        ));

        // The number of lines that can fit might have changed - reprint history.
        self.update_ui_history(engine);
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
        //
        // Note that it might still be better to read the text from the UI as the souce of truth
        // because right now the console doesn't know about any text we set from code.

        if let Some(TextBoxMessage::Text(text)) = msg.data() {
            self.console.prompt = text.to_owned();
        }

        match msg.data() {
            Some(WidgetMessage::Unfocus) => {
                // As long as the console is open, always keep the prompt focused
                if self.is_open {
                    dbg!(msg);
                    engine.user_interface.send_message(WidgetMessage::focus(
                        self.prompt_text_box,
                        MessageDirection::ToWidget,
                    ));
                }
            }
            Some(WidgetMessage::KeyDown(KeyCode::Up)) => {
                self.console.history_back();
                self.update_ui_prompt(engine);
            }
            Some(WidgetMessage::KeyDown(KeyCode::Down)) => {
                self.console.history_forward();
                self.update_ui_prompt(engine);
            }
            Some(WidgetMessage::KeyDown(KeyCode::PageUp)) => {
                self.console.history_scroll_up(10);
                self.update_ui_history(engine);
            }
            Some(WidgetMessage::KeyDown(KeyCode::PageDown)) => {
                self.console.history_scroll_down(10);
                self.update_ui_history(engine);
            }
            Some(WidgetMessage::KeyDown(KeyCode::Return | KeyCode::NumpadEnter)) => {
                self.console.enter(cvars);
                self.update_ui_prompt(engine);
                self.update_ui_history(engine);
            }
            _ => (),
        }
    }

    fn update_ui_prompt(&mut self, engine: &mut Engine) {
        dbg!(&self.console.prompt);
        engine.user_interface.send_message(TextBoxMessage::text(
            self.prompt_text_box,
            MessageDirection::ToWidget,
            self.console.prompt.clone(),
        ));
    }

    fn update_ui_history(&mut self, engine: &mut Engine) {
        // LATER There should be a cleaner way to measure lines
        let line_height = 14;
        // Leave 1 line room for the prompt
        // LATER This is not exact for tiny windows but good enough for now.
        let max_lines = (self.height / line_height).saturating_sub(1);

        let hi = self.console.history_view_end;
        let lo = hi.saturating_sub(max_lines.try_into().unwrap());

        let mut hist = String::new();
        for line in &self.console.history[lo..hi] {
            if line.is_input {
                hist.push_str("> ");
            }
            hist.push_str(&line.text);
            hist.push('\n');
        }

        engine.user_interface.send_message(TextMessage::text(
            self.history,
            MessageDirection::ToWidget,
            hist,
        ));
    }

    pub(crate) fn is_open(&self) -> bool {
        self.is_open
    }

    /// Open the console.
    ///
    /// If your game grabs the mouse, you can save the previous state here
    /// and get it back when closing.
    pub(crate) fn open(&mut self, engine: &mut Engine, was_mouse_grabbed: bool) {
        self.is_open = true;
        self.was_mouse_grabbed = was_mouse_grabbed;

        engine.user_interface.send_message(WidgetMessage::visibility(
            self.layout,
            MessageDirection::ToWidget,
            true,
        ));

        engine
            .user_interface
            .send_message(WidgetMessage::focus(self.prompt_text_box, MessageDirection::ToWidget));

        if self.first_open {
            // Currently it's not necessary to track the first opening,
            // the history will be empty so we could just print it when creating the console.
            // Eventually though, all stdout will be printed in the console
            // so if the message was at the top, nobody would see it.
            self.first_open = false;
            self.console.print("Type 'help' or '?' for basic info");
            self.console.print("Known issue: click the console to focus it");
            self.update_ui_history(engine);
        }
    }

    /// Close the console. Returns whether the mouse was grabbed before opening the console.
    ///
    /// It's #[must_use] so you don't accidentally forget to restore it.
    /// You can safely ignore it intentionally.
    #[must_use]
    pub(crate) fn close(&mut self, engine: &mut Engine) -> bool {
        engine.user_interface.send_message(WidgetMessage::visibility(
            self.layout,
            MessageDirection::ToWidget,
            false,
        ));
        engine
            .user_interface
            .send_message(WidgetMessage::unfocus(self.prompt_text_box, MessageDirection::ToWidget));

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
