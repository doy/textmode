use textmode::Textmode as _;

fn main() {
    let mut tm = textmode::blocking::Output::new().unwrap();
    let mut input = textmode::blocking::Input::new().unwrap();

    tm.move_to(5, 5);
    tm.write_str("foo");
    input.read_key().unwrap();
    tm.refresh().unwrap();
    input.read_key().unwrap();

    tm.move_to(8, 8);
    tm.set_fgcolor(textmode::color::GREEN);
    tm.write_str("bar");
    tm.move_to(11, 11);
    tm.set_fgcolor(vt100::Color::Default);
    tm.write_str("baz");
    input.read_key().unwrap();
    tm.refresh().unwrap();
    input.read_key().unwrap();
}
