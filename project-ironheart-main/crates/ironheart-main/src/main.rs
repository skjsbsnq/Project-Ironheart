fn main() {
    let mut app = ironheart_engine::ProjectIronheart::new();
    app.tick();
    println!("{}", app.startup_banner());
}
