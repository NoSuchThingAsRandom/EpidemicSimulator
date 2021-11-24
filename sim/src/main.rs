mod simulator;

fn main() {
    println!("Hello, world!");
    let sim = simulator::Simulator::new();
    sim.unwrap();
}
