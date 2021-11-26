#![allow(dead_code)]

mod simulator;
mod disease;
mod models;
mod error;

fn main() {
    println!("Hello, world!");
    let sim = simulator::Simulator::new();
    let mut sim = sim.unwrap();
    sim.simulate().unwrap();
}
