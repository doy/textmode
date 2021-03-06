use textmode::TextmodeExt as _;

fn main() {
    let mut tm = textmode::blocking::Textmode::new().unwrap();

    tm.move_to(5, 5);
    tm.write_str("foo");
    std::thread::sleep(std::time::Duration::from_secs(2));
    tm.refresh().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(2));

    tm.move_to(8, 8);
    tm.set_fgcolor(textmode::color::GREEN);
    tm.write_str("bar");
    tm.move_to(11, 11);
    tm.set_fgcolor(vt100::Color::Default);
    tm.write_str("baz");
    std::thread::sleep(std::time::Duration::from_secs(2));
    tm.refresh().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(2));
}
