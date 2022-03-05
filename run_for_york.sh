RUST_LOG="warn,visualisation,osm_data=trace,sim=trace,run=debug,load_census_data=trace,voronoice=off"
RUST_BACKTRACE=1
RAYON_NUM_THREADS=40
#2013265923TYPE299
#1946157112TYPE299
cargo run --release -- 1946157112TYPE299 --directory=data --grid-size=250000 --simulate --use-cache &>logs/v1.5.log