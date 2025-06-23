use std::{io::{self, Write}};
use crossterm::terminal::{self, ClearType};

pub fn clear_screen() {
    print!("{esc}[2J{esc}[H", esc = 27 as char);  // 清屏
    io::stdout().flush().unwrap();
}

pub fn pause() {
    println!("Press any key to continue...");
    let mut _input = String::new();
    io::stdin().read_line(&mut _input).unwrap();
}
