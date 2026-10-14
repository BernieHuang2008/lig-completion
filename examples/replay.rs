//! Debug helper: replay captured terminal output and print the final screen.
//! `cargo run --example replay -- <file> <rows> <cols>`
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let (rows, cols) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let mut p = vt100::Parser::new(rows, cols, 0);
    p.process(&data);
    let s = p.screen();
    let (cr, cc) = s.cursor_position();
    for (i, row) in s.rows(0, cols).enumerate() {
        let mark = if i as u16 == cr { format!(">{cc:>2}") } else { "   ".into() };
        println!("{mark}|{row}");
    }
}
