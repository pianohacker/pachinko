mod app;
mod item;
mod sheet;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use lazy_static::lazy_static;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{io, time::Duration};
use tui::{backend::CrosstermBackend, Terminal};

use crate::{AHResult, CommonOpts};

static CTRLC_INSTALLED: AtomicBool = AtomicBool::new(false);

lazy_static! {
    static ref RUNNING: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

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

    RUNNING.store(true, Ordering::SeqCst);
    if CTRLC_INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_or_else(|e| e, |a| a)
        == false
    {
        let running = RUNNING.clone();
        ctrlc::set_handler(move || {
            running.store(false, Ordering::SeqCst);
        })?;
    }

    let mut app = app::App::new(store, RUNNING.clone());

    while RUNNING.load(Ordering::SeqCst) {
        terminal.draw(|f| app.render_to(f))?;

        loop {
            if event::poll(Duration::from_secs(1))? {
                if app.handle(event::read()?) {
                    break;
                }
            } else {
                if app.handle_idle() {
                    break;
                }
            }
        }
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
