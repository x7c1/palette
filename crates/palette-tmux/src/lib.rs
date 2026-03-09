mod error;
pub use error::{Error, Result};

mod tmux_manager;
pub use tmux_manager::TmuxManager;

mod tmux_manager_impl;
pub use tmux_manager_impl::TmuxManagerImpl;
