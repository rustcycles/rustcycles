#![allow(unreachable_pub)] // TODO

/// Engine-independant parts of the in-game console.
///
/// Parsing and executing commands, history, eventually tab completion, ...
#[derive(Debug, Clone, Default)]
pub struct Console {
    pub prompt: String,
    prompt_saved: String,

    /// Where we are in history when using up and down keys.
    prompt_history_index: usize,

    pub history: Vec<HistoryLine>,

    /// Where we are in the history view when scrolling using page up and down keys.
    ///
    /// It's the index of the *last* line that is to be displayed at the *bottom*.
    pub history_view_index: usize,
}

impl Console {
    pub fn new() -> Self {
        #[allow(unused_mut)]
        let mut con = Console {
            prompt: String::new(),
            prompt_saved: String::new(),
            prompt_history_index: 0,
            history: Vec::new(),
            history_view_index: 0,
        };
        //con.print("Type 'help' for basic info".to_owned()); TODO
        con
    }

    /// Go back in command history.
    pub fn history_back(&mut self) {
        // Save the prompt so that users can go back in history,
        // then come back to present and get what they typed back.
        if self.prompt_history_index == self.history.len() {
            self.prompt_saved = self.prompt.clone();
        }

        let search_slice = &self.history[0..self.prompt_history_index];
        if let Some(new_index) = search_slice.iter().rposition(|hist_line| hist_line.is_input) {
            self.prompt_history_index = new_index;
            self.prompt = self.history[self.prompt_history_index].text.clone();
        }
    }

    /// Go forward in command history.
    pub fn history_forward(&mut self) {
        // Since we're starting the search at history_index+1, the condition must remain here
        // otherwise the range could start at history.len()+1 and panic.
        if self.prompt_history_index >= self.history.len() {
            return;
        }

        let search_slice = &self.history[self.prompt_history_index + 1..];
        if let Some(new_index) = search_slice.iter().position(|hist_line| hist_line.is_input) {
            // `position` starts counting from the iterator's start,
            // not from history's start so we add the found index to what we skipped
            // instead of using it directly.
            self.prompt_history_index += new_index + 1;
            self.prompt = self.history[self.prompt_history_index].text.clone();
        } else {
            // We're at the end of history, restore the saved prompt.
            self.prompt_history_index = self.history.len();
            self.prompt = self.prompt_saved.clone();
        }
    }

    pub fn history_scroll_up(&mut self, count: usize) {
        self.history_view_index = self.history_view_index.saturating_sub(count);
        if self.history_view_index == 0 && !self.history.is_empty() {
            // Keep at least one line in history when possible
            // because scrolling up to an empty view looks weird.
            self.history_view_index = 1;
        }
    }

    pub fn history_scroll_down(&mut self, count: usize) {
        self.history_view_index = (self.history_view_index + count).min(self.history.len());
    }

    /// The user pressed enter - process the line of text
    pub fn enter(&mut self, cvars: &mut impl CvarAccess) {
        let hist_len_old = self.history.len();

        self.push_history_line(self.prompt.clone(), true);

        // The actual command parsing logic
        let res = self.process_line(cvars);
        if let Err(msg) = res {
            self.push_history_line(msg, false);
        }

        self.prompt = String::new();

        // Entering a new command resets the user's position in history to the end.
        self.prompt_history_index = self.history.len();

        // If the view was at the end, keep scrolling down as new lines are added.
        // Otherwise the view's position shouldn't change.
        if self.history_view_index == hist_len_old {
            self.history_view_index = self.history.len();
        }
    }

    /// Parse what the user typed and get or set a cvar
    fn process_line(&mut self, cvars: &mut impl CvarAccess) -> Result<(), String> {
        // Splitting on whitespace also in effect trims leading and trailing whitespace.
        let mut parts = self.prompt.split_whitespace();

        let cvar_name = match parts.next() {
            Some(name) => name,
            None => return Ok(()),
        };
        if cvar_name == "help" || cvar_name == "?" {
            self.print("Available actions:".to_owned());
            self.print("    help                 Print this message".to_owned());
            self.print("    <cvar name>          Print the cvar's value".to_owned());
            self.print("    <cvar name> <value>  Set the cvar's value".to_owned());
            return Ok(());
        }

        let cvar_value = match parts.next() {
            Some(val) => val,
            None => {
                let val = cvars.get_string(cvar_name)?;
                self.print(val);
                return Ok(());
            }
        };
        if let Some(rest) = parts.next() {
            return Err(format!("expected only cvar name and value, found {}", rest));
        }
        cvars.set_str(cvar_name, cvar_value)
    }

    fn print(&mut self, text: String) {
        self.push_history_line(text, false);
    }

    fn push_history_line(&mut self, text: String, is_input: bool) {
        let hist_line = HistoryLine::new(text, is_input);
        self.history.push(hist_line);
    }
}

#[derive(Debug, Clone)]
pub struct HistoryLine {
    pub text: String,
    /// Whether the line is input from the user or output from running a command.
    pub is_input: bool,
}

impl HistoryLine {
    pub fn new(text: String, is_input: bool) -> Self {
        Self { text, is_input }
    }
}

/// A mostly internal trait for writing generic code
/// that can access cvars but doesn't know the concrete Cvars struct.
pub trait CvarAccess {
    fn get_string(&self, cvar_name: &str) -> Result<String, String>;
    fn set_str(&mut self, cvar_name: &str, str_value: &str) -> Result<(), String>;
}
