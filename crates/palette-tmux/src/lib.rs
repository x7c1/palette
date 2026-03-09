mod error;
pub use error::{Error, Result};

mod terminal_manager;
pub use terminal_manager::TerminalManager;

mod tmux_manager_impl;
pub use tmux_manager_impl::TmuxManagerImpl;
