mod app;
mod sheet;

use crossterm::{
    event::{
        self, read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use qualia::{Store, Q};
use std::io;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{thread, time::Duration};
use tui::{backend::CrosstermBackend, widgets::Block, Terminal};

use crate::{AHResult, CommonOpts};

pub(crate) fn run_editor(opts: CommonOpts) -> AHResult<()> {
    let store = opts.open_store().unwrap();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES,
        )
    )?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let running = Arc::new(AtomicBool::new(true));

    {
        let running = running.clone();
        ctrlc::set_handler(move || {
            running.store(false, Ordering::SeqCst);
        })?;
    }

    let mut app = app::App::new(store, running.clone());

    while running.load(Ordering::SeqCst) {
        terminal.draw(|f| app.render_to(f))?;
        app.handle(read()?);
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        PopKeyboardEnhancementFlags,
        DisableMouseCapture,
        LeaveAlternateScreen,
    )?;
    terminal.show_cursor()?;

    Ok(())
}
