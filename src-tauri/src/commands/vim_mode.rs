//! Vim mode Tauri commands

use tauri::State;

use crate::AppState;

#[tauri::command]
pub fn get_vim_mode(state: State<AppState>) -> String {
    let vim_state = state.vim_state.lock().unwrap();
    vim_state.mode().as_str().to_string()
}

#[tauri::command]
pub fn get_pending_keys(state: State<AppState>) -> String {
    let vim_state = state.vim_state.lock().unwrap();
    vim_state.get_pending_keys()
}
